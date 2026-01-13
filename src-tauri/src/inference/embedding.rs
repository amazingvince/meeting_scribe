//! Embedding service for generating text embeddings
//!
//! Uses ONNX Runtime with EmbeddingGemma model for semantic search.

use anyhow::{Context, Result};
use ndarray::Array2;
use ort::{inputs, session::Session, value::TensorRef};
use parking_lot::Mutex;
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// Embedding dimension for EmbeddingGemma (all variants)
pub const EMBEDDING_DIM: usize = 768;

/// Maximum context length in tokens
pub const MAX_TOKENS: usize = 2048;

/// Embedding service for generating text embeddings
pub struct EmbeddingService {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
}

impl EmbeddingService {
    /// Load embedding model and tokenizer from files
    pub fn load(model_path: impl AsRef<Path>, tokenizer_path: impl AsRef<Path>) -> Result<Self> {
        let model_path = model_path.as_ref();
        let tokenizer_path = tokenizer_path.as_ref();

        info!("Loading embedding model from {:?}", model_path);

        // Create ONNX session with optimizations
        let session = Session::builder()?
            .with_intra_threads(num_cpus::get())?
            .commit_from_file(model_path)
            .context("Failed to load ONNX model")?;

        // Log session info
        debug!(
            "Embedding model loaded: {} inputs, {} outputs",
            session.inputs.len(),
            session.outputs.len()
        );

        for input in &session.inputs {
            debug!("  Input: {:?}", input);
        }

        for output in &session.outputs {
            debug!("  Output: {:?}", output);
        }

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        info!("Embedding service initialized successfully");

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }

    /// Generate embedding for a single text
    pub fn embed(&self, text: &str, task: EmbeddingTask) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text], task)?;
        embeddings
            .into_iter()
            .next()
            .context("No embedding generated")
    }

    /// Generate embeddings for multiple texts (batched for efficiency)
    pub fn embed_batch(&self, texts: &[&str], task: EmbeddingTask) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Apply task-specific prompts
        let prompted_texts: Vec<String> = texts.iter().map(|t| task.apply_prompt(t)).collect();

        // Tokenize all texts
        let encodings = self
            .tokenizer
            .encode_batch(prompted_texts.iter().map(|s| s.as_str()).collect(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        // Find max length for padding
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0)
            .min(MAX_TOKENS);

        let batch_size = texts.len();

        // Prepare input tensors with padding
        let mut input_ids = vec![0i64; batch_size * max_len];
        let mut attention_mask = vec![0i64; batch_size * max_len];

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(max_len);

            for j in 0..len {
                input_ids[i * max_len + j] = ids[j] as i64;
                attention_mask[i * max_len + j] = mask[j] as i64;
            }
        }

        // Create ndarray tensors
        let input_ids_array =
            Array2::from_shape_vec((batch_size, max_len), input_ids).context("Invalid shape")?;

        let attention_mask_array =
            Array2::from_shape_vec((batch_size, max_len), attention_mask).context("Invalid shape")?;

        // Run inference
        debug!(
            "Running embedding inference for {} texts (max_len={})",
            batch_size, max_len
        );

        let mut session = self.session.lock();
        let outputs = session.run(inputs![
            "input_ids" => TensorRef::from_array_view(&input_ids_array)?,
            "attention_mask" => TensorRef::from_array_view(&attention_mask_array)?,
        ])?;

        // Extract embeddings from output
        let embeddings = self.extract_embeddings(&outputs, batch_size)?;

        // Normalize embeddings (L2 normalization)
        let normalized: Vec<Vec<f32>> = embeddings.into_iter().map(|e| normalize_l2(&e)).collect();

        Ok(normalized)
    }

    /// Extract embeddings from ONNX output
    fn extract_embeddings(
        &self,
        outputs: &ort::session::SessionOutputs<'_>,
        _batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        // Get first output (embeddings) by index
        let output = &outputs[0];

        let tensor = output
            .try_extract_array::<f32>()
            .context("Failed to extract tensor")?;

        let shape = tensor.shape();

        // Handle different output formats
        if shape.len() == 3 {
            // [batch, seq_len, hidden] - need mean pooling
            let batch = shape[0];
            let seq_len = shape[1];
            let hidden = shape[2];

            let mut embeddings = Vec::with_capacity(batch);
            let data = tensor.as_slice().context("Failed to get tensor data")?;

            for b in 0..batch {
                let mut pooled = vec![0.0f32; hidden];

                for s in 0..seq_len {
                    for h in 0..hidden {
                        let idx = b * seq_len * hidden + s * hidden + h;
                        pooled[h] += data[idx];
                    }
                }

                // Mean pooling
                for h in 0..hidden {
                    pooled[h] /= seq_len as f32;
                }

                embeddings.push(pooled);
            }

            Ok(embeddings)
        } else if shape.len() == 2 {
            // [batch, hidden] - already pooled
            let batch = shape[0];
            let hidden = shape[1];

            let mut embeddings = Vec::with_capacity(batch);
            let data = tensor.as_slice().context("Failed to get tensor data")?;

            for b in 0..batch {
                let start = b * hidden;
                let end = start + hidden;
                embeddings.push(data[start..end].to_vec());
            }

            Ok(embeddings)
        } else {
            anyhow::bail!("Unexpected output shape: {:?}", shape);
        }
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        EMBEDDING_DIM
    }

    /// Truncate embeddings to lower dimension (MRL/Matryoshka support)
    ///
    /// EmbeddingGemma supports truncation to 512, 256, or 128 dimensions
    /// with minimal quality loss.
    pub fn truncate(&self, embedding: &[f32], target_dim: usize) -> Vec<f32> {
        let truncated: Vec<f32> = embedding.iter().take(target_dim).copied().collect();
        normalize_l2(&truncated)
    }
}

