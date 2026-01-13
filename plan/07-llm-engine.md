# 07 - LLM Engine: Local Language Model Integration

## Goal
Integrate `llama-cpp-2` for local LLM inference to power meeting summarization, action item extraction, and RAG-based chat conversations. This enables fully offline, privacy-preserving AI features.

**Estimated Time:** 4-5 days

## Prerequisites
- Document 05 (Storage Layer) completed - SQLite schema ready
- Document 06 (Embedding Engine) completed - vector search available
- Basic understanding of GGUF model format and quantization

## Technology Overview

### Why llama-cpp-2?

[llama-cpp-2](https://github.com/edgenai/llama-cpp-rs) provides Rust bindings to [llama.cpp](https://github.com/ggerganov/llama.cpp), offering:

| Feature | Benefit |
|---------|---------|
| **GGUF format** | Single-file models, easy distribution |
| **Quantization** | 4-bit models run on consumer hardware |
| **GPU acceleration** | CUDA, Metal, Vulkan support |
| **Low memory** | 3B model uses ~3GB RAM (Q4) |
| **Active development** | Weekly updates, new model support |

### Model Recommendations

| Model | Size | Context | Speed | Best For |
|-------|------|---------|-------|----------|
| **Llama 3.2 3B Q4_K_M** | 2.0 GB | 8K | ~30 tok/s | Fast summaries, limited VRAM |
| **Qwen2.5 3B Q4_K_M** | 2.0 GB | 32K | ~28 tok/s | Long meetings, multilingual |
| **Mistral 7B Q4_K_M** | 4.1 GB | 8K | ~20 tok/s | Higher quality summaries |
| **Llama 3.1 8B Q4_K_M** | 4.7 GB | 8K | ~15 tok/s | Best quality (8GB+ VRAM) |

**Default Choice:** Llama 3.2 3B Q4_K_M
- Works on most hardware (4GB+ VRAM or 8GB+ RAM for CPU)
- Good balance of speed and quality
- Excellent instruction following

### Download Sources

```
# HuggingFace GGUF repositories
https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF
https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF
https://huggingface.co/TheBloke/Mistral-7B-Instruct-v0.2-GGUF
```

## Architecture

### Processing Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                         LLM Engine                                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │   Prompts    │    │   llama.cpp  │    │     Outputs          │  │
│  │              │───▶│   Backend    │───▶│                      │  │
│  │ - Summary    │    │              │    │ - Meeting Summary    │  │
│  │ - RAG Chat   │    │ - GPU/CPU    │    │ - Action Items       │  │
│  │ - Q&A        │    │ - Streaming  │    │ - Chat Response      │  │
│  └──────────────┘    └──────────────┘    └──────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Component Design

```
src-tauri/src/inference/
├── mod.rs
├── llm.rs              # LLM service
├── prompts.rs          # Prompt templates
├── summarization.rs    # Summary generation
└── chat.rs             # RAG chat logic
```

## Implementation

### Step 1: Add Dependencies

**File: `src-tauri/Cargo.toml`**

```toml
[dependencies]
# LLM inference via llama.cpp
llama-cpp-2 = "0.1"

# For streaming responses
futures = "0.3"
tokio-stream = "0.1"

# Template rendering (optional, for complex prompts)
minijinja = "2.0"
```

### Step 2: Model Configuration

**File: `src-tauri/src/inference/llm.rs`**

```rust
use anyhow::{anyhow, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Available LLM models
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LlmModel {
    Llama3_2_3B,
    Qwen2_5_3B,
    Mistral7B,
    Llama3_1_8B,
    Custom,
}

impl LlmModel {
    pub fn filename(&self) -> &str {
        match self {
            Self::Llama3_2_3B => "llama-3.2-3b-instruct-q4_k_m.gguf",
            Self::Qwen2_5_3B => "qwen2.5-3b-instruct-q4_k_m.gguf",
            Self::Mistral7B => "mistral-7b-instruct-v0.2-q4_k_m.gguf",
            Self::Llama3_1_8B => "llama-3.1-8b-instruct-q4_k_m.gguf",
            Self::Custom => "custom.gguf",
        }
    }
    
    pub fn download_url(&self) -> &str {
        match self {
            Self::Llama3_2_3B => "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
            Self::Qwen2_5_3B => "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf",
            Self::Mistral7B => "https://huggingface.co/TheBloke/Mistral-7B-Instruct-v0.2-GGUF/resolve/main/mistral-7b-instruct-v0.2.Q4_K_M.gguf",
            Self::Llama3_1_8B => "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
            Self::Custom => "",
        }
    }
    
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::Llama3_2_3B => 2_000_000_000,  // ~2 GB
            Self::Qwen2_5_3B => 2_000_000_000,
            Self::Mistral7B => 4_100_000_000,    // ~4.1 GB
            Self::Llama3_1_8B => 4_700_000_000,  // ~4.7 GB
            Self::Custom => 0,
        }
    }
    
    pub fn context_length(&self) -> u32 {
        match self {
            Self::Llama3_2_3B => 8192,
            Self::Qwen2_5_3B => 32768,
            Self::Mistral7B => 8192,
            Self::Llama3_1_8B => 8192,
            Self::Custom => 4096,
        }
    }
}

/// LLM generation configuration
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub repeat_penalty: f32,
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
    /// Config for structured outputs (summaries, action items)
    pub fn for_summarization() -> Self {
        Self {
            max_tokens: 1500,
            temperature: 0.3,  // Lower for consistency
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.15,
            stop_sequences: vec![],
        }
    }
    
    /// Config for conversational chat
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
}
```

### Step 3: LLM Service Implementation

**File: `src-tauri/src/inference/llm.rs` (continued)**

```rust
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
            gpu_layers: 99,  // Offload all layers to GPU by default
        })
    }
    
    /// Set number of GPU layers (0 = CPU only)
    pub fn set_gpu_layers(&mut self, layers: u32) {
        self.gpu_layers = layers;
    }
    
    /// Check if a model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }
    
    /// Get currently loaded model
    pub fn current_model(&self) -> Option<LlmModel> {
        self.current_model
    }
    
    /// Load a model from disk
    pub fn load_model(&mut self, model: LlmModel) -> Result<()> {
        let model_file = self.model_path.join(model.filename());
        
        if !model_file.exists() {
            return Err(anyhow!("Model not found: {:?}", model_file));
        }
        
        info!("Loading LLM model: {:?}", model);
        
        // Configure model parameters
        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(self.gpu_layers);
        
        // Load the model
        let loaded_model = LlamaModel::load_from_file(
            &self.backend,
            model_file,
            &model_params,
        )?;
        
        self.model = Some(loaded_model);
        self.current_model = Some(model);
        
        info!("LLM model loaded successfully");
        Ok(())
    }
    
    /// Unload current model to free memory
    pub fn unload_model(&mut self) {
        self.model = None;
        self.current_model = None;
        info!("LLM model unloaded");
    }
    
    /// Generate text completion (blocking)
    pub fn generate(&self, prompt: &str, config: &GenerationConfig) -> Result<String> {
        let model = self.model.as_ref()
            .ok_or_else(|| anyhow!("No model loaded"))?;
        
        // Create context for this generation
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(4096).unwrap())
            .with_n_batch(512);
        
        let mut ctx = model.new_context(&self.backend, ctx_params)?;
        
        // Tokenize prompt
        let tokens = model.str_to_token(prompt, AddBos::Always)?;
        debug!("Prompt tokens: {}", tokens.len());
        
        if tokens.len() > 3500 {
            warn!("Prompt is very long ({} tokens), may truncate output", tokens.len());
        }
        
        // Create batch and add tokens
        let mut batch = LlamaBatch::new(512, 1);
        
        for (i, token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch.add(*token, i as i32, &[0], is_last)?;
        }
        
        // Process prompt
        ctx.decode(&mut batch)?;
        
        // Generate tokens
        let mut output_tokens = Vec::new();
        let mut n_cur = tokens.len();
        
        while output_tokens.len() < config.max_tokens as usize {
            // Get logits for last token
            let logits = ctx.get_logits_ith((batch.n_tokens() - 1) as i32);
            
            // Sample next token
            let mut candidates = LlamaTokenDataArray::from_iter(
                logits.iter().enumerate().map(|(i, &logit)| {
                    llama_cpp_2::token::data::LlamaTokenData::new(
                        llama_cpp_2::token::LlamaToken::new(i as i32),
                        logit,
                        0.0,
                    )
                }),
                false,
            );
            
            // Apply sampling
            ctx.sample_temp(&mut candidates, config.temperature);
            ctx.sample_top_k(&mut candidates, config.top_k, 1);
            ctx.sample_top_p(&mut candidates, config.top_p, 1);
            ctx.sample_repetition_penalty(
                &mut candidates,
                &output_tokens,
                64,
                config.repeat_penalty,
                1.0,
                1.0,
            );
            
            let new_token = ctx.sample_token(&mut candidates);
            
            // Check for EOS
            if new_token == model.token_eos() {
                break;
            }
            
            output_tokens.push(new_token);
            
            // Check stop sequences
            let partial_output = model.token_to_str(new_token)?;
            let full_output: String = output_tokens.iter()
                .filter_map(|t| model.token_to_str(*t).ok())
                .collect();
            
            if config.stop_sequences.iter().any(|s| full_output.contains(s)) {
                break;
            }
            
            // Prepare next batch
            batch.clear();
            batch.add(new_token, n_cur as i32, &[0], true)?;
            n_cur += 1;
            
            ctx.decode(&mut batch)?;
        }
        
        // Decode output
        let output: String = output_tokens.iter()
            .filter_map(|t| model.token_to_str(*t).ok())
            .collect();
        
        Ok(output.trim().to_string())
    }
    
    /// Generate text with streaming (async)
    pub async fn generate_stream(
        &self,
        prompt: String,
        config: GenerationConfig,
    ) -> Result<mpsc::Receiver<String>> {
        let (tx, rx) = mpsc::channel(100);
        
        let model = self.model.as_ref()
            .ok_or_else(|| anyhow!("No model loaded"))?;
        
        // Clone what we need for the task
        let backend = self.backend.clone();
        let model = model.clone();
        
        tokio::task::spawn_blocking(move || {
            // Similar to generate(), but send tokens as they're generated
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(4096).unwrap())
                .with_n_batch(512);
            
            let mut ctx = match model.new_context(&backend, ctx_params) {
                Ok(ctx) => ctx,
                Err(e) => {
                    let _ = tx.blocking_send(format!("[Error: {}]", e));
                    return;
                }
            };
            
            let tokens = match model.str_to_token(&prompt, AddBos::Always) {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx.blocking_send(format!("[Error: {}]", e));
                    return;
                }
            };
            
            let mut batch = LlamaBatch::new(512, 1);
            
            for (i, token) in tokens.iter().enumerate() {
                let is_last = i == tokens.len() - 1;
                if batch.add(*token, i as i32, &[0], is_last).is_err() {
                    return;
                }
            }
            
            if ctx.decode(&mut batch).is_err() {
                return;
            }
            
            let mut output_tokens = Vec::new();
            let mut n_cur = tokens.len();
            let mut full_output = String::new();
            
            while output_tokens.len() < config.max_tokens as usize {
                let logits = ctx.get_logits_ith((batch.n_tokens() - 1) as i32);
                
                let mut candidates = LlamaTokenDataArray::from_iter(
                    logits.iter().enumerate().map(|(i, &logit)| {
                        llama_cpp_2::token::data::LlamaTokenData::new(
                            llama_cpp_2::token::LlamaToken::new(i as i32),
                            logit,
                            0.0,
                        )
                    }),
                    false,
                );
                
                ctx.sample_temp(&mut candidates, config.temperature);
                ctx.sample_top_k(&mut candidates, config.top_k, 1);
                ctx.sample_top_p(&mut candidates, config.top_p, 1);
                
                let new_token = ctx.sample_token(&mut candidates);
                
                if new_token == model.token_eos() {
                    break;
                }
                
                output_tokens.push(new_token);
                
                // Stream the token
                if let Ok(token_str) = model.token_to_str(new_token) {
                    full_output.push_str(&token_str);
                    if tx.blocking_send(token_str).is_err() {
                        break;  // Receiver dropped
                    }
                }
                
                // Check stop sequences
                if config.stop_sequences.iter().any(|s| full_output.contains(s)) {
                    break;
                }
                
                batch.clear();
                if batch.add(new_token, n_cur as i32, &[0], true).is_err() {
                    break;
                }
                n_cur += 1;
                
                if ctx.decode(&mut batch).is_err() {
                    break;
                }
            }
        });
        
        Ok(rx)
    }
}
```

### Step 4: Prompt Templates

**File: `src-tauri/src/inference/prompts.rs`**

```rust
//! Prompt templates for various LLM tasks

/// Format a summarization prompt
pub fn summary_prompt(transcript: &str) -> String {
    format!(r#"<|begin_of_text|><|start_header_id|>system<|end_header_id|>

You are a professional meeting assistant. Your task is to create clear, actionable meeting summaries.

Guidelines:
- Be concise but comprehensive
- Use bullet points for lists
- Identify speakers when relevant
- Extract concrete action items with owners
- Note any unresolved questions or concerns<|eot_id|><|start_header_id|>user<|end_header_id|>

Please summarize the following meeting transcript:

---
{transcript}
---

Provide:
1. **Summary** (2-3 paragraphs)
2. **Key Discussion Points** (bullet list)
3. **Decisions Made** (bullet list)
4. **Action Items** (with owner and deadline if mentioned)
5. **Open Questions** (any unresolved topics)<|eot_id|><|start_header_id|>assistant<|end_header_id|>

"#, transcript = transcript)
}

/// Format an action items extraction prompt
pub fn action_items_prompt(transcript: &str) -> String {
    format!(r#"<|begin_of_text|><|start_header_id|>system<|end_header_id|>

You are a meeting assistant focused on extracting action items.

For each action item, identify:
- What needs to be done
- Who is responsible (if mentioned)
- Deadline (if mentioned)
- Priority (High/Medium/Low based on context)<|eot_id|><|start_header_id|>user<|end_header_id|>

Extract all action items from this meeting transcript:

---
{transcript}
---

Format as JSON array:
```json
[
  {{
    "task": "Description of the task",
    "owner": "Person name or 'Unassigned'",
    "deadline": "Date or 'Not specified'",
    "priority": "High|Medium|Low"
  }}
]
```<|eot_id|><|start_header_id|>assistant<|end_header_id|>

```json
"#, transcript = transcript)
}

