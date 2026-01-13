# 06 - Embedding Engine

> **Goal:** Implement text embedding generation using ONNX Runtime and EmbeddingGemma  
> **Time Estimate:** 3-4 days  
> **Prerequisites:** [05-storage-layer.md](./05-storage-layer.md) completed

---

## Table of Contents

1. [Overview](#overview)
2. [Model Selection](#model-selection)
3. [Dependencies](#dependencies)
4. [Model Download](#model-download)
5. [ONNX Runtime Setup](#onnx-runtime-setup)
6. [Embedding Service](#embedding-service)
7. [Text Chunking](#text-chunking)
8. [Batch Processing](#batch-processing)
9. [Tauri Integration](#tauri-integration)
10. [Frontend Components](#frontend-components)
11. [Performance Optimization](#performance-optimization)
12. [Testing](#testing)
13. [Troubleshooting](#troubleshooting)
14. [Acceptance Criteria](#acceptance-criteria)

---

## Overview

The embedding engine converts text into dense vector representations for semantic search (RAG). We use **EmbeddingGemma 300M** with **ONNX Runtime** for efficient, cross-platform inference.

```
                    ┌─────────────────────────────────────┐
                    │         Text Input                   │
                    │   "We discussed the Q1 budget..."    │
                    └──────────────┬──────────────────────┘
                                   │
                                   ▼
                    ┌─────────────────────────────────────┐
                    │         Tokenization                 │
                    │   SentencePiece / Gemma Tokenizer    │
                    └──────────────┬──────────────────────┘
                                   │
                                   ▼
                    ┌─────────────────────────────────────┐
                    │         ONNX Runtime                 │
                    │   EmbeddingGemma 300M (q8)           │
                    │   Hardware: CPU / GPU / NPU          │
                    └──────────────┬──────────────────────┘
                                   │
                                   ▼
                    ┌─────────────────────────────────────┐
                    │         768-dim Vector               │
                    │   [0.023, -0.156, 0.089, ...]        │
                    └──────────────┬──────────────────────┘
                                   │
                                   ▼
                    ┌─────────────────────────────────────┐
                    │         LanceDB Storage              │
                    │   Semantic search ready              │
                    └─────────────────────────────────────┘
```

### Why EmbeddingGemma?

| Aspect | EmbeddingGemma 300M | all-MiniLM | BGE-small |
|--------|---------------------|------------|-----------|
| **Dimensions** | 768 | 384 | 384 |
| **Languages** | 100+ | English | English |
| **Quality** | State-of-the-art | Good | Good |
| **Size (q8)** | ~300MB | ~90MB | ~130MB |
| **Context** | 2048 tokens | 512 tokens | 512 tokens |
| **MRL Support** | Yes (truncatable) | No | No |

**MRL (Matryoshka Representation Learning)**: Can truncate embeddings to 512/256/128 dimensions with minimal quality loss - useful for storage optimization.

---

## Model Selection

### Available Variants

| Model | Size | Performance | Recommended For |
|-------|------|-------------|-----------------|
| **embeddinggemma-300m-q8** | 300MB | Best balance | Default choice |
| embeddinggemma-300m-fp32 | 600MB | Highest quality | High-end hardware |
| embeddinggemma-300m-q4 | 150MB | Fastest | Low-end hardware |

> ⚠️ **Important:** EmbeddingGemma does NOT support fp16. Use fp32, q8, or q4 only.

### Model Source

**HuggingFace Repository:**
- https://huggingface.co/onnx-community/embeddinggemma-e5-300M-ONNX
- Quantized variants available in model tree

### Task-Specific Prompts

EmbeddingGemma uses task-specific prompts for optimal performance:

```rust
/// Document embedding (for storing transcript chunks)
pub fn document_prompt(text: &str) -> String {
    format!("title: none | text: {}", text)
}

/// Search query embedding (for user questions)
pub fn query_prompt(text: &str) -> String {
    format!("task: search result | query: {}", text)
}

/// Question answering query embedding
pub fn qa_prompt(text: &str) -> String {
    format!("task: question answering | query: {}", text)
}
```

---

## Dependencies

### Update Cargo.toml

```toml
[dependencies]
# ONNX Runtime - use load-dynamic for flexibility
ort = { version = "2.0", features = ["load-dynamic"] }

# Tokenizer
tokenizers = "0.19"

# Async runtime
tokio = { version = "1.37", features = ["full"] }

# Utilities
anyhow = "1.0"
tracing = "0.1"

# For downloading models
reqwest = { version = "0.12", features = ["stream"] }
futures-util = "0.3"
indicatif = "0.17"

[target.'cfg(windows)'.dependencies]
ort = { version = "2.0", features = ["load-dynamic", "directml"] }

[target.'cfg(target_os = "macos")'.dependencies]
ort = { version = "2.0", features = ["load-dynamic", "coreml"] }

[target.'cfg(target_os = "linux")'.dependencies]
ort = { version = "2.0", features = ["load-dynamic", "cuda"] }
```

### Crate Documentation

| Crate | Purpose | Docs |
|-------|---------|------|
| **ort** | ONNX Runtime bindings | [docs.rs/ort](https://docs.rs/ort/latest/ort/) |
| **tokenizers** | HuggingFace tokenizers | [docs.rs/tokenizers](https://docs.rs/tokenizers/latest/tokenizers/) |

### External Resources

- [ONNX Runtime GitHub](https://github.com/microsoft/onnxruntime)
- [ort crate examples](https://github.com/pykeio/ort/tree/main/examples)
- [EmbeddingGemma Paper](https://arxiv.org/abs/2403.05151)

---

## Model Download

### Model Downloader

Create `src-tauri/src/inference/model_downloader.rs`:

```rust
use anyhow::{Result, Context};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{info, debug};

/// Model download configuration
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub size_bytes: u64,
    pub checksum: Option<String>,
}

impl ModelConfig {
    pub fn embedding_gemma_q8() -> Self {
        Self {
            id: "embeddinggemma-300m-q8".to_string(),
            name: "EmbeddingGemma 300M (q8)".to_string(),
            url: "https://huggingface.co/onnx-community/embeddinggemma-e5-300M-ONNX/resolve/main/onnx/model_q8.onnx".to_string(),
            size_bytes: 300_000_000,  // ~300MB
            checksum: None,
        }
    }
    
    pub fn embedding_tokenizer() -> Self {
        Self {
            id: "embeddinggemma-tokenizer".to_string(),
            name: "EmbeddingGemma Tokenizer".to_string(),
            url: "https://huggingface.co/onnx-community/embeddinggemma-e5-300M-ONNX/resolve/main/tokenizer.json".to_string(),
            size_bytes: 4_000_000,  // ~4MB
            checksum: None,
        }
    }
}

/// Download manager for ML models
pub struct ModelDownloader {
    client: Client,
    models_dir: PathBuf,
}

impl ModelDownloader {
    pub fn new(models_dir: impl AsRef<Path>) -> Result<Self> {
        let models_dir = models_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&models_dir)
            .context("Failed to create models directory")?;
        
        let client = Client::builder()
            .user_agent("meeting-scribe/1.0")
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self { client, models_dir })
    }
    
    /// Check if model exists locally
    pub fn model_exists(&self, config: &ModelConfig) -> bool {
        self.model_path(config).exists()
    }
    
    /// Get local path for a model
    pub fn model_path(&self, config: &ModelConfig) -> PathBuf {
        let filename = config.url
            .rsplit('/')
            .next()
            .unwrap_or(&config.id);
        self.models_dir.join(&config.id).join(filename)
    }
    
    /// Download model with progress tracking
    pub async fn download<F>(
        &self,
        config: &ModelConfig,
        progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: Fn(DownloadProgress) + Send + 'static,
    {
        let model_dir = self.models_dir.join(&config.id);
        std::fs::create_dir_all(&model_dir)
            .context("Failed to create model directory")?;
        
        let file_path = self.model_path(config);
        
        // Check if already downloaded
        if file_path.exists() {
            let metadata = std::fs::metadata(&file_path)?;
            if metadata.len() >= config.size_bytes / 2 {  // Allow some variance
                info!("Model {} already exists at {:?}", config.id, file_path);
                progress_callback(DownloadProgress {
                    model_id: config.id.clone(),
                    downloaded: config.size_bytes,
                    total: config.size_bytes,
                    status: DownloadStatus::Complete,
                });
                return Ok(file_path);
            }
        }
        
        info!("Downloading {} from {}", config.name, config.url);
        
        // Start download
        let response = self.client
            .get(&config.url)
            .send()
            .await
            .context("Failed to start download")?;
        
        let total_size = response
            .content_length()
            .unwrap_or(config.size_bytes);
        
        progress_callback(DownloadProgress {
            model_id: config.id.clone(),
            downloaded: 0,
            total: total_size,
            status: DownloadStatus::Downloading,
        });
        
        // Stream to file
        let mut file = File::create(&file_path)
            .await
            .context("Failed to create model file")?;
        
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk)
                .await
                .context("Error writing to file")?;
            
            downloaded += chunk.len() as u64;
            
            progress_callback(DownloadProgress {
                model_id: config.id.clone(),
                downloaded,
                total: total_size,
                status: DownloadStatus::Downloading,
            });
        }
        
        file.flush().await?;
        
        progress_callback(DownloadProgress {
            model_id: config.id.clone(),
            downloaded: total_size,
            total: total_size,
            status: DownloadStatus::Complete,
        });
        
        info!("Download complete: {:?}", file_path);
        Ok(file_path)
    }
    
    /// Download embedding model and tokenizer
    pub async fn ensure_embedding_model<F>(
        &self,
        progress_callback: F,
    ) -> Result<EmbeddingModelPaths>
    where
        F: Fn(DownloadProgress) + Send + Clone + 'static,
    {
        let model_config = ModelConfig::embedding_gemma_q8();
        let tokenizer_config = ModelConfig::embedding_tokenizer();
        
        let model_path = self.download(&model_config, progress_callback.clone()).await?;
        let tokenizer_path = self.download(&tokenizer_config, progress_callback).await?;
        
        Ok(EmbeddingModelPaths {
            model: model_path,
            tokenizer: tokenizer_path,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub status: DownloadStatus,
}

impl DownloadProgress {
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f32 / self.total as f32) * 100.0
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Complete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct EmbeddingModelPaths {
    pub model: PathBuf,
    pub tokenizer: PathBuf,
}
```

---

## ONNX Runtime Setup

### Session Configuration

Create `src-tauri/src/inference/onnx_runtime.rs`:

```rust
use ort::{Environment, ExecutionProvider, Session, SessionBuilder, Value};
use std::path::Path;
use std::sync::Arc;
use anyhow::{Result, Context};
use tracing::{info, debug, warn};

/// Global ONNX Runtime environment
static ENVIRONMENT: std::sync::OnceLock<Arc<Environment>> = std::sync::OnceLock::new();

/// Get or initialize the ONNX Runtime environment
pub fn get_environment() -> Arc<Environment> {
    ENVIRONMENT.get_or_init(|| {
        Arc::new(
            Environment::builder()
                .with_name("meeting-scribe")
                .with_execution_providers([
                    // Try GPU acceleration first, fall back to CPU
                    #[cfg(windows)]
                    ExecutionProvider::DirectML(Default::default()),
                    #[cfg(target_os = "macos")]
                    ExecutionProvider::CoreML(Default::default()),
                    #[cfg(target_os = "linux")]
                    ExecutionProvider::CUDA(Default::default()),
                    ExecutionProvider::CPU(Default::default()),
                ])
                .build()
                .expect("Failed to create ONNX Runtime environment")
        )
    }).clone()
}

/// ONNX session wrapper with proper configuration
pub struct OnnxSession {
    session: Session,
    name: String,
}

impl OnnxSession {
    /// Load a model from file
    pub fn load(model_path: impl AsRef<Path>, name: impl Into<String>) -> Result<Self> {
        let model_path = model_path.as_ref();
        let name = name.into();
        
        info!("Loading ONNX model '{}' from {:?}", name, model_path);
        
        let env = get_environment();
        
        let session = SessionBuilder::new(&env)?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(num_cpus::get() as i16)?
            .commit_from_file(model_path)
            .context("Failed to load ONNX model")?;
        
        // Log session info
        debug!(
            "Model '{}' loaded: {} inputs, {} outputs",
            name,
            session.inputs.len(),
            session.outputs.len()
        );
        
        for input in &session.inputs {
            debug!("  Input: {} ({:?})", input.name, input.input_type);
        }
        
        for output in &session.outputs {
            debug!("  Output: {} ({:?})", output.name, output.output_type);
        }
        
        Ok(Self { session, name })
    }
    
    /// Get the underlying session
    pub fn session(&self) -> &Session {
        &self.session
    }
    
    /// Run inference with named inputs
    pub fn run<'a>(&'a self, inputs: Vec<(String, Value<'a>)>) -> Result<Vec<Value<'a>>> {
        let inputs: Vec<(&str, Value)> = inputs
            .iter()
            .map(|(name, value)| (name.as_str(), value.clone()))
            .collect();
        
        self.session
            .run(inputs)
            .context("ONNX inference failed")
    }
}

/// Check available execution providers
pub fn available_providers() -> Vec<String> {
    let mut providers = vec!["CPU".to_string()];
    
    #[cfg(windows)]
    if is_directml_available() {
        providers.push("DirectML".to_string());
    }
    
    #[cfg(target_os = "macos")]
    providers.push("CoreML".to_string());
    
    #[cfg(target_os = "linux")]
    if is_cuda_available() {
        providers.push("CUDA".to_string());
    }
    
    providers
}

#[cfg(windows)]
fn is_directml_available() -> bool {
    // Check if DirectML is available
    std::process::Command::new("dxdiag")
        .arg("/t")
        .arg("dxdiag_output.txt")
        .output()
        .is_ok()
}

#[cfg(target_os = "linux")]
fn is_cuda_available() -> bool {
    // Check if CUDA is available
    std::path::Path::new("/usr/local/cuda").exists()
        || std::env::var("CUDA_HOME").is_ok()
}
```

---

## Embedding Service

### Core Embedding Implementation

Create `src-tauri/src/inference/embedding.rs`:

```rust
use crate::inference::onnx_runtime::OnnxSession;
use anyhow::{Result, Context};
use ndarray::{Array1, Array2, ArrayView2, Axis};
use ort::Value;
use std::path::Path;
use std::sync::Arc;
use tokenizers::Tokenizer;
use tracing::{debug, info};

/// Embedding dimension for EmbeddingGemma
pub const EMBEDDING_DIM: usize = 768;

/// Maximum context length
pub const MAX_TOKENS: usize = 2048;

/// Embedding service for generating text embeddings
pub struct EmbeddingService {
    session: Arc<OnnxSession>,
    tokenizer: Tokenizer,
}

impl EmbeddingService {
    /// Load embedding model and tokenizer
    pub fn load(
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let session = OnnxSession::load(model_path, "embedding")?;
        
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;
        
        info!("Embedding service initialized");
        
        Ok(Self {
            session: Arc::new(session),
            tokenizer,
        })
    }
    
    /// Generate embedding for a single text
    pub fn embed(&self, text: &str, task: EmbeddingTask) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text], task)?;
        Ok(embeddings.into_iter().next().unwrap())
    }
    
    /// Generate embeddings for multiple texts (batched)
    pub fn embed_batch(&self, texts: &[&str], task: EmbeddingTask) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        
        // Apply task-specific prompts
        let prompted_texts: Vec<String> = texts
            .iter()
            .map(|t| task.apply_prompt(t))
            .collect();
        
        // Tokenize all texts
        let encodings = self.tokenizer
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
        
        // Create ONNX tensors
        let input_ids_array = Array2::from_shape_vec(
            (batch_size, max_len),
            input_ids,
        )?;
        
        let attention_mask_array = Array2::from_shape_vec(
            (batch_size, max_len),
            attention_mask,
        )?;
        
        let input_ids_value = Value::from_array(input_ids_array.view())?;
        let attention_mask_value = Value::from_array(attention_mask_array.view())?;
        
        // Run inference
        debug!("Running embedding inference for {} texts", batch_size);
        
        let outputs = self.session.run(vec![
            ("input_ids".to_string(), input_ids_value),
            ("attention_mask".to_string(), attention_mask_value),
        ])?;
        
        // Extract embeddings from output
        let embeddings = self.extract_embeddings(&outputs, batch_size)?;
        
        // Normalize embeddings (L2 normalization)
        let normalized: Vec<Vec<f32>> = embeddings
            .into_iter()
            .map(|e| normalize_l2(&e))
            .collect();
        
        Ok(normalized)
    }
    
    /// Extract embeddings from ONNX output
    fn extract_embeddings(
        &self,
        outputs: &[Value],
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        // Output shape: [batch_size, sequence_length, hidden_size]
        // We use mean pooling over the sequence dimension
        
        let output = outputs
            .first()
            .context("No output from model")?;
        
        let tensor = output
            .try_extract_tensor::<f32>()
            .context("Failed to extract tensor")?;
        
        let view = tensor.view();
        let shape = view.shape();
        
        // Handle different output formats
        if shape.len() == 3 {
            // [batch, seq_len, hidden] - need pooling
            let batch = shape[0];
            let seq_len = shape[1];
            let hidden = shape[2];
            
            let mut embeddings = Vec::with_capacity(batch);
            
            for b in 0..batch {
                let mut pooled = vec![0.0f32; hidden];
                
                for s in 0..seq_len {
                    for h in 0..hidden {
                        pooled[h] += view[[b, s, h]];
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
            
            for b in 0..batch {
                let embedding: Vec<f32> = (0..hidden)
                    .map(|h| view[[b, h]])
                    .collect();
                embeddings.push(embedding);
            }
            
            Ok(embeddings)
        } else {
            anyhow::bail!("Unexpected output shape: {:?}", shape);
        }
    }
    
    /// Get embedding dimension
    pub fn dimension(&self) -> usize {
        EMBEDDING_DIM
    }
    
    /// Truncate embeddings to lower dimension (MRL support)
    pub fn truncate(&self, embedding: &[f32], target_dim: usize) -> Vec<f32> {
        let truncated: Vec<f32> = embedding.iter()
            .take(target_dim)
            .copied()
            .collect();
        normalize_l2(&truncated)
    }
}

/// L2 normalization
fn normalize_l2(vec: &[f32]) -> Vec<f32> {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm > 0.0 {
        vec.iter().map(|x| x / norm).collect()
    } else {
        vec.to_vec()
    }
}

/// Embedding task types (determines prompting strategy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingTask {
    /// Embed documents for storage (transcripts, notes)
    Document,
    /// Embed search queries
    Search,
    /// Embed questions for QA
    QuestionAnswering,
}

impl EmbeddingTask {
    /// Apply task-specific prompt
    pub fn apply_prompt(&self, text: &str) -> String {
        match self {
            Self::Document => format!("title: none | text: {}", text),
            Self::Search => format!("task: search result | query: {}", text),
            Self::QuestionAnswering => format!("task: question answering | query: {}", text),
        }
    }
}

/// Calculate cosine similarity between two embeddings
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
```

---

## Text Chunking

### Intelligent Chunking for Transcripts

Create `src-tauri/src/inference/chunking.rs`:

```rust
use anyhow::Result;

/// Maximum chunk size in characters (roughly 512 tokens)
pub const MAX_CHUNK_CHARS: usize = 2000;

/// Overlap between chunks
pub const CHUNK_OVERLAP: usize = 200;

/// Minimum chunk size
pub const MIN_CHUNK_CHARS: usize = 100;

/// A chunk of text with metadata
#[derive(Debug, Clone)]
pub struct TextChunk {
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub speaker: Option<String>,
    pub chunk_index: usize,
}

/// Chunk transcript segments intelligently
pub fn chunk_transcript(
    segments: &[TranscriptSegmentInput],
    max_chars: usize,
) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut current_start: Option<i64> = None;
    let mut current_end: Option<i64> = None;
    let mut current_speaker: Option<String> = None;
    let mut chunk_index = 0;
    
    for segment in segments {
        let segment_text = format!(
            "[{}] {}\n",
            segment.speaker.as_deref().unwrap_or("Unknown"),
            segment.text
        );
        
        // Check if adding this segment would exceed limit
        if !current_chunk.is_empty() 
            && current_chunk.len() + segment_text.len() > max_chars 
        {
            // Save current chunk
            if current_chunk.len() >= MIN_CHUNK_CHARS {
                chunks.push(TextChunk {
                    text: current_chunk.trim().to_string(),
                    start_ms: current_start,
                    end_ms: current_end,
                    speaker: current_speaker.clone(),
                    chunk_index,
                });
                chunk_index += 1;
            }
            
            // Start new chunk with overlap
            let overlap_text = get_overlap_text(&current_chunk, CHUNK_OVERLAP);
            current_chunk = overlap_text;
            current_start = Some(segment.start_ms);
            current_speaker = segment.speaker.clone();
        }
        
        if current_start.is_none() {
            current_start = Some(segment.start_ms);
        }
        current_end = Some(segment.end_ms);
        
        // Track majority speaker in chunk
        if current_speaker.is_none() {
            current_speaker = segment.speaker.clone();
        }
        
        current_chunk.push_str(&segment_text);
    }
    
    // Don't forget the last chunk
    if current_chunk.len() >= MIN_CHUNK_CHARS {
        chunks.push(TextChunk {
            text: current_chunk.trim().to_string(),
            start_ms: current_start,
            end_ms: current_end,
            speaker: current_speaker,
            chunk_index,
        });
    }
    
    chunks
}

/// Get overlap text from the end of a string
fn get_overlap_text(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return String::new();
    }
    
    // Find a good break point (sentence or word boundary)
    let start_pos = text.len() - max_chars;
    
    // Look for sentence boundary
    let search_text = &text[start_pos..];
    if let Some(pos) = search_text.find(". ") {
        return search_text[pos + 2..].to_string();
    }
    
    // Look for word boundary
    if let Some(pos) = search_text.find(' ') {
        return search_text[pos + 1..].to_string();
    }
    
    search_text.to_string()
}

/// Chunk plain text (for notes, summaries)
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut chunk_index = 0;
    
    // Split by paragraphs first
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    
    let mut current_chunk = String::new();
    
    for para in paragraphs {
        if current_chunk.len() + para.len() + 2 > max_chars && !current_chunk.is_empty() {
            chunks.push(TextChunk {
                text: current_chunk.trim().to_string(),
                start_ms: None,
                end_ms: None,
                speaker: None,
                chunk_index,
            });
            chunk_index += 1;
            
            let overlap = get_overlap_text(&current_chunk, CHUNK_OVERLAP);
            current_chunk = overlap;
        }
        
        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(para);
    }
    
    if !current_chunk.is_empty() {
        chunks.push(TextChunk {
            text: current_chunk.trim().to_string(),
            start_ms: None,
            end_ms: None,
            speaker: None,
            chunk_index,
        });
    }
    
    // Handle very long paragraphs
    chunks = chunks
        .into_iter()
        .flat_map(|chunk| {
            if chunk.text.len() > max_chars * 2 {
                split_long_chunk(&chunk, max_chars)
            } else {
                vec![chunk]
            }
        })
        .collect();
    
    chunks
}

/// Split a very long chunk into smaller pieces
fn split_long_chunk(chunk: &TextChunk, max_chars: usize) -> Vec<TextChunk> {
    let mut result = Vec::new();
    let text = &chunk.text;
    let mut start = 0;
    let mut chunk_index = chunk.chunk_index;
    
    while start < text.len() {
        let end = (start + max_chars).min(text.len());
        
        // Find a good break point
        let actual_end = if end < text.len() {
            text[start..end]
                .rfind(|c: char| c == '.' || c == '!' || c == '?' || c == '\n')
                .map(|pos| start + pos + 1)
                .unwrap_or(end)
        } else {
            end
        };
        
        result.push(TextChunk {
            text: text[start..actual_end].trim().to_string(),
            start_ms: chunk.start_ms,
            end_ms: chunk.end_ms,
            speaker: chunk.speaker.clone(),
            chunk_index,
        });
        
        start = actual_end;
        chunk_index += 1;
    }
    
    result
}

/// Input format for transcript segments
#[derive(Debug, Clone)]
pub struct TranscriptSegmentInput {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker: Option<String>,
}
```

---

## Batch Processing

### Efficient Pipeline

Create `src-tauri/src/inference/embedding_pipeline.rs`:

```rust
use crate::inference::embedding::{EmbeddingService, EmbeddingTask};
use crate::inference::chunking::{chunk_transcript, chunk_text, TextChunk, TranscriptSegmentInput, MAX_CHUNK_CHARS};
use crate::storage::{VectorStore, EmbeddingRecord};
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, debug};

/// Batch size for embedding generation
const EMBEDDING_BATCH_SIZE: usize = 8;

/// Pipeline for embedding generation and storage
pub struct EmbeddingPipeline {
    embedding_service: Arc<EmbeddingService>,
    vector_store: Arc<VectorStore>,
}

impl EmbeddingPipeline {
    pub fn new(
        embedding_service: Arc<EmbeddingService>,
        vector_store: Arc<VectorStore>,
    ) -> Self {
        Self {
            embedding_service,
            vector_store,
        }
    }
    
    /// Process transcript segments and store embeddings
    pub async fn process_transcript(
        &self,
        meeting_id: &str,
        segments: Vec<TranscriptSegmentInput>,
        progress_callback: impl Fn(EmbeddingProgress),
    ) -> Result<ProcessingResult> {
        info!("Processing transcript for meeting {}", meeting_id);
        
        // Chunk the transcript
        let chunks = chunk_transcript(&segments, MAX_CHUNK_CHARS);
        let total_chunks = chunks.len();
        
        debug!("Created {} chunks from {} segments", total_chunks, segments.len());
        
        progress_callback(EmbeddingProgress {
            stage: "chunking".to_string(),
            current: 0,
            total: total_chunks,
        });
        
        // Generate embeddings in batches
        let mut records = Vec::new();
        
        for (batch_idx, batch) in chunks.chunks(EMBEDDING_BATCH_SIZE).enumerate() {
            let texts: Vec<&str> = batch.iter().map(|c| c.text.as_str()).collect();
            
            let embeddings = self.embedding_service
                .embed_batch(&texts, EmbeddingTask::Document)?;
            
            for (chunk, embedding) in batch.iter().zip(embeddings) {
                records.push(EmbeddingRecord::new_transcript(
                    meeting_id,
                    &chunk.text,
                    chunk.start_ms.unwrap_or(0),
                    embedding,
                ));
            }
            
            let processed = (batch_idx + 1) * EMBEDDING_BATCH_SIZE;
            progress_callback(EmbeddingProgress {
                stage: "embedding".to_string(),
                current: processed.min(total_chunks),
                total: total_chunks,
            });
        }
        
        // Store in vector database
        progress_callback(EmbeddingProgress {
            stage: "storing".to_string(),
            current: 0,
            total: records.len(),
        });
        
        self.vector_store.add_embeddings(records.clone()).await?;
        
        progress_callback(EmbeddingProgress {
            stage: "complete".to_string(),
            current: total_chunks,
            total: total_chunks,
        });
        
        info!(
            "Stored {} embeddings for meeting {}",
            records.len(),
            meeting_id
        );
        
        Ok(ProcessingResult {
            meeting_id: meeting_id.to_string(),
            chunks_processed: total_chunks,
            embeddings_stored: records.len(),
        })
    }
    
    /// Process a note and store embedding
    pub async fn process_note(
        &self,
        meeting_id: &str,
        note_id: i64,
        content: &str,
    ) -> Result<String> {
        let chunks = chunk_text(content, MAX_CHUNK_CHARS);
        
        let mut records = Vec::new();
        
        for chunk in &chunks {
            let embedding = self.embedding_service
                .embed(&chunk.text, EmbeddingTask::Document)?;
            
            records.push(EmbeddingRecord::new_note(
                meeting_id,
                &chunk.text,
                embedding,
            ));
        }
        
        self.vector_store.add_embeddings(records.clone()).await?;
        
        // Return first embedding ID for reference
        Ok(records.first()
            .map(|r| r.id.clone())
            .unwrap_or_default())
    }
    
    /// Process a summary and store embedding
    pub async fn process_summary(
        &self,
        meeting_id: &str,
        summary_id: i64,
        content: &str,
    ) -> Result<String> {
        let chunks = chunk_text(content, MAX_CHUNK_CHARS);
        
        let mut records = Vec::new();
        
        for chunk in &chunks {
            let embedding = self.embedding_service
                .embed(&chunk.text, EmbeddingTask::Document)?;
            
            records.push(EmbeddingRecord::new_summary(
                meeting_id,
                &chunk.text,
                embedding,
            ));
        }
        
        self.vector_store.add_embeddings(records.clone()).await?;
        
        Ok(records.first()
            .map(|r| r.id.clone())
            .unwrap_or_default())
    }
    
    /// Embed a query for search
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embedding_service.embed(query, EmbeddingTask::Search)
    }
    
    /// Embed a question for QA
    pub fn embed_question(&self, question: &str) -> Result<Vec<f32>> {
        self.embedding_service.embed(question, EmbeddingTask::QuestionAnswering)
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingProgress {
    pub stage: String,
    pub current: usize,
    pub total: usize,
}

impl EmbeddingProgress {
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f32 / self.total as f32) * 100.0
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessingResult {
    pub meeting_id: String,
    pub chunks_processed: usize,
    pub embeddings_stored: usize,
}
```

---

## Tauri Integration

### Embedding Commands

Create `src-tauri/src/commands/embedding.rs`:

```rust
use crate::inference::{
    EmbeddingService, EmbeddingPipeline, EmbeddingProgress,
    ModelDownloader, ModelConfig, DownloadProgress,
};
use crate::storage::{StorageState, TranscriptSegment};
use crate::inference::chunking::TranscriptSegmentInput;
use tauri::{State, AppHandle, Manager};
use std::sync::Arc;
use tokio::sync::Mutex;

type EmbeddingServiceHandle = Arc<Mutex<Option<EmbeddingService>>>;

/// Initialize embedding service (download model if needed)
#[tauri::command]
pub async fn initialize_embedding(
    app: AppHandle,
    embedding: State<'_, EmbeddingServiceHandle>,
) -> Result<bool, String> {
    let mut service = embedding.lock().await;
    
    if service.is_some() {
        return Ok(true);  // Already initialized
    }
    
    // Get data directory
    let data_dir = app.path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    
    let models_dir = data_dir.join("models").join("embedding");
    let downloader = ModelDownloader::new(&models_dir)
        .map_err(|e| e.to_string())?;
    
    // Download model with progress events
    let paths = downloader
        .ensure_embedding_model(move |progress| {
            let _ = app.emit("embedding-download-progress", &progress);
        })
        .await
        .map_err(|e| e.to_string())?;
    
    // Load model
    let embedding_service = EmbeddingService::load(&paths.model, &paths.tokenizer)
        .map_err(|e| e.to_string())?;
    
    *service = Some(embedding_service);
    
    Ok(true)
}

/// Check if embedding model is ready
#[tauri::command]
pub async fn is_embedding_ready(
    embedding: State<'_, EmbeddingServiceHandle>,
) -> Result<bool, String> {
    let service = embedding.lock().await;
    Ok(service.is_some())
}

/// Generate embedding for text
#[tauri::command]
pub async fn embed_text(
    embedding: State<'_, EmbeddingServiceHandle>,
    text: String,
    task: String,
) -> Result<Vec<f32>, String> {
    let service = embedding.lock().await;
    let service = service.as_ref()
        .ok_or("Embedding service not initialized")?;
    
    let task = match task.as_str() {
        "document" => crate::inference::embedding::EmbeddingTask::Document,
        "search" => crate::inference::embedding::EmbeddingTask::Search,
        "qa" => crate::inference::embedding::EmbeddingTask::QuestionAnswering,
        _ => crate::inference::embedding::EmbeddingTask::Document,
    };
    
    service.embed(&text, task)
        .map_err(|e| e.to_string())
}

/// Process meeting transcript and store embeddings
#[tauri::command]
pub async fn embed_meeting_transcript(
    app: AppHandle,
    embedding: State<'_, EmbeddingServiceHandle>,
    storage: State<'_, Arc<Mutex<StorageState>>>,
    meeting_id: String,
) -> Result<crate::inference::embedding_pipeline::ProcessingResult, String> {
    // Get embedding service
    let service = embedding.lock().await;
    let service = service.as_ref()
        .ok_or("Embedding service not initialized")?;
    
    // Get storage
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    // Load transcript segments
    let segments = repos.transcripts
        .get_by_meeting(&meeting_id)
        .map_err(|e| e.to_string())?;
    
    // Convert to input format
    let segment_inputs: Vec<TranscriptSegmentInput> = segments
        .iter()
        .map(|s| TranscriptSegmentInput {
            text: s.text.clone(),
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            speaker: Some(s.speaker.as_str().to_string()),
        })
        .collect();
    
    // Create pipeline
    let pipeline = EmbeddingPipeline::new(
        Arc::new(service.clone()),
        storage.vectors.clone(),
    );
    
    // Process with progress events
    let app_clone = app.clone();
    let result = pipeline
        .process_transcript(&meeting_id, segment_inputs, move |progress| {
            let _ = app_clone.emit("embedding-progress", &progress);
        })
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(result)
}

/// Calculate similarity between two embeddings
#[tauri::command]
pub fn calculate_similarity(
    embedding_a: Vec<f32>,
    embedding_b: Vec<f32>,
) -> Result<f32, String> {
    Ok(crate::inference::embedding::cosine_similarity(&embedding_a, &embedding_b))
}

/// Get embedding model info
#[tauri::command]
pub async fn get_embedding_info(
    embedding: State<'_, EmbeddingServiceHandle>,
) -> Result<EmbeddingInfo, String> {
    let service = embedding.lock().await;
    
    Ok(EmbeddingInfo {
        loaded: service.is_some(),
        dimension: crate::inference::embedding::EMBEDDING_DIM,
        max_tokens: crate::inference::embedding::MAX_TOKENS,
        model_name: "EmbeddingGemma 300M (q8)".to_string(),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingInfo {
    pub loaded: bool,
    pub dimension: usize,
    pub max_tokens: usize,
    pub model_name: String,
}

// Clone implementation for EmbeddingService (if needed)
impl Clone for EmbeddingService {
    fn clone(&self) -> Self {
        // Note: This creates a new session, use Arc in production
        unimplemented!("Use Arc<EmbeddingService> for sharing")
    }
}
```

### Register Commands

Update `src-tauri/src/main.rs`:

```rust
mod inference;
mod commands;

use commands::embedding::*;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // ... existing setup ...
    
    // Initialize embedding service handle (lazy loading)
    let embedding_handle: Arc<Mutex<Option<inference::EmbeddingService>>> = 
        Arc::new(Mutex::new(None));
    
    tauri::Builder::default()
        .manage(embedding_handle)
        // ... existing state ...
        .invoke_handler(tauri::generate_handler![
            // Embedding commands
            initialize_embedding,
            is_embedding_ready,
            embed_text,
            embed_meeting_transcript,
            calculate_similarity,
            get_embedding_info,
            // ... existing commands ...
        ])
        .run(tauri::generate_context!())
        .expect("Error running tauri application");
}
```

---

## Frontend Components

### TypeScript Types

Create `src/types/embedding.ts`:

```typescript
export interface EmbeddingInfo {
  loaded: boolean;
  dimension: number;
  max_tokens: number;
  model_name: string;
}

export interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  status: 'pending' | 'downloading' | 'complete' | 'error';
}

export interface EmbeddingProgress {
  stage: string;
  current: number;
  total: number;
}

export interface ProcessingResult {
  meeting_id: string;
  chunks_processed: number;
  embeddings_stored: number;
}
```

### API Client

Create `src/lib/embedding-api.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  EmbeddingInfo,
  DownloadProgress,
  EmbeddingProgress,
  ProcessingResult,
} from '../types/embedding';

// Initialize embedding service (downloads model if needed)
export async function initializeEmbedding(): Promise<boolean> {
  return invoke('initialize_embedding');
}

// Check if embedding model is ready
export async function isEmbeddingReady(): Promise<boolean> {
  return invoke('is_embedding_ready');
}

// Generate embedding for text
export async function embedText(
  text: string,
  task: 'document' | 'search' | 'qa' = 'document'
): Promise<number[]> {
  return invoke('embed_text', { text, task });
}

// Process meeting transcript and store embeddings
export async function embedMeetingTranscript(
  meetingId: string
): Promise<ProcessingResult> {
  return invoke('embed_meeting_transcript', { meetingId });
}

// Calculate similarity between two embeddings
export async function calculateSimilarity(
  embeddingA: number[],
  embeddingB: number[]
): Promise<number> {
  return invoke('calculate_similarity', { embeddingA, embeddingB });
}

// Get embedding model info
export async function getEmbeddingInfo(): Promise<EmbeddingInfo> {
  return invoke('get_embedding_info');
}

// Listen for download progress
export function onDownloadProgress(
  callback: (progress: DownloadProgress) => void
): Promise<() => void> {
  return listen<DownloadProgress>('embedding-download-progress', (event) => {
    callback(event.payload);
  });
}

// Listen for embedding progress
export function onEmbeddingProgress(
  callback: (progress: EmbeddingProgress) => void
): Promise<() => void> {
  return listen<EmbeddingProgress>('embedding-progress', (event) => {
    callback(event.payload);
  });
}
```

### React Hook

Create `src/hooks/useEmbedding.ts`:

```typescript
import { useState, useEffect, useCallback } from 'react';
import * as embeddingApi from '../lib/embedding-api';
import type { EmbeddingInfo, DownloadProgress, EmbeddingProgress } from '../types/embedding';

export function useEmbedding() {
  const [info, setInfo] = useState<EmbeddingInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Check initial state
  useEffect(() => {
    embeddingApi.getEmbeddingInfo()
      .then(setInfo)
      .catch((err) => setError(err.toString()));
  }, []);

  // Listen for download progress
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    embeddingApi.onDownloadProgress((progress) => {
      setDownloadProgress(progress);
      if (progress.status === 'complete') {
        // Refresh info after download
        embeddingApi.getEmbeddingInfo().then(setInfo);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const initialize = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      await embeddingApi.initializeEmbedding();
      const newInfo = await embeddingApi.getEmbeddingInfo();
      setInfo(newInfo);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to initialize');
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    info,
    loading,
    downloadProgress,
    error,
    initialize,
    isReady: info?.loaded ?? false,
  };
}
```

### Download Progress Component

Create `src/components/EmbeddingDownload.tsx`:

```tsx
import React from 'react';
import { useEmbedding } from '../hooks/useEmbedding';
import { formatBytes } from '../lib/storage-api';

export function EmbeddingDownload() {
  const { info, loading, downloadProgress, error, initialize, isReady } = useEmbedding();

  if (isReady) {
    return (
      <div className="p-4 bg-green-50 rounded-lg">
        <div className="flex items-center gap-2">
          <span className="text-green-600">✓</span>
          <span className="font-medium">Embedding Model Ready</span>
        </div>
        <p className="text-sm text-gray-600 mt-1">
          {info?.model_name} ({info?.dimension} dimensions)
        </p>
      </div>
    );
  }

  if (downloadProgress && downloadProgress.status === 'downloading') {
    const percentage = (downloadProgress.downloaded / downloadProgress.total) * 100;
    
    return (
      <div className="p-4 bg-blue-50 rounded-lg">
        <div className="flex items-center justify-between mb-2">
          <span className="font-medium">Downloading Embedding Model</span>
          <span className="text-sm text-gray-600">
            {formatBytes(downloadProgress.downloaded)} / {formatBytes(downloadProgress.total)}
          </span>
        </div>
        <div className="w-full bg-gray-200 rounded-full h-2">
          <div
            className="bg-blue-600 h-2 rounded-full transition-all"
            style={{ width: `${percentage}%` }}
          />
        </div>
        <p className="text-sm text-gray-600 mt-2">
          {percentage.toFixed(1)}% complete
        </p>
      </div>
    );
  }

  return (
    <div className="p-4 bg-yellow-50 rounded-lg">
      <div className="flex items-center justify-between">
        <div>
          <span className="font-medium">Embedding Model Required</span>
          <p className="text-sm text-gray-600 mt-1">
            Download EmbeddingGemma 300M (~300 MB) for semantic search
          </p>
        </div>
        <button
          onClick={initialize}
          disabled={loading}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? 'Initializing...' : 'Download'}
        </button>
      </div>
      {error && (
        <p className="text-sm text-red-600 mt-2">{error}</p>
      )}
    </div>
  );
}
```

---

## Performance Optimization

### Batch Size Tuning

```rust
// Optimal batch sizes by hardware
const BATCH_SIZE_CPU: usize = 4;
const BATCH_SIZE_GPU: usize = 16;
const BATCH_SIZE_HIGH_VRAM: usize = 32;

fn optimal_batch_size() -> usize {
    // Check available VRAM/memory and adjust
    if has_gpu_with_vram(8000) {  // 8GB+
        BATCH_SIZE_HIGH_VRAM
    } else if has_gpu() {
        BATCH_SIZE_GPU
    } else {
        BATCH_SIZE_CPU
    }
}
```

### Caching Embeddings

```rust
use std::collections::HashMap;
use parking_lot::RwLock;

/// LRU cache for embeddings
pub struct EmbeddingCache {
    cache: RwLock<HashMap<String, Vec<f32>>>,
    max_size: usize,
}

impl EmbeddingCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_size,
        }
    }
    
    pub fn get(&self, key: &str) -> Option<Vec<f32>> {
        self.cache.read().get(key).cloned()
    }
    
    pub fn insert(&self, key: String, value: Vec<f32>) {
        let mut cache = self.cache.write();
        
        // Simple eviction if at capacity
        if cache.len() >= self.max_size {
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }
        
        cache.insert(key, value);
    }
}
```

### Performance Metrics

| Operation | CPU Time | GPU Time (DirectML) |
|-----------|----------|---------------------|
| Single embedding | ~100ms | ~20ms |
| Batch of 8 | ~400ms | ~50ms |
| 1-hour transcript (~100 chunks) | ~5s | ~1s |

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_chunking() {
        let segments = vec![
            TranscriptSegmentInput {
                text: "Hello world".to_string(),
                start_ms: 0,
                end_ms: 1000,
                speaker: Some("you".to_string()),
            },
            TranscriptSegmentInput {
                text: "How are you".to_string(),
                start_ms: 1000,
                end_ms: 2000,
                speaker: Some("others".to_string()),
            },
        ];
        
        let chunks = chunk_transcript(&segments, 100);
        
        assert!(!chunks.is_empty());
        assert!(chunks[0].text.contains("Hello"));
    }
    
    #[test]
    fn test_normalize_l2() {
        let vec = vec![3.0, 4.0];
        let normalized = normalize_l2(&vec);
        
        // Should be [0.6, 0.8] (unit vector)
        assert!((normalized[0] - 0.6).abs() < 0.001);
        assert!((normalized[1] - 0.8).abs() < 0.001);
        
        // Magnitude should be 1.0
        let mag: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((mag - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        let c = vec![0.0, 1.0];
        
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);  // Same direction
        assert!(cosine_similarity(&a, &c).abs() < 0.001);  // Perpendicular
    }
}
```

### Integration Test

```rust
#[tokio::test]
async fn test_embedding_pipeline() {
    // Skip if model not available
    let model_path = std::env::var("EMBEDDING_MODEL_PATH").ok();
    if model_path.is_none() {
        eprintln!("Skipping embedding test - model not available");
        return;
    }
    
    let model_path = model_path.unwrap();
    let tokenizer_path = std::env::var("EMBEDDING_TOKENIZER_PATH").unwrap();
    
    let service = EmbeddingService::load(&model_path, &tokenizer_path).unwrap();
    
    // Test single embedding
    let embedding = service.embed("Hello world", EmbeddingTask::Document).unwrap();
    assert_eq!(embedding.len(), EMBEDDING_DIM);
    
    // Test batch embedding
    let texts = vec!["First text", "Second text", "Third text"];
    let embeddings = service.embed_batch(&texts, EmbeddingTask::Document).unwrap();
    assert_eq!(embeddings.len(), 3);
    
    // Similar texts should have higher similarity
    let sim1 = cosine_similarity(&embeddings[0], &embeddings[1]);
    let sim2 = cosine_similarity(
        &service.embed("Cat", EmbeddingTask::Document).unwrap(),
        &service.embed("Dog", EmbeddingTask::Document).unwrap(),
    );
    let sim3 = cosine_similarity(
        &service.embed("Cat", EmbeddingTask::Document).unwrap(),
        &service.embed("Mathematics", EmbeddingTask::Document).unwrap(),
    );
    
    // Cat-Dog should be more similar than Cat-Mathematics
    assert!(sim2 > sim3);
}
```

---

## Troubleshooting

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| **"Model not found"** | Model not downloaded | Call `initialize_embedding` first |
| **"ONNX Runtime error"** | Missing runtime | Ensure `ort` features are correct |
| **Slow inference** | No GPU acceleration | Check execution providers |
| **OOM errors** | Batch too large | Reduce `EMBEDDING_BATCH_SIZE` |
| **Wrong dimensions** | Model mismatch | Verify model version |

### Debug Logging

```rust
// Enable ONNX Runtime logging
std::env::set_var("ORT_LOG_LEVEL", "verbose");

// Enable tracing
std::env::set_var("RUST_LOG", "meeting_scribe::inference=debug");
```

### Verify GPU Usage

```rust
pub fn log_execution_providers() {
    let providers = available_providers();
    info!("Available execution providers: {:?}", providers);
    
    #[cfg(windows)]
    {
        if providers.contains(&"DirectML".to_string()) {
            info!("DirectML GPU acceleration available");
        }
    }
}
```

---

## Acceptance Criteria

### Required

- [ ] Model downloads successfully with progress tracking
- [ ] Tokenizer loads and processes text correctly
- [ ] Embeddings generate with correct dimension (768)
- [ ] Batch processing works efficiently
- [ ] Embeddings store in LanceDB
- [ ] Task-specific prompts applied correctly

### Nice to Have

- [ ] GPU acceleration working (DirectML/CoreML/CUDA)
- [ ] MRL dimension truncation supported
- [ ] Embedding cache reduces redundant computation
- [ ] Progress events emit to frontend

---

## Next Steps

After completing the embedding engine:

1. **[07-llm-engine.md](./07-llm-engine.md)** - Implement summarization with llama-cpp
2. **[09-rag-implementation.md](./09-rag-implementation.md)** - Connect embeddings to RAG chat

---

## References

### Documentation

- [ort (ONNX Runtime for Rust)](https://docs.rs/ort/latest/ort/)
- [HuggingFace Tokenizers](https://docs.rs/tokenizers/latest/tokenizers/)
- [ONNX Runtime](https://onnxruntime.ai/docs/)
- [EmbeddingGemma Model Card](https://huggingface.co/onnx-community/embeddinggemma-e5-300M-ONNX)

### Examples

- [ort examples](https://github.com/pykeio/ort/tree/main/examples)
- [Sentence Transformers](https://www.sbert.net/)

### Papers

- [EmbeddingGemma: Training Large Embedding Models](https://arxiv.org/abs/2403.05151)
- [Matryoshka Representation Learning](https://arxiv.org/abs/2205.13147)
