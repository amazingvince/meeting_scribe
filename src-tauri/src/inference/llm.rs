//! LLM service for text generation using llama.cpp
//!
//! Provides local LLM inference for summarization, action item extraction,
//! and conversational chat using GGUF models.

use anyhow::{anyhow, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::models::LlmModel;

/// LLM generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Temperature for sampling (0.0 = deterministic, higher = more random)
    pub temperature: f32,
    /// Top-p (nucleus) sampling threshold
    pub top_p: f32,
    /// Top-k sampling (consider only top k tokens)
    pub top_k: i32,
    /// Repetition penalty
    pub repeat_penalty: f32,
    /// Stop sequences (generation stops when any are encountered)
    pub stop_sequences: Vec<String>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            stop_sequences: vec![],
        }
    }
}

impl GenerationConfig {
    /// Configuration optimized for summarization (lower temperature for consistency)
    pub fn for_summarization() -> Self {
        Self {
            max_tokens: 1500,
            temperature: 0.3,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.15,
            stop_sequences: vec![],
        }
    }

    /// Configuration optimized for conversational chat
    pub fn for_chat() -> Self {
        Self {
            max_tokens: 1024,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            stop_sequences: vec![],
        }
    }

    /// Configuration for title generation (very short output)
    pub fn for_title() -> Self {
        Self {
            max_tokens: 20,
            temperature: 0.5,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.0,
            stop_sequences: vec!["\n".to_string()],
        }
    }

    /// Configuration for JSON extraction (low temperature, deterministic)
    pub fn for_json() -> Self {
        Self {
            max_tokens: 1000,
            temperature: 0.2,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.0,
            stop_sequences: vec!["```".to_string()],
        }
    }
}

/// Main LLM service for inference
pub struct LlmService {
    backend: LlamaBackend,
    model: Option<LlamaModel>,
    model_path: PathBuf,
    current_model: Option<LlmModel>,
    gpu_layers: u32,
}

impl LlmService {
    /// Create a new LLM service
    pub fn new(models_dir: PathBuf) -> Result<Self> {
        // Initialize llama.cpp backend
        let backend = LlamaBackend::init()?;

        Ok(Self {
            backend,
            model: None,
            model_path: models_dir.join("llm"),
            current_model: None,
            gpu_layers: 99, // Offload all layers to GPU by default
        })
    }

    /// Set number of GPU layers (0 = CPU only, 99+ = all layers on GPU)
    pub fn set_gpu_layers(&mut self, layers: u32) {
        self.gpu_layers = layers;
    }

    /// Check if a model is currently loaded
    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    /// Get the currently loaded model variant
    pub fn current_model(&self) -> Option<LlmModel> {
        self.current_model
    }

    /// Get the model directory path
    pub fn model_path(&self) -> &PathBuf {
        &self.model_path
    }

    /// Load a model from disk
    pub fn load_model(&mut self, model: LlmModel) -> Result<()> {
        let model_file = self.model_path.join(model.filename());

        if !model_file.exists() {
            return Err(anyhow!(
                "Model not found: {:?}. Please download it first.",
                model_file
            ));
        }

        info!("Loading LLM model: {} from {:?}", model, model_file);

        // Configure model parameters with GPU offloading
        let model_params = LlamaModelParams::default().with_n_gpu_layers(self.gpu_layers);

        // Load the model
        let loaded_model =
            LlamaModel::load_from_file(&self.backend, &model_file, &model_params).map_err(
                |e| {
                    anyhow!(
                        "Failed to load model {:?}: {}",
                        model_file,
                        format!("{:?}", e)
                    )
                },
            )?;

        info!(
            "LLM model loaded: {} (vocab: {}, params: {})",
            model,
            loaded_model.n_vocab(),
            loaded_model.n_params()
        );

        self.model = Some(loaded_model);
        self.current_model = Some(model);

        Ok(())
    }

    /// Unload the current model to free memory
    pub fn unload_model(&mut self) {
        if self.model.is_some() {
            info!("Unloading LLM model");
            self.model = None;
            self.current_model = None;
        }
    }