/// Format a RAG chat prompt with context
pub fn rag_chat_prompt(context: &str, question: &str, chat_history: &str) -> String {
    let history_section = if chat_history.is_empty() {
        String::new()
    } else {
        format!("\nPrevious conversation:\n{}\n", chat_history)
    };
    
    format!(r#"<|begin_of_text|><|start_header_id|>system<|end_header_id|>

You are a helpful assistant that answers questions about meetings using the provided context.

Rules:
- Only use information from the provided meeting excerpts
- If the answer isn't in the excerpts, say "I don't have that information in the meeting records"
- Cite which meeting or speaker the information comes from when relevant
- Be conversational but accurate<|eot_id|><|start_header_id|>user<|end_header_id|>
{history}
Relevant meeting excerpts:
---
{context}
---

Question: {question}<|eot_id|><|start_header_id|>assistant<|end_header_id|>

"#, history = history_section, context = context, question = question)
}

/// Format a quick question prompt (no RAG context)
pub fn quick_question_prompt(question: &str, transcript: &str) -> String {
    format!(r#"<|begin_of_text|><|start_header_id|>system<|end_header_id|>

You are a meeting assistant. Answer questions about the meeting transcript provided.<|eot_id|><|start_header_id|>user<|end_header_id|>

Meeting transcript:
---
{transcript}
---

Question: {question}<|eot_id|><|start_header_id|>assistant<|end_header_id|>

"#, transcript = transcript, question = question)
}

/// Format a title generation prompt
pub fn title_prompt(transcript_start: &str) -> String {
    format!(r#"<|begin_of_text|><|start_header_id|>system<|end_header_id|>

Generate a brief, descriptive title for a meeting based on its opening content. 
The title should be 3-8 words, no punctuation at the end.<|eot_id|><|start_header_id|>user<|end_header_id|>

Opening of meeting transcript:
---
{transcript}
---

Meeting title:<|eot_id|><|start_header_id|>assistant<|end_header_id|>

"#, transcript = transcript_start)
}

/// Prompt template for Qwen models (different format)
pub mod qwen {
    pub fn summary_prompt(transcript: &str) -> String {
        format!(r#"<|im_start|>system
You are a professional meeting assistant. Create clear, actionable meeting summaries.<|im_end|>
<|im_start|>user
Summarize this meeting transcript:

---
{transcript}
---

Provide:
1. **Summary** (2-3 paragraphs)
2. **Key Discussion Points**
3. **Decisions Made**
4. **Action Items** (with owner)
5. **Open Questions**<|im_end|>
<|im_start|>assistant
"#, transcript = transcript)
    }
}

/// Prompt template for Mistral models
pub mod mistral {
    pub fn summary_prompt(transcript: &str) -> String {
        format!(r#"[INST] You are a professional meeting assistant. Summarize the following meeting transcript.

Transcript:
---
{transcript}
---

Provide:
1. Summary (2-3 paragraphs)
2. Key Discussion Points (bullet list)
3. Decisions Made (bullet list)
4. Action Items (with owner and deadline)
5. Open Questions [/INST]

"#, transcript = transcript)
    }
}
```

### Step 5: Summarization Service

**File: `src-tauri/src/inference/summarization.rs`**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::llm::{GenerationConfig, LlmModel, LlmService};
use super::prompts;

/// Types of summaries that can be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SummaryType {
    Full,           // Complete summary with all sections
    Brief,          // 2-3 sentence overview
    ActionItems,    // Just action items
    KeyPoints,      // Just key discussion points
}

/// Structured summary output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingSummary {
    pub summary: String,
    pub key_points: Vec<String>,
    pub decisions: Vec<String>,
    pub action_items: Vec<ActionItem>,
    pub open_questions: Vec<String>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Action item extracted from meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub task: String,
    pub owner: Option<String>,
    pub deadline: Option<String>,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

/// Summarization service
pub struct SummarizationService<'a> {
    llm: &'a LlmService,
}

impl<'a> SummarizationService<'a> {
    pub fn new(llm: &'a LlmService) -> Self {
        Self { llm }
    }
    
    /// Generate a full meeting summary
    pub fn summarize(&self, transcript: &str) -> Result<String> {
        info!("Generating meeting summary for {} chars", transcript.len());
        
        // Truncate if too long (leave room for prompt and output)
        let max_transcript_chars = 12000;  // ~3000 tokens
        let truncated = if transcript.len() > max_transcript_chars {
            let truncated = &transcript[..max_transcript_chars];
            format!("{}...\n\n[Transcript truncated for length]", truncated)
        } else {
            transcript.to_string()
        };
        
        // Select prompt based on model
        let prompt = match self.llm.current_model() {
            Some(LlmModel::Qwen2_5_3B) => prompts::qwen::summary_prompt(&truncated),
            Some(LlmModel::Mistral7B) => prompts::mistral::summary_prompt(&truncated),
            _ => prompts::summary_prompt(&truncated),
        };
        
        let config = GenerationConfig::for_summarization();
        self.llm.generate(&prompt, &config)
    }
    
    /// Extract action items as structured data
    pub fn extract_action_items(&self, transcript: &str) -> Result<Vec<ActionItem>> {
        let max_chars = 12000;
        let truncated = if transcript.len() > max_chars {
            &transcript[..max_chars]
        } else {
            transcript
        };
        
        let prompt = prompts::action_items_prompt(truncated);
        let config = GenerationConfig {
            max_tokens: 1000,
            temperature: 0.2,  // Very low for JSON
            stop_sequences: vec!["```".to_string()],
            ..GenerationConfig::default()
        };
        
        let output = self.llm.generate(&prompt, &config)?;
        
        // Parse JSON output
        let items: Vec<ActionItemRaw> = serde_json::from_str(&output)
            .unwrap_or_default();
        
        Ok(items.into_iter().map(|raw| ActionItem {
            task: raw.task,
            owner: if raw.owner == "Unassigned" { None } else { Some(raw.owner) },
            deadline: if raw.deadline == "Not specified" { None } else { Some(raw.deadline) },
            priority: match raw.priority.to_lowercase().as_str() {
                "high" => Priority::High,
                "low" => Priority::Low,
                _ => Priority::Medium,
            },
        }).collect())
    }
    
    /// Generate a meeting title
    pub fn generate_title(&self, transcript_start: &str) -> Result<String> {
        let max_chars = 2000;
        let truncated = if transcript_start.len() > max_chars {
            &transcript_start[..max_chars]
        } else {
            transcript_start
        };
        
        let prompt = prompts::title_prompt(truncated);
        let config = GenerationConfig {
            max_tokens: 20,
            temperature: 0.5,
            ..GenerationConfig::default()
        };
        
        let title = self.llm.generate(&prompt, &config)?;
        
        // Clean up title
        let title = title
            .lines()
            .next()
            .unwrap_or(&title)
            .trim()
            .trim_matches('"')
            .to_string();
        
        Ok(title)
    }
}

#[derive(Deserialize)]
struct ActionItemRaw {
    task: String,
    owner: String,
    deadline: String,
    priority: String,
}
```

### Step 6: Model Download Integration

**File: `src-tauri/src/models/llm_downloader.rs`**

```rust
use anyhow::Result;
use reqwest::Client;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::info;

use crate::inference::llm::LlmModel;

/// Download an LLM model with progress tracking
pub async fn download_llm_model<F>(
    model: LlmModel,
    dest_dir: PathBuf,
    progress_callback: F,
) -> Result<PathBuf>
where
    F: Fn(f32, u64, u64) + Send + 'static,
{
    let url = model.download_url();
    if url.is_empty() {
        anyhow::bail!("No download URL for custom models");
    }
    
    let filename = model.filename();
    let dest_path = dest_dir.join(filename);
    
    // Create directory if needed
    tokio::fs::create_dir_all(&dest_dir).await?;
    
    // Check if already downloaded
    if dest_path.exists() {
        let metadata = tokio::fs::metadata(&dest_path).await?;
        if metadata.len() > model.size_bytes() / 2 {  // At least half size = probably complete
            info!("Model already exists: {:?}", dest_path);
            return Ok(dest_path);
        }
    }
    
    info!("Downloading LLM model from {}", url);
    
    let client = Client::new();
    let response = client.get(url)
        .send()
        .await?
        .error_for_status()?;
    
    let total_size = response.content_length().unwrap_or(model.size_bytes());
    
    let mut file = File::create(&dest_path).await?;
    let mut downloaded: u64 = 0;
    
    let mut stream = response.bytes_stream();
    use futures::StreamExt;
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        
        let progress = downloaded as f32 / total_size as f32;
        progress_callback(progress, downloaded, total_size);
    }
    
    file.flush().await?;
    
    info!("Download complete: {:?}", dest_path);
    Ok(dest_path)
}

/// Check if a model is downloaded
pub async fn is_model_downloaded(model: LlmModel, models_dir: &PathBuf) -> bool {
    let path = models_dir.join("llm").join(model.filename());
    if let Ok(metadata) = tokio::fs::metadata(&path).await {
        metadata.len() > model.size_bytes() / 2
    } else {
        false
    }
}

/// Get disk space used by LLM models
pub async fn get_llm_models_size(models_dir: &PathBuf) -> Result<u64> {
    let llm_dir = models_dir.join("llm");
    if !llm_dir.exists() {
        return Ok(0);
    }
    
    let mut total = 0u64;
    let mut entries = tokio::fs::read_dir(&llm_dir).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        if let Ok(metadata) = entry.metadata().await {
            total += metadata.len();
        }
    }
    
    Ok(total)
}
```

### Step 7: Tauri Commands

**File: `src-tauri/src/commands/llm.rs`**

```rust
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::{command, State, Window};
use tracing::info;

