//! LLM-related Tauri commands
//!
//! Commands for LLM model management, summarization, and text generation.

use crate::inference::llm::{GenerationConfig, LlmService};
use crate::inference::summarization::{ActionItem, SummarizationService};
use crate::models::{delete_llm_model, download_llm_model, is_llm_downloaded, LlmModel};
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::info;

use super::SharedStorageState;

/// Shared LLM service state type
pub type SharedLlmService = Arc<Mutex<LlmService>>;

/// LLM status information
#[derive(Debug, Clone, Serialize)]
pub struct LlmStatus {
    /// Whether a model is currently loaded
    pub loaded: bool,
    /// The currently loaded model (if any)
    pub current_model: Option<LlmModel>,
}

/// LLM model information with status
#[derive(Debug, Clone, Serialize)]
pub struct LlmModelInfo {
    /// Model variant
    pub model: LlmModel,
    /// Human-readable name
    pub name: String,
    /// Model size in bytes
    pub size_bytes: u64,
    /// Formatted size string
    pub size_formatted: String,
    /// Context length in tokens
    pub context_length: u32,
    /// Whether the model is downloaded
    pub downloaded: bool,
}

/// Initialize the LLM service (downloads model if needed)
#[tauri::command]
pub async fn initialize_llm(
    app: AppHandle,
    config: tauri::State<'_, crate::AppConfig>,
    llm: tauri::State<'_, SharedLlmService>,
    model: Option<LlmModel>,
) -> Result<bool, String> {
    let model = model.unwrap_or_default();

    // Check if already loaded with the same model
    {
        let service = llm.lock();
        if service.is_loaded() && service.current_model() == Some(model) {
            info!("LLM model {} already loaded", model);
            return Ok(true);
        }
    }

    info!("Initializing LLM with model: {}", model);

    let models_dir = config.models_dir.join("llm");

    // Download model if needed
    if !is_llm_downloaded(model, &config.models_dir) {
        info!("Downloading LLM model: {}", model);

        let app_handle = app.clone();
        download_llm_model(model, models_dir.clone(), move |progress| {
            let _ = app_handle.emit("llm-download-progress", &progress);
        })
        .await
        .map_err(|e| format!("Failed to download LLM model: {}", e))?;
    }

    // Load the model
    info!("Loading LLM model: {}", model);
    {
        let mut service = llm.lock();
        service
            .load_model(model)
            .map_err(|e| format!("Failed to load LLM model: {}", e))?;
    }

    info!("LLM model {} loaded successfully", model);
    Ok(true)
}

/// Load an LLM model (must be downloaded first)
#[tauri::command]
pub fn load_llm_model(
    llm: tauri::State<'_, SharedLlmService>,
    model: LlmModel,
) -> Result<(), String> {
    let mut service = llm.lock();
    service
        .load_model(model)
        .map_err(|e| format!("Failed to load model: {}", e))
}

/// Unload the current LLM model to free memory
#[tauri::command]
pub fn unload_llm_model(llm: tauri::State<'_, SharedLlmService>) -> Result<(), String> {
    let mut service = llm.lock();
    service.unload_model();
    Ok(())
}

/// Get current LLM status
#[tauri::command]
pub fn get_llm_status(llm: tauri::State<'_, SharedLlmService>) -> Result<LlmStatus, String> {
    let service = llm.lock();
    Ok(LlmStatus {
        loaded: service.is_loaded(),
        current_model: service.current_model(),
    })
}

/// Check if an LLM model is downloaded
#[tauri::command]
pub fn is_llm_model_downloaded(
    config: tauri::State<'_, crate::AppConfig>,
    model: LlmModel,
) -> Result<bool, String> {
    Ok(is_llm_downloaded(model, &config.models_dir))
}

/// Download an LLM model
#[tauri::command]
pub async fn download_llm(
    app: AppHandle,
    config: tauri::State<'_, crate::AppConfig>,
    model: LlmModel,
) -> Result<(), String> {
    let models_dir = config.models_dir.join("llm");

    let app_handle = app.clone();
    download_llm_model(model, models_dir, move |progress| {
        let _ = app_handle.emit("llm-download-progress", &progress);
    })
    .await
    .map_err(|e| format!("Failed to download model: {}", e))?;

    Ok(())
}

/// Delete an LLM model (auto-unloads if currently loaded)
#[tauri::command]
pub async fn delete_llm(
    config: tauri::State<'_, crate::AppConfig>,
    llm: tauri::State<'_, SharedLlmService>,
    model: LlmModel,
) -> Result<(), String> {
    // Auto-unload if this model is currently loaded
    {
        let mut service = llm.lock();
        if service.is_loaded() && service.current_model() == Some(model) {
            info!("Unloading model before deletion: {}", model);
            service.unload_model();
        }
    }

    // Delete the model files
    info!("Deleting LLM model: {}", model);
    delete_llm_model(model, &config.models_dir)
        .await
        .map_err(|e| format!("Failed to delete model: {}", e))?;

    info!("LLM model {} deleted successfully", model);
    Ok(())
}