    /// Generate text completion (blocking)
    pub fn generate(&self, prompt: &str, config: &GenerationConfig) -> Result<String> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("No model loaded. Call load_model() first."))?;

        // Tokenize prompt first to determine batch size
        let tokens = model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| anyhow!("Failed to tokenize prompt: {:?}", e))?;

        debug!("Prompt tokens: {}", tokens.len());

        if tokens.len() > 3500 {
            warn!(
                "Prompt is very long ({} tokens), may truncate output",
                tokens.len()
            );
        }

        // Create context with batch size matching prompt length
        let batch_size = tokens.len().max(512) as u32;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(4096).unwrap()))
            .with_n_batch(batch_size);

        let mut ctx = model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| anyhow!("Failed to create context: {:?}", e))?;

        // Create batch with capacity for all prompt tokens
        let mut batch = LlamaBatch::new(tokens.len(), 1);

        for (i, token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch
                .add(*token, i as i32, &[0], is_last)
                .map_err(|e| anyhow!("Failed to add token to batch: {:?}", e))?;
        }

        // Process prompt (prefill)
        ctx.decode(&mut batch)
            .map_err(|e| anyhow!("Failed to decode prompt: {:?}", e))?;

        // Create sampler chain
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::top_k(config.top_k),
            LlamaSampler::top_p(config.top_p, 1),
            LlamaSampler::temp(config.temperature),
            LlamaSampler::dist(42), // Seed for reproducibility
        ]);

        // Generate tokens
        let mut output = String::new();
        let mut n_cur = tokens.len();

        for _ in 0..config.max_tokens {
            // Sample next token
            let new_token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(new_token);

            // Check for end-of-generation
            if model.is_eog_token(new_token) {
                break;
            }

            // Convert token to string
            if let Ok(token_str) = model.token_to_str(new_token, Special::Tokenize) {
                output.push_str(&token_str);

                // Check stop sequences
                if config
                    .stop_sequences
                    .iter()
                    .any(|s| output.contains(s.as_str()))
                {
                    // Remove the stop sequence from output
                    for stop in &config.stop_sequences {
                        if let Some(pos) = output.find(stop.as_str()) {
                            output.truncate(pos);
                            break;
                        }
                    }
                    break;
                }
            }

            // Prepare next batch
            batch.clear();
            batch
                .add(new_token, n_cur as i32, &[0], true)
                .map_err(|e| anyhow!("Failed to add generated token: {:?}", e))?;
            n_cur += 1;

            // Decode
            ctx.decode(&mut batch)
                .map_err(|e| anyhow!("Failed to decode generated token: {:?}", e))?;
        }

        Ok(output.trim().to_string())
    }

    /// Generate text with streaming via callback
    pub fn generate_stream<F>(&self, prompt: &str, config: &GenerationConfig, mut callback: F) -> Result<String>
    where
        F: FnMut(&str),
    {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("No model loaded"))?;

        // Tokenize prompt first to determine batch size
        let tokens = model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| anyhow!("Failed to tokenize: {:?}", e))?;

        // Create context with batch size matching prompt length
        let batch_size = tokens.len().max(512) as u32;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(NonZeroU32::new(4096).unwrap()))
            .with_n_batch(batch_size);

        let mut ctx = model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| anyhow!("Failed to create context: {:?}", e))?;

        // Create batch with capacity for all prompt tokens
        let mut batch = LlamaBatch::new(tokens.len(), 1);
        for (i, token) in tokens.iter().enumerate() {
            batch.add(*token, i as i32, &[0], i == tokens.len() - 1)?;
        }

        ctx.decode(&mut batch)?;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::top_k(config.top_k),
            LlamaSampler::top_p(config.top_p, 1),
            LlamaSampler::temp(config.temperature),
            LlamaSampler::dist(42),
        ]);

        let mut output = String::new();
        let mut n_cur = tokens.len();

        for _ in 0..config.max_tokens {
            let new_token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(new_token);

            if model.is_eog_token(new_token) {
                break;
            }

            if let Ok(token_str) = model.token_to_str(new_token, Special::Tokenize) {
                output.push_str(&token_str);
                callback(&token_str);

                if config.stop_sequences.iter().any(|s| output.contains(s.as_str())) {
                    for stop in &config.stop_sequences {
                        if let Some(pos) = output.find(stop.as_str()) {
                            output.truncate(pos);
                            break;
                        }
                    }
                    break;
                }
            }

            batch.clear();
            batch.add(new_token, n_cur as i32, &[0], true)?;
            n_cur += 1;
            ctx.decode(&mut batch)?;
        }

        Ok(output.trim().to_string())
    }

    /// Get token count for a text (for context length estimation)
    pub fn count_tokens(&self, text: &str) -> Result<usize> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("No model loaded"))?;

        let tokens = model
            .str_to_token(text, AddBos::Never)
            .map_err(|e| anyhow!("Failed to tokenize: {:?}", e))?;

        Ok(tokens.len())
    }
}

/// Prepare a transcript for LLM processing by truncating if needed
pub fn prepare_transcript_for_llm(transcript: &str, max_chars: usize) -> String {
    if transcript.len() <= max_chars {
        return transcript.to_string();
    }

    // Strategy: Keep first 60% and last 35% to capture context
    let keep_start = max_chars * 60 / 100;
    let keep_end = max_chars * 35 / 100;

    let start = &transcript[..keep_start];
    let end = &transcript[transcript.len() - keep_end..];

    format!(
        "{}\n\n[... {} characters omitted for length ...]\n\n{}",
        start,
        transcript.len() - keep_start - keep_end,
        end
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_config_defaults() {
        let config = GenerationConfig::default();
        assert_eq!(config.max_tokens, 2048);
        assert!(config.temperature > 0.0 && config.temperature <= 1.0);
    }

    #[test]
    fn test_summarization_config() {
        let config = GenerationConfig::for_summarization();
        assert!(config.temperature < 0.5);
        assert!(config.max_tokens > 1000);
    }

    #[test]
    fn test_chat_config() {
        let config = GenerationConfig::for_chat();
        assert!(config.temperature >= 0.5);
    }

    #[test]
    fn test_transcript_truncation() {
        let long_text = "word ".repeat(5000);
        let prepared = prepare_transcript_for_llm(&long_text, 10000);
        assert!(prepared.len() < long_text.len());
        assert!(prepared.contains("omitted"));
    }

    #[test]
    fn test_no_truncation_for_short_text() {
        let short_text = "This is a short text.";
        let prepared = prepare_transcript_for_llm(short_text, 10000);
        assert_eq!(prepared, short_text);
    }
}