use crate::inference::llm::{GenerationConfig, LlmModel, LlmService};
use crate::inference::summarization::{ActionItem, SummarizationService};
use crate::models::llm_downloader;
use crate::AppState;

/// Load an LLM model
#[command]
pub async fn load_llm_model(
    model: LlmModel,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut state = state.lock();
    state.llm_service.load_model(model)
        .map_err(|e| e.to_string())
}

/// Unload current LLM model
#[command]
pub async fn unload_llm_model(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut state = state.lock();
    state.llm_service.unload_model();
    Ok(())
}

/// Get current LLM status
#[command]
pub async fn get_llm_status(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<LlmStatus, String> {
    let state = state.lock();
    Ok(LlmStatus {
        loaded: state.llm_service.is_loaded(),
        current_model: state.llm_service.current_model(),
    })
}

#[derive(serde::Serialize)]
pub struct LlmStatus {
    loaded: bool,
    current_model: Option<LlmModel>,
}

/// Generate meeting summary
#[command]
pub async fn generate_summary(
    meeting_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    // Get transcript from database
    let transcript = {
        let state = state.lock();
        state.transcript_repo.get_full_transcript(&meeting_id)
            .map_err(|e| e.to_string())?
    };
    
    // Generate summary
    let summary = {
        let state = state.lock();
        let service = SummarizationService::new(&state.llm_service);
        service.summarize(&transcript)
            .map_err(|e| e.to_string())?
    };
    
    // Store summary
    {
        let state = state.lock();
        state.meeting_repo.save_summary(&meeting_id, &summary, "full")
            .map_err(|e| e.to_string())?;
    }
    
    Ok(summary)
}

/// Extract action items from meeting
#[command]
pub async fn extract_action_items(
    meeting_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<ActionItem>, String> {
    let transcript = {
        let state = state.lock();
        state.transcript_repo.get_full_transcript(&meeting_id)
            .map_err(|e| e.to_string())?
    };
    
    let items = {
        let state = state.lock();
        let service = SummarizationService::new(&state.llm_service);
        service.extract_action_items(&transcript)
            .map_err(|e| e.to_string())?
    };
    
    Ok(items)
}

/// Generate meeting title
#[command]
pub async fn generate_meeting_title(
    meeting_id: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let transcript_start = {
        let state = state.lock();
        state.transcript_repo.get_transcript_start(&meeting_id, 2000)
            .map_err(|e| e.to_string())?
    };
    
    let title = {
        let state = state.lock();
        let service = SummarizationService::new(&state.llm_service);
        service.generate_title(&transcript_start)
            .map_err(|e| e.to_string())?
    };
    
    // Update meeting title
    {
        let state = state.lock();
        state.meeting_repo.update_title(&meeting_id, &title)
            .map_err(|e| e.to_string())?;
    }
    
    Ok(title)
}

/// Download LLM model with progress
#[command]
pub async fn download_llm(
    model: LlmModel,
    window: Window,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let models_dir = {
        let state = state.lock();
        state.config.models_dir.clone()
    };
    
    llm_downloader::download_llm_model(
        model,
        models_dir.join("llm"),
        move |progress, downloaded, total| {
            let _ = window.emit("llm-download-progress", DownloadProgress {
                model,
                progress,
                downloaded,
                total,
            });
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    model: LlmModel,
    progress: f32,
    downloaded: u64,
    total: u64,
}

/// List available LLM models and their status
#[command]
pub async fn list_llm_models(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<LlmModelInfo>, String> {
    let models_dir = {
        let state = state.lock();
        state.config.models_dir.clone()
    };
    
    let models = vec![
        LlmModel::Llama3_2_3B,
        LlmModel::Qwen2_5_3B,
        LlmModel::Mistral7B,
        LlmModel::Llama3_1_8B,
    ];
    
    let mut result = Vec::new();
    for model in models {
        let downloaded = llm_downloader::is_model_downloaded(model, &models_dir).await;
        result.push(LlmModelInfo {
            model,
            name: format!("{:?}", model),
            size_bytes: model.size_bytes(),
            context_length: model.context_length(),
            downloaded,
        });
    }
    
    Ok(result)
}

#[derive(serde::Serialize)]
pub struct LlmModelInfo {
    model: LlmModel,
    name: String,
    size_bytes: u64,
    context_length: u32,
    downloaded: bool,
}

/// Generate text with streaming (for chat)
#[command]
pub async fn generate_stream(
    prompt: String,
    window: Window,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let mut rx = {
        let state = state.lock();
        state.llm_service.generate_stream(
            prompt,
            GenerationConfig::for_chat(),
        )
        .await
        .map_err(|e| e.to_string())?
    };
    
    let mut full_response = String::new();
    
    while let Some(token) = rx.recv().await {
        full_response.push_str(&token);
        let _ = window.emit("llm-token", &token);
    }
    
    let _ = window.emit("llm-complete", &full_response);
    
    Ok(full_response)
}
```

### Step 8: Frontend Integration

**File: `src/hooks/useLlm.ts`**

```typescript
import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type LlmModel = 'Llama3_2_3B' | 'Qwen2_5_3B' | 'Mistral7B' | 'Llama3_1_8B';

export interface LlmModelInfo {
  model: LlmModel;
  name: string;
  size_bytes: number;
  context_length: number;
  downloaded: boolean;
}

export interface LlmStatus {
  loaded: boolean;
  current_model: LlmModel | null;
}

export interface ActionItem {
  task: string;
  owner: string | null;
  deadline: string | null;
  priority: 'High' | 'Medium' | 'Low';
}

interface DownloadProgress {
  model: LlmModel;
  progress: number;
  downloaded: number;
  total: number;
}

export function useLlm() {
  const [status, setStatus] = useState<LlmStatus>({ loaded: false, current_model: null });
  const [models, setModels] = useState<LlmModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);

  // Load status on mount
  useEffect(() => {
    refreshStatus();
    loadModels();
    
    const unlisten = listen<DownloadProgress>('llm-download-progress', (event) => {
      setDownloadProgress(event.payload);
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const newStatus = await invoke<LlmStatus>('get_llm_status');
      setStatus(newStatus);
    } catch (err) {
      console.error('Failed to get LLM status:', err);
    }
  }, []);

  const loadModels = useCallback(async () => {
    try {
      const modelList = await invoke<LlmModelInfo[]>('list_llm_models');
      setModels(modelList);
    } catch (err) {
      console.error('Failed to list models:', err);
    }
  }, []);

  const loadModel = useCallback(async (model: LlmModel) => {
    setLoading(true);
    setError(null);
    try {
      await invoke('load_llm_model', { model });
      await refreshStatus();
    } catch (err) {
      setError(String(err));
      throw err;
    } finally {
      setLoading(false);
    }
  }, [refreshStatus]);

  const unloadModel = useCallback(async () => {
    setLoading(true);
    try {
      await invoke('unload_llm_model');
      await refreshStatus();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [refreshStatus]);

  const downloadModel = useCallback(async (model: LlmModel) => {
    setDownloadProgress({ model, progress: 0, downloaded: 0, total: 0 });
    try {
      await invoke('download_llm', { model });
      await loadModels();
    } catch (err) {
      setError(String(err));
      throw err;
    } finally {
      setDownloadProgress(null);
    }
  }, [loadModels]);

  return {
    status,
    models,
    loading,
    error,
    downloadProgress,
    loadModel,
    unloadModel,
    downloadModel,
    refreshStatus,
    loadModels,
  };
}

export function useSummarization() {
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const generateSummary = useCallback(async (meetingId: string): Promise<string> => {
    setGenerating(true);
    setError(null);
    try {
      const summary = await invoke<string>('generate_summary', { meetingId });
      return summary;
    } catch (err) {
      const message = String(err);
      setError(message);
      throw new Error(message);
    } finally {
      setGenerating(false);
    }
  }, []);

  const extractActionItems = useCallback(async (meetingId: string): Promise<ActionItem[]> => {
    setGenerating(true);
    setError(null);
    try {
      const items = await invoke<ActionItem[]>('extract_action_items', { meetingId });
      return items;
    } catch (err) {
      const message = String(err);
      setError(message);
      throw new Error(message);
    } finally {
      setGenerating(false);
    }
  }, []);

  const generateTitle = useCallback(async (meetingId: string): Promise<string> => {
    try {
      return await invoke<string>('generate_meeting_title', { meetingId });
    } catch (err) {
      setError(String(err));
      throw err;
    }
  }, []);

  return {
    generating,
    error,
    generateSummary,
    extractActionItems,
    generateTitle,
  };
}
```

**File: `src/components/Summary/SummaryPanel.tsx`**

```tsx
import React, { useState } from 'react';
import { useSummarization, ActionItem } from '../../hooks/useLlm';

interface SummaryPanelProps {
  meetingId: string;
  existingSummary?: string;
}

export const SummaryPanel: React.FC<SummaryPanelProps> = ({ 
  meetingId, 
  existingSummary 
}) => {
  const { generating, error, generateSummary, extractActionItems } = useSummarization();
  const [summary, setSummary] = useState(existingSummary || '');
  const [actionItems, setActionItems] = useState<ActionItem[]>([]);
  const [activeTab, setActiveTab] = useState<'summary' | 'actions'>('summary');

  const handleGenerateSummary = async () => {
    try {
      const result = await generateSummary(meetingId);
      setSummary(result);
    } catch (err) {
      console.error('Failed to generate summary:', err);
    }
  };

  const handleExtractActions = async () => {
    try {
      const items = await extractActionItems(meetingId);
      setActionItems(items);
      setActiveTab('actions');
    } catch (err) {
      console.error('Failed to extract actions:', err);
    }
  };

  return (
    <div className="summary-panel">
      <div className="tabs">
        <button 
          className={`tab ${activeTab === 'summary' ? 'active' : ''}`}
          onClick={() => setActiveTab('summary')}
        >
          Summary
        </button>
        <button 
          className={`tab ${activeTab === 'actions' ? 'active' : ''}`}
          onClick={() => setActiveTab('actions')}
        >
          Action Items ({actionItems.length})
        </button>
      </div>

      {activeTab === 'summary' && (
        <div className="summary-content">
          {summary ? (
            <div className="summary-text">
              <pre style={{ whiteSpace: 'pre-wrap', fontFamily: 'inherit' }}>
                {summary}
              </pre>
            </div>
          ) : (
            <div className="empty-state">
              <p>No summary generated yet.</p>
            </div>
          )}
          
          <button 
            className="generate-btn"
            onClick={handleGenerateSummary}
            disabled={generating}
          >
            {generating ? (
              <>
                <span className="spinner" />
                Generating...
              </>
            ) : summary ? (
              'Regenerate Summary'
            ) : (
              'Generate Summary'
            )}
          </button>
        </div>
      )}

      {activeTab === 'actions' && (
        <div className="actions-content">
          {actionItems.length > 0 ? (
            <ul className="action-list">
              {actionItems.map((item, index) => (
                <li key={index} className={`action-item priority-${item.priority.toLowerCase()}`}>
                  <div className="action-header">
                    <span className={`priority-badge ${item.priority.toLowerCase()}`}>
                      {item.priority}
                    </span>
                    {item.owner && (
                      <span className="owner">@{item.owner}</span>
                    )}
                  </div>
                  <p className="task">{item.task}</p>
                  {item.deadline && (
                    <span className="deadline">Due: {item.deadline}</span>
                  )}
                </li>
              ))}
            </ul>
          ) : (
            <div className="empty-state">
              <p>No action items extracted yet.</p>
            </div>
          )}
          
          <button 
            className="generate-btn"
            onClick={handleExtractActions}
            disabled={generating}
          >
            {generating ? 'Extracting...' : 'Extract Action Items'}
          </button>
        </div>
      )}

      {error && (
        <div className="error-message">
          {error}
        </div>
      )}
    </div>
  );
};
```

## Performance Optimization

### GPU Layer Configuration

```rust
/// Auto-detect optimal GPU layers based on available VRAM
pub fn detect_optimal_gpu_layers(model: LlmModel) -> u32 {
    // Try to detect VRAM (platform-specific)
    #[cfg(target_os = "windows")]
    {
        // Query DirectX for VRAM
        if let Some(vram_mb) = detect_vram_windows() {
            return calculate_layers(model, vram_mb);
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        // Apple Silicon unified memory
        return 99;  // Offload all layers
    }
    
    #[cfg(target_os = "linux")]
    {
        if let Some(vram_mb) = detect_vram_linux() {
            return calculate_layers(model, vram_mb);
        }
    }
    
    // Default: use 32 layers if can't detect
    32
}

fn calculate_layers(model: LlmModel, vram_mb: u64) -> u32 {
    // Approximate VRAM per layer
    let vram_per_layer = match model {
        LlmModel::Llama3_2_3B | LlmModel::Qwen2_5_3B => 60,  // ~60MB per layer
        LlmModel::Mistral7B => 100,  // ~100MB per layer
        LlmModel::Llama3_1_8B => 120,
        LlmModel::Custom => 80,
    };
    
    let available_vram = vram_mb.saturating_sub(500);  // Reserve 500MB
    let max_layers = (available_vram / vram_per_layer) as u32;
    
    max_layers.min(99)  // Cap at 99
}
```

### Context Management

```rust
/// Manage context window efficiently for long transcripts
pub fn prepare_transcript_for_llm(
    transcript: &str,
    max_tokens: usize,
) -> String {
    // Rough estimate: 4 chars per token
    let max_chars = max_tokens * 4;
    
    if transcript.len() <= max_chars {
        return transcript.to_string();
    }
    
    // Strategy: Keep first and last portions
    let keep_start = max_chars * 60 / 100;  // 60% from start
    let keep_end = max_chars * 35 / 100;    // 35% from end
    
    let start = &transcript[..keep_start];
    let end = &transcript[transcript.len() - keep_end..];
    
    format!(
        "{}\n\n[... {} characters omitted for length ...]\n\n{}",
        start,
        transcript.len() - keep_start - keep_end,
        end
    )
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generation_config_defaults() {
        let config = GenerationConfig::default();
        assert_eq!(config.max_tokens, 2048);
        assert!(config.temperature > 0.0 && config.temperature < 1.0);
    }
    
    #[test]
    fn test_summarization_config() {
        let config = GenerationConfig::for_summarization();
        assert!(config.temperature < 0.5);  // Lower for consistency
    }
    
    #[test]
    fn test_transcript_truncation() {
        let long_transcript = "word ".repeat(5000);
        let prepared = prepare_transcript_for_llm(&long_transcript, 2000);
        assert!(prepared.len() < long_transcript.len());
        assert!(prepared.contains("omitted"));
    }
    
    #[test]
    fn test_prompt_generation() {
        let prompt = prompts::summary_prompt("Test transcript");
        assert!(prompt.contains("Test transcript"));
        assert!(prompt.contains("Summary"));
    }
}
```

### Integration Test

```rust
#[tokio::test]
#[ignore]  // Requires model to be downloaded
async fn test_full_summarization_pipeline() {
    let models_dir = PathBuf::from("./test_models");
    let mut llm = LlmService::new(models_dir).unwrap();
    
    // Load model
    llm.load_model(LlmModel::Llama3_2_3B).unwrap();
    
    let transcript = r#"
        [You]: Let's discuss the Q4 roadmap.
        [Others]: Sounds good. I think we should focus on the mobile app.
        [You]: Agreed. Can you take the lead on the design?
        [Others]: Yes, I'll have mockups ready by next Friday.
    "#;
    
    let service = SummarizationService::new(&llm);
    let summary = service.summarize(transcript).unwrap();
    
    assert!(!summary.is_empty());
    assert!(summary.len() > 100);
}
```

## Troubleshooting

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "Model not found" | Model not downloaded | Use `download_llm` command first |
| "Out of memory" | Insufficient VRAM/RAM | Reduce `gpu_layers` or use smaller model |
| Slow generation | CPU-only mode | Enable GPU acceleration |
| Garbled output | Wrong prompt format | Use correct template for model |
| Generation hangs | Context too long | Truncate input or increase timeout |

### Debug Logging

```rust
// Enable detailed llama.cpp logging
std::env::set_var("LLAMA_LOG_LEVEL", "debug");

// Or in Tauri config:
// RUST_LOG=llama_cpp_2=debug
```

## Acceptance Criteria

- [ ] Model download with progress indicator
- [ ] GPU acceleration working on Windows/macOS/Linux
- [ ] Summary generation completes in <30s for 1-hour meeting
- [ ] Action items extracted as structured JSON
- [ ] Streaming output for chat responses
- [ ] Memory usage stable during long sessions
- [ ] Graceful fallback when GPU unavailable

## References

- [llama-cpp-2 Documentation](https://github.com/edgenai/llama-cpp-rs)
- [llama.cpp](https://github.com/ggerganov/llama.cpp)
- [GGUF Format Specification](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [HuggingFace GGUF Models](https://huggingface.co/models?library=gguf)
- [Llama 3.2 Model Card](https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct)
- [Qwen2.5 Technical Report](https://qwenlm.github.io/blog/qwen2.5/)

## Next Steps

After completing the LLM engine:
1. **Document 08**: Frontend UI - Complete React component library
2. **Document 09**: RAG Implementation - Combine embeddings + LLM for chat