/// List available LLM models with their status
#[tauri::command]
pub fn list_llm_models(
    config: tauri::State<'_, crate::AppConfig>,
) -> Result<Vec<LlmModelInfo>, String> {
    let models: Vec<LlmModelInfo> = LlmModel::all()
        .iter()
        .map(|model| {
            let downloaded = is_llm_downloaded(*model, &config.models_dir);
            LlmModelInfo {
                model: *model,
                name: model.to_string(),
                size_bytes: model.size_bytes(),
                size_formatted: model.size_formatted(),
                context_length: model.context_length(),
                downloaded,
            }
        })
        .collect();

    Ok(models)
}

/// Generate a meeting summary
#[tauri::command]
pub fn generate_summary(
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<String, String> {
    // Get transcript from storage
    let transcript = {
        let storage = storage.lock();
        let repos = storage.repositories();
        repos
            .transcripts
            .get_full_text(&meeting_id)
            .map_err(|e| format!("Failed to get transcript: {}", e))?
    };

    if transcript.is_empty() {
        return Err("No transcript available for this meeting".to_string());
    }

    // Generate summary
    let service = llm.lock();
    let summarizer = SummarizationService::new(&service);
    summarizer
        .summarize(&transcript)
        .map_err(|e| format!("Failed to generate summary: {}", e))
}

/// Extract action items from a meeting
#[tauri::command]
pub fn extract_action_items(
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<Vec<ActionItem>, String> {
    let transcript = {
        let storage = storage.lock();
        let repos = storage.repositories();
        repos
            .transcripts
            .get_full_text(&meeting_id)
            .map_err(|e| format!("Failed to get transcript: {}", e))?
    };

    if transcript.is_empty() {
        return Ok(vec![]);
    }

    let service = llm.lock();
    let summarizer = SummarizationService::new(&service);
    summarizer
        .extract_action_items(&transcript)
        .map_err(|e| format!("Failed to extract action items: {}", e))
}

/// Generate a meeting title
#[tauri::command]
pub fn generate_meeting_title(
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<String, String> {
    // Get first part of transcript
    let transcript_start = {
        let storage = storage.lock();
        let repos = storage.repositories();
        let segments = repos
            .transcripts
            .get_by_meeting(&meeting_id)
            .map_err(|e| format!("Failed to get transcript: {}", e))?;

        // Get first ~2000 chars
        let mut text = String::new();
        for seg in segments {
            text.push_str(&seg.text);
            text.push(' ');
            if text.len() > 2000 {
                break;
            }
        }
        text
    };

    if transcript_start.is_empty() {
        return Err("No transcript available for title generation".to_string());
    }

    let service = llm.lock();
    let summarizer = SummarizationService::new(&service);
    let title = summarizer
        .generate_title(&transcript_start)
        .map_err(|e| format!("Failed to generate title: {}", e))?;

    // Update meeting title in storage
    {
        let storage = storage.lock();
        let repos = storage.repositories();
        let mut meeting = repos
            .meetings
            .get(&meeting_id)
            .map_err(|e| format!("Failed to get meeting: {}", e))?
            .ok_or_else(|| format!("Meeting not found: {}", meeting_id))?;
        meeting.title = title.clone();
        meeting.updated_at = chrono::Utc::now().timestamp_millis();
        repos
            .meetings
            .update(&meeting)
            .map_err(|e| format!("Failed to update meeting title: {}", e))?;
    }

    Ok(title)
}

/// Answer a question about a meeting
#[tauri::command]
pub fn ask_meeting_question(
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    question: String,
) -> Result<String, String> {
    let transcript = {
        let storage = storage.lock();
        let repos = storage.repositories();
        repos
            .transcripts
            .get_full_text(&meeting_id)
            .map_err(|e| format!("Failed to get transcript: {}", e))?
    };

    if transcript.is_empty() {
        return Err("No transcript available".to_string());
    }

    let service = llm.lock();
    let summarizer = SummarizationService::new(&service);
    summarizer
        .answer_question(&question, &transcript)
        .map_err(|e| format!("Failed to answer question: {}", e))
}

/// Generate raw text (for testing/debugging)
#[tauri::command]
pub fn generate_text(
    llm: tauri::State<'_, SharedLlmService>,
    prompt: String,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let config = GenerationConfig {
        max_tokens: max_tokens.unwrap_or(256),
        ..GenerationConfig::for_chat()
    };

    let service = llm.lock();
    service
        .generate(&prompt, &config)
        .map_err(|e| format!("Failed to generate text: {}", e))
}

/// Get estimated token count for text
#[tauri::command]
pub fn count_tokens(
    llm: tauri::State<'_, SharedLlmService>,
    text: String,
) -> Result<usize, String> {
    let service = llm.lock();
    service
        .count_tokens(&text)
        .map_err(|e| format!("Failed to count tokens: {}", e))
}
