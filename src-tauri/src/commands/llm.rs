//! LLM-related Tauri commands
//!
//! Commands for LLM model management, summarization, and text generation.

use crate::inference::llm::{GenerationConfig, LlmService};
use crate::inference::summarization::{ActionItem, SummarizationService};
use crate::models::{delete_llm_model, download_llm_model, is_llm_downloaded, LlmModel};
use crate::storage::models::SummaryType as StorageSummaryType;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

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

/// Chat message for conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistoryMessage {
    /// Message role (user or assistant)
    pub role: String,
    /// Message content
    pub content: String,
}

/// Retrieved context chunk for RAG answering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedContextChunk {
    pub meeting_id: String,
    pub meeting_title: String,
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub similarity: f32,
}

const MAX_RAG_CONTEXT_CHUNKS: usize = 12;
const MAX_RAG_CONTEXT_CHARS: usize = 14_000;
const MAX_RAG_CHUNK_EXCERPT_CHARS: usize = 900;

fn format_chat_history(history: Option<Vec<ChatHistoryMessage>>) -> String {
    history
        .map(|msgs| {
            msgs.iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn format_timestamp_label(ms: i64) -> String {
    let total_seconds = (ms.max(0)) / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

fn trim_text_for_context(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }

    let truncated: String = compact.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", truncated)
}

fn format_context_chunks(chunks: &[RetrievedContextChunk]) -> String {
    let mut context = String::new();

    for (idx, chunk) in chunks.iter().take(MAX_RAG_CONTEXT_CHUNKS).enumerate() {
        let excerpt = trim_text_for_context(&chunk.text, MAX_RAG_CHUNK_EXCERPT_CHARS);
        let time_label = match (chunk.start_ms, chunk.end_ms) {
            (Some(start), Some(end)) => {
                format!(
                    "{}-{}",
                    format_timestamp_label(start),
                    format_timestamp_label(end)
                )
            }
            (Some(start), None) => format_timestamp_label(start),
            _ => "n/a".to_string(),
        };

        let block = format!(
            "[Source {idx} | meeting: {title} | time: {time}]\n{excerpt}\n\n",
            idx = idx + 1,
            title = chunk.meeting_title,
            time = time_label,
            excerpt = excerpt
        );

        if !context.is_empty() && context.len() + block.len() > MAX_RAG_CONTEXT_CHARS {
            break;
        }
        context.push_str(&block);
    }

    context
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryGenerationProgressEvent {
    pub meeting_id: String,
    pub summary_type: String,
    pub stage: String,
    pub percent: f32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryGenerationFinishedEvent {
    pub meeting_id: String,
    pub summary_type: String,
    pub success: bool,
    pub summary: Option<String>,
    pub action_items: Option<Vec<ActionItem>>,
    pub error_message: Option<String>,
}

enum GeneratedSummaryPayload {
    Full(String),
    ActionItems(Vec<ActionItem>),
}

type SummaryStoragePayload = (String, Option<String>, Option<Vec<ActionItem>>);

impl GeneratedSummaryPayload {
    fn into_content_for_storage(self) -> Result<SummaryStoragePayload, String> {
        match self {
            Self::Full(summary) => Ok((summary.clone(), Some(summary), None)),
            Self::ActionItems(items) => {
                let serialized = serde_json::to_string(&items)
                    .map_err(|e| format!("Failed to serialize action items: {}", e))?;
                Ok((serialized, None, Some(items)))
            }
        }
    }
}

fn emit_summary_progress(app: &AppHandle, event: SummaryGenerationProgressEvent) {
    if let Err(e) = app.emit("summary-generation-progress", &event) {
        warn!("Failed to emit summary-generation-progress event: {}", e);
    }
}

fn emit_summary_finished(app: &AppHandle, event: SummaryGenerationFinishedEvent) {
    if let Err(e) = app.emit("summary-generation-finished", &event) {
        warn!("Failed to emit summary-generation-finished event: {}", e);
    }
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

/// Start generating a summary in the background and return immediately.
///
/// Emits progress on `summary-generation-progress` and completion/failure on
/// `summary-generation-finished`.
#[tauri::command]
pub async fn start_summary_generation(
    app: AppHandle,
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    summary_type: String,
) -> Result<(), String> {
    let summary_type: StorageSummaryType = summary_type
        .parse()
        .map_err(|e: anyhow::Error| e.to_string())?;

    if !matches!(
        summary_type,
        StorageSummaryType::Full | StorageSummaryType::ActionItems
    ) {
        return Err(
            "Only 'full' and 'action_items' background generation are supported".to_string(),
        );
    }

    let llm_state = llm.inner().clone();
    let storage_state = storage.inner().clone();
    let summary_type_name = summary_type.as_str().to_string();
    let meeting_for_task = meeting_id.clone();

    emit_summary_progress(
        &app,
        SummaryGenerationProgressEvent {
            meeting_id: meeting_id.clone(),
            summary_type: summary_type_name.clone(),
            stage: "Queued".to_string(),
            percent: 0.0,
            message: "Queued for background generation".to_string(),
        },
    );

    let app_for_task = app.clone();
    tokio::spawn(async move {
        emit_summary_progress(
            &app_for_task,
            SummaryGenerationProgressEvent {
                meeting_id: meeting_for_task.clone(),
                summary_type: summary_type_name.clone(),
                stage: "Generating".to_string(),
                percent: 55.0,
                message: "Generating content".to_string(),
            },
        );

        let generation_result = tokio::task::spawn_blocking({
            let llm_worker = llm_state.clone();
            let storage_worker = storage_state.clone();
            let meeting_worker = meeting_for_task.clone();

            move || -> Result<(GeneratedSummaryPayload, Option<String>), String> {
                let transcript = {
                    let storage_guard = storage_worker.lock();
                    let repos = storage_guard.repositories();
                    repos
                        .transcripts
                        .get_full_text(&meeting_worker)
                        .map_err(|e| format!("Failed to get transcript: {}", e))?
                };

                if transcript.trim().is_empty() {
                    return Err("No transcript available for this meeting".to_string());
                }

                let (generated, model_used) = {
                    let service = llm_worker.lock();
                    if !service.is_loaded() {
                        return Err("Language model is not initialized".to_string());
                    }

                    let summarizer = SummarizationService::new(&service);
                    let model_used = service.current_model().map(|model| model.to_string());

                    let generated = match summary_type {
                        StorageSummaryType::Full => GeneratedSummaryPayload::Full(
                            summarizer
                                .summarize(&transcript)
                                .map_err(|e| format!("Failed to generate summary: {}", e))?,
                        ),
                        StorageSummaryType::ActionItems => GeneratedSummaryPayload::ActionItems(
                            summarizer
                                .extract_action_items(&transcript)
                                .map_err(|e| format!("Failed to extract action items: {}", e))?,
                        ),
                        _ => {
                            return Err(
                                "Unsupported summary type for background generation".to_string()
                            )
                        }
                    };

                    (generated, model_used)
                };

                Ok((generated, model_used))
            }
        })
        .await;

        let (generated, model_used) = match generation_result {
            Ok(Ok(payload)) => payload,
            Ok(Err(err_msg)) => {
                emit_summary_finished(
                    &app_for_task,
                    SummaryGenerationFinishedEvent {
                        meeting_id: meeting_for_task,
                        summary_type: summary_type_name,
                        success: false,
                        summary: None,
                        action_items: None,
                        error_message: Some(err_msg),
                    },
                );
                return;
            }
            Err(join_err) => {
                let err_msg = format!("Background summary task failed: {}", join_err);
                error!("{}", err_msg);
                emit_summary_finished(
                    &app_for_task,
                    SummaryGenerationFinishedEvent {
                        meeting_id: meeting_for_task,
                        summary_type: summary_type_name,
                        success: false,
                        summary: None,
                        action_items: None,
                        error_message: Some(err_msg),
                    },
                );
                return;
            }
        };

        emit_summary_progress(
            &app_for_task,
            SummaryGenerationProgressEvent {
                meeting_id: meeting_for_task.clone(),
                summary_type: summary_type_name.clone(),
                stage: "Saving".to_string(),
                percent: 90.0,
                message: "Saving generated content".to_string(),
            },
        );

        let (content_to_store, summary, action_items) = match generated.into_content_for_storage() {
            Ok(parts) => parts,
            Err(err_msg) => {
                emit_summary_finished(
                    &app_for_task,
                    SummaryGenerationFinishedEvent {
                        meeting_id: meeting_for_task,
                        summary_type: summary_type_name,
                        success: false,
                        summary: None,
                        action_items: None,
                        error_message: Some(err_msg),
                    },
                );
                return;
            }
        };

        let save_result = {
            let storage_guard = storage_state.lock();
            let repos = storage_guard.repositories();
            repos
                .summaries
                .upsert(
                    &meeting_for_task,
                    summary_type,
                    &content_to_store,
                    model_used.as_deref(),
                )
                .map(|_| ())
                .map_err(|e| format!("Failed to save summary: {}", e))
        };

        match save_result {
            Ok(()) => {
                emit_summary_progress(
                    &app_for_task,
                    SummaryGenerationProgressEvent {
                        meeting_id: meeting_for_task.clone(),
                        summary_type: summary_type_name.clone(),
                        stage: "Complete".to_string(),
                        percent: 100.0,
                        message: "Summary generation complete".to_string(),
                    },
                );
                emit_summary_finished(
                    &app_for_task,
                    SummaryGenerationFinishedEvent {
                        meeting_id: meeting_for_task,
                        summary_type: summary_type_name,
                        success: true,
                        summary,
                        action_items,
                        error_message: None,
                    },
                );
            }
            Err(err_msg) => {
                emit_summary_finished(
                    &app_for_task,
                    SummaryGenerationFinishedEvent {
                        meeting_id: meeting_for_task,
                        summary_type: summary_type_name,
                        success: false,
                        summary: None,
                        action_items: None,
                        error_message: Some(err_msg),
                    },
                );
            }
        }
    });

    Ok(())
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

/// Answer a question about a meeting with optional conversation history
#[tauri::command]
pub fn ask_meeting_question(
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    question: String,
    history: Option<Vec<ChatHistoryMessage>>,
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

    // Format chat history if provided
    let chat_history = history
        .map(|msgs| {
            msgs.iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    if chat_history.is_empty() {
        // No history - use simple Q&A
        summarizer
            .answer_question(&question, &transcript)
            .map_err(|e| format!("Failed to answer question: {}", e))
    } else {
        // With history - use RAG prompt with context
        summarizer
            .answer_with_context(&question, &transcript, &chat_history)
            .map_err(|e| format!("Failed to answer question: {}", e))
    }
}

/// Answer a question using retrieved RAG chunks.
#[tauri::command]
pub fn answer_with_retrieval(
    llm: tauri::State<'_, SharedLlmService>,
    question: String,
    context_chunks: Vec<RetrievedContextChunk>,
    history: Option<Vec<ChatHistoryMessage>>,
) -> Result<String, String> {
    if context_chunks.is_empty() {
        return Ok(
            "I couldn't find any relevant information in your meetings for that question."
                .to_string(),
        );
    }

    let context = format_context_chunks(&context_chunks);
    if context.trim().is_empty() {
        return Ok(
            "I found related meetings, but there wasn't enough context to answer confidently."
                .to_string(),
        );
    }

    let chat_history = format_chat_history(history);
    let service = llm.lock();
    if !service.is_loaded() {
        return Err("Language model is not initialized".to_string());
    }

    let summarizer = SummarizationService::new(&service);
    summarizer
        .answer_with_context(&question, &context, &chat_history)
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

/// Streaming chat token event
#[derive(Debug, Clone, Serialize)]
pub struct ChatTokenEvent {
    /// Unique stream ID
    pub stream_id: String,
    /// Token text
    pub token: String,
    /// Whether this is the final token
    pub done: bool,
}

/// Answer a question about a meeting with streaming response
#[tauri::command]
pub fn stream_meeting_question(
    app: AppHandle,
    llm: tauri::State<'_, SharedLlmService>,
    storage: tauri::State<'_, SharedStorageState>,
    stream_id: String,
    meeting_id: String,
    question: String,
    history: Option<Vec<ChatHistoryMessage>>,
) -> Result<(), String> {
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

    // Format chat history if provided
    let chat_history = history
        .map(|msgs| {
            msgs.iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    let service = llm.lock();

    // Generate with streaming
    let app_handle = app.clone();
    let stream_id_clone = stream_id.clone();

    let result = if chat_history.is_empty() {
        // Build prompt manually for streaming
        let prepared = crate::inference::llm::prepare_transcript_for_llm(&transcript, 12000);
        let prompt = crate::inference::prompts::quick_question_prompt(&question, &prepared);
        let config = GenerationConfig::for_chat();

        service.generate_stream(&prompt, &config, |token| {
            let _ = app_handle.emit(
                "chat-token",
                ChatTokenEvent {
                    stream_id: stream_id_clone.clone(),
                    token: token.to_string(),
                    done: false,
                },
            );
        })
    } else {
        // Build RAG prompt for streaming
        let prepared = crate::inference::llm::prepare_transcript_for_llm(&transcript, 12000);
        let prompt =
            crate::inference::prompts::rag_chat_prompt(&prepared, &question, &chat_history);
        let config = GenerationConfig::for_chat();

        service.generate_stream(&prompt, &config, |token| {
            let _ = app_handle.emit(
                "chat-token",
                ChatTokenEvent {
                    stream_id: stream_id_clone.clone(),
                    token: token.to_string(),
                    done: false,
                },
            );
        })
    };

    // Emit final event
    let _ = app.emit(
        "chat-token",
        ChatTokenEvent {
            stream_id,
            token: String::new(),
            done: true,
        },
    );

    result
        .map(|_| ())
        .map_err(|e| format!("Streaming failed: {}", e))
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
