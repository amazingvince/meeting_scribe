//! Embedding-related Tauri commands
//!
//! Commands for initializing embedding service and processing embeddings.

use crate::inference::chunking::TranscriptSegmentInput;
use crate::inference::embedding::{cosine_similarity, EmbeddingService, EmbeddingTask, EMBEDDING_DIM, MAX_TOKENS};
use crate::inference::embedding_pipeline::{EmbeddingPipeline, ProcessingResult};
use crate::models::EmbeddingModel;
use futures::StreamExt;
use parking_lot::Mutex;
use serde::Serialize;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info};

use super::SharedStorageState;

/// Shared embedding service state type (lazy loaded)
pub type SharedEmbeddingService = Arc<Mutex<Option<Arc<EmbeddingService>>>>;

/// Initialize embedding service (downloads model if needed)
///
/// This will download the embedding model (~300MB) if it's not already present.
/// Progress events are emitted as `embedding-download-progress`.
#[tauri::command]
pub async fn initialize_embedding(
    app: AppHandle,
    config: tauri::State<'_, crate::AppConfig>,
    embedding: tauri::State<'_, SharedEmbeddingService>,
) -> Result<bool, String> {
    // Check if already initialized
    {
        let service = embedding.lock();
        if service.is_some() {
            info!("Embedding service already initialized");
            return Ok(true);
        }
    }

    info!("Initializing embedding service...");

    // Get model paths
    let embedding_model = EmbeddingModel::default();
    let models_dir = config.models_dir.join("embedding");
    let model_dir = models_dir.join(embedding_model.model_dir_name());

    // Create model directory
    std::fs::create_dir_all(&model_dir).map_err(|e| e.to_string())?;

    // Check if model files exist
    let model_path = model_dir.join("model.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    // Download model if needed
    if !model_path.exists() {
        info!("Downloading embedding model...");
        let model_info = embedding_model.model_info();
        download_file_with_progress(
            &app,
            &model_info.download_url,
            &model_path,
            model_info.size_bytes,
            "model",
        )
        .await?;
    }

    // Download tokenizer if needed
    if !tokenizer_path.exists() {
        info!("Downloading tokenizer...");
        let tokenizer_info = EmbeddingModel::tokenizer_info();
        download_file_with_progress(
            &app,
            &tokenizer_info.download_url,
            &tokenizer_path,
            tokenizer_info.size_bytes,
            "tokenizer",
        )
        .await?;
    }

    // Load the model
    info!("Loading embedding model from {:?}", model_path);
    let service = EmbeddingService::load(&model_path, &tokenizer_path)
        .map_err(|e| format!("Failed to load embedding model: {}", e))?;

    // Store in state (wrapped in Arc)
    {
        let mut guard = embedding.lock();
        *guard = Some(Arc::new(service));
    }

    info!("Embedding service initialized successfully");
    Ok(true)
}

/// Download a file with progress events
async fn download_file_with_progress(
    app: &AppHandle,
    url: &str,
    dest_path: &PathBuf,
    expected_size: u64,
    file_name: &str,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("meeting-scribe/0.1.0")
        .build()
        .map_err(|e| e.to_string())?;

    debug!("Downloading {} to {:?}", url, dest_path);

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Server returned error: {}", e))?;

    let total_size = response.content_length().unwrap_or(expected_size);

    // Create parent directory if needed
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut file =
        std::fs::File::create(dest_path).map_err(|e| format!("Failed to create file: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    // Report starting
    let _ = app.emit(
        "embedding-download-progress",
        EmbeddingDownloadProgress {
            model_id: "embeddinggemma".to_string(),
            file: file_name.to_string(),
            downloaded: 0,
            total: total_size,
            percent: 0,
            status: "downloading".to_string(),
        },
    );

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Error reading stream: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write: {}", e))?;

        downloaded += chunk.len() as u64;

        // Report progress (limit updates to avoid flooding)
        if downloaded % (1024 * 100) < chunk.len() as u64 || downloaded >= total_size {
            let percent = if total_size > 0 {
                ((downloaded as f32 / total_size as f32) * 100.0) as u8
            } else {
                0
            };

            let _ = app.emit(
                "embedding-download-progress",
                EmbeddingDownloadProgress {
                    model_id: "embeddinggemma".to_string(),
                    file: file_name.to_string(),
                    downloaded,
                    total: total_size,
                    percent,
                    status: "downloading".to_string(),
                },
            );
        }
    }

    file.flush().map_err(|e| format!("Failed to flush: {}", e))?;

    // Report complete
    let _ = app.emit(
        "embedding-download-progress",
        EmbeddingDownloadProgress {
            model_id: "embeddinggemma".to_string(),
            file: file_name.to_string(),
            downloaded,
            total: total_size,
            percent: 100,
            status: "complete".to_string(),
        },
    );

    info!("Downloaded {} bytes to {:?}", downloaded, dest_path);
    Ok(())
}

/// Check if embedding model is ready
#[tauri::command]
pub fn is_embedding_ready(embedding: tauri::State<'_, SharedEmbeddingService>) -> bool {
    embedding.lock().is_some()
}

/// Check if embedding model files exist (but not necessarily loaded)
#[tauri::command]
pub fn is_embedding_downloaded(config: tauri::State<'_, crate::AppConfig>) -> bool {
    let embedding_model = EmbeddingModel::default();
    let model_dir = config
        .models_dir
        .join("embedding")
        .join(embedding_model.model_dir_name());

    let model_path = model_dir.join("model.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    model_path.exists() && tokenizer_path.exists()
}

/// Generate embedding for text
#[tauri::command]
pub fn embed_text(
    embedding: tauri::State<'_, SharedEmbeddingService>,
    text: String,
    task: String,
) -> Result<Vec<f32>, String> {
    let service = embedding.lock();
    let service = service.as_ref().ok_or("Embedding service not initialized")?;

    let task = match task.as_str() {
        "document" => EmbeddingTask::Document,
        "search" => EmbeddingTask::Search,
        "qa" | "question" => EmbeddingTask::QuestionAnswering,
        _ => EmbeddingTask::Document,
    };

    service.embed(&text, task).map_err(|e| e.to_string())
}

/// Process meeting transcript and store embeddings
#[tauri::command]
pub async fn embed_meeting_transcript(
    app: AppHandle,
    embedding: tauri::State<'_, SharedEmbeddingService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<ProcessingResult, String> {
    // Get embedding service (clone the Arc)
    let embedding_service = {
        let guard = embedding.lock();
        guard
            .as_ref()
            .ok_or("Embedding service not initialized")?
            .clone()
    };

    // Get transcript segments and vector store from storage
    let (segments, vector_store) = {
        let storage_guard = storage.lock();
        let repos = storage_guard.repositories();

        let stored_segments = repos
            .transcripts
            .get_by_meeting(&meeting_id)
            .map_err(|e| e.to_string())?;

        // Convert to input format
        let segments: Vec<TranscriptSegmentInput> = stored_segments
            .iter()
            .map(TranscriptSegmentInput::from_stored)
            .collect();

        (segments, storage_guard.vectors.clone())
    };

    if segments.is_empty() {
        return Ok(ProcessingResult {
            meeting_id,
            chunks_processed: 0,
            embeddings_stored: 0,
        });
    }

    // Create pipeline
    let pipeline = EmbeddingPipeline::new(embedding_service, vector_store);

    // Process with progress events
    let app_clone = app.clone();
    let result = pipeline
        .process_transcript(&meeting_id, segments, move |progress| {
            let _ = app_clone.emit("embedding-progress", &progress);
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Calculate similarity between two embeddings
#[tauri::command]
pub fn calculate_similarity(embedding_a: Vec<f32>, embedding_b: Vec<f32>) -> f32 {
    cosine_similarity(&embedding_a, &embedding_b)
}

/// Get embedding model info
#[tauri::command]
pub fn get_embedding_info(embedding: tauri::State<'_, SharedEmbeddingService>) -> EmbeddingInfo {
    let loaded = embedding.lock().is_some();
    let model = EmbeddingModel::default();
    let model_info = model.model_info();

    let size_formatted = model_info.size_formatted();
    EmbeddingInfo {
        loaded,
        dimension: EMBEDDING_DIM,
        max_tokens: MAX_TOKENS,
        model_id: model_info.id,
        model_name: model_info.name,
        model_size: size_formatted,
    }
}

/// Search embeddings by query text
#[tauri::command]
pub async fn semantic_search(
    embedding: tauri::State<'_, SharedEmbeddingService>,
    storage: tauri::State<'_, SharedStorageState>,
    query: String,
    limit: Option<usize>,
    meeting_id: Option<String>,
) -> Result<Vec<SemanticSearchResult>, String> {
    // Generate query embedding
    let query_vector = {
        let guard = embedding.lock();
        let service = guard.as_ref().ok_or("Embedding service not initialized")?;
        service
            .embed(&query, EmbeddingTask::Search)
            .map_err(|e| e.to_string())?
    };

    // Build filter
    let filter = meeting_id.map(|id| format!("meeting_id = '{}'", id));

    // Get vector store (clone Arc to release lock before await)
    let vectors = {
        let storage_guard = storage.lock();
        storage_guard.vectors.clone()
    };

    // Search vector store
    let results = vectors
        .search(&query_vector, limit.unwrap_or(10), filter.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    // Convert to response format
    let search_results: Vec<SemanticSearchResult> = results
        .into_iter()
        .map(|r| SemanticSearchResult {
            id: r.id,
            meeting_id: r.meeting_id,
            chunk_type: r.chunk_type,
            text: r.text,
            start_ms: r.start_ms,
            similarity: r.similarity,
        })
        .collect();

    Ok(search_results)
}

/// Unload embedding model to free memory
#[tauri::command]
pub fn unload_embedding(embedding: tauri::State<'_, SharedEmbeddingService>) -> bool {
    let mut guard = embedding.lock();
    if guard.is_some() {
        *guard = None;
        info!("Embedding service unloaded");
        true
    } else {
        false
    }
}

// === Response Types ===

/// Download progress event for embedding model
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingDownloadProgress {
    pub model_id: String,
    pub file: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: u8,
    pub status: String,
}

/// Embedding model information
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingInfo {
    pub loaded: bool,
    pub dimension: usize,
    pub max_tokens: usize,
    pub model_id: String,
    pub model_name: String,
    pub model_size: String,
}

/// Semantic search result
#[derive(Debug, Clone, Serialize)]
pub struct SemanticSearchResult {
    pub id: String,
    pub meeting_id: String,
    pub chunk_type: String,
    pub text: String,
    pub start_ms: Option<i64>,
    pub similarity: f32,
}