/// L2 normalization - ensures vector has unit length
fn normalize_l2(vec: &[f32]) -> Vec<f32> {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm > 0.0 {
        vec.iter().map(|x| x / norm).collect()
    } else {
        vec.to_vec()
    }
}

/// Embedding task types (determines prompting strategy)
///
/// EmbeddingGemma uses task-specific prompts for optimal performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingTask {
    /// Embed documents for storage (transcripts, notes, summaries)
    Document,
    /// Embed search queries
    Search,
    /// Embed questions for QA
    QuestionAnswering,
}

impl EmbeddingTask {
    /// Apply task-specific prompt per EmbeddingGemma requirements
    pub fn apply_prompt(&self, text: &str) -> String {
        match self {
            Self::Document => format!("title: none | text: {}", text),
            Self::Search => format!("task: search result | query: {}", text),
            Self::QuestionAnswering => format!("task: question answering | query: {}", text),
        }
    }
}

impl Default for EmbeddingTask {
    fn default() -> Self {
        Self::Document
    }
}

/// Calculate cosine similarity between two embeddings
///
/// Returns a value between -1.0 and 1.0, where:
/// - 1.0 means identical direction
/// - 0.0 means orthogonal (unrelated)
/// - -1.0 means opposite direction
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_l2() {
        let vec = vec![3.0, 4.0];
        let normalized = normalize_l2(&vec);

        // Should be [0.6, 0.8] (unit vector for 3-4-5 triangle)
        assert!((normalized[0] - 0.6).abs() < 0.001);
        assert!((normalized[1] - 0.8).abs() < 0.001);

        // Magnitude should be 1.0
        let mag: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((mag - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let vec = vec![0.0, 0.0, 0.0];
        let normalized = normalize_l2(&vec);

        // Zero vector should remain zero
        assert_eq!(normalized, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001); // Same direction
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];

        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001); // Perpendicular
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001); // Opposite direction
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0); // Different lengths should return 0
    }

    #[test]
    fn test_embedding_task_prompts() {
        let text = "Hello world";

        let doc_prompt = EmbeddingTask::Document.apply_prompt(text);
        assert!(doc_prompt.contains("title: none | text:"));

        let search_prompt = EmbeddingTask::Search.apply_prompt(text);
        assert!(search_prompt.contains("task: search result | query:"));

        let qa_prompt = EmbeddingTask::QuestionAnswering.apply_prompt(text);
        assert!(qa_prompt.contains("task: question answering | query:"));
    }

    #[test]
    fn test_embedding_task_default() {
        assert_eq!(EmbeddingTask::default(), EmbeddingTask::Document);
    }
}
