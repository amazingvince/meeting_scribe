//! Embedding-related Tauri commands
//!
//! Commands for initializing embedding service and processing embeddings.

use crate::inference::chunking::TranscriptSegmentInput;
use crate::inference::embedding::{
    cosine_similarity, EmbeddingService, EmbeddingTask, EMBEDDING_DIM, MAX_TOKENS,
};
use crate::inference::embedding_pipeline::{EmbeddingPipeline, ProcessingResult};
use crate::models::EmbeddingModel;
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde::Serialize;
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter};
use tracing::{debug, info};

use super::SharedStorageState;

/// Shared embedding service state type (lazy loaded)
pub type SharedEmbeddingService = Arc<Mutex<Option<Arc<EmbeddingService>>>>;

fn embedding_init_mutex() -> &'static tokio::sync::Mutex<()> {
    static EMBEDDING_INIT_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    EMBEDDING_INIT_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

async fn ensure_embedding_initialized(
    app: &AppHandle,
    config: &crate::AppConfig,
    embedding: &SharedEmbeddingService,
) -> Result<bool, String> {
    // Check if already initialized
    {
        let service = embedding.lock();
        if service.is_some() {
            info!("Embedding service already initialized");
            return Ok(true);
        }
    }

    let _init_guard = embedding_init_mutex().lock().await;

    // Re-check after acquiring the init lock in case another request initialized first.
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

    // Check if model files exist - use model-specific file names
    let model_file_name = embedding_model.model_file_name();
    let data_file_name = embedding_model.data_file_name();
    let model_path = model_dir.join(model_file_name);
    let data_path = model_dir.join(data_file_name);
    let tokenizer_path = model_dir.join("tokenizer.json");

    // Download model file if needed
    if !model_path.exists() {
        info!("Downloading embedding model file...");
        let model_info = embedding_model.model_info();
        download_file_with_progress(
            app,
            &model_info.download_url,
            &model_path,
            568_000, // Model file is small (~568KB), weights are in data file
            "model",
        )
        .await?;
    }

    // Download data file (external weights) if needed
    if !data_path.exists() {
        info!("Downloading embedding model weights...");
        download_file_with_progress(
            app,
            embedding_model.data_file_url(),
            &data_path,
            embedding_model.data_file_size(),
            "weights",
        )
        .await?;
    }

    // Download tokenizer if needed
    if !tokenizer_path.exists() {
        info!("Downloading tokenizer...");
        let tokenizer_info = EmbeddingModel::tokenizer_info();
        download_file_with_progress(
            app,
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
    ensure_embedding_initialized(&app, &config, embedding.inner()).await
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

    file.flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

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

    let model_path = model_dir.join(embedding_model.model_file_name());
    let data_path = model_dir.join(embedding_model.data_file_name());
    let tokenizer_path = model_dir.join("tokenizer.json");

    model_path.exists() && data_path.exists() && tokenizer_path.exists()
}

/// Generate embedding for text
#[tauri::command]
pub fn embed_text(
    embedding: tauri::State<'_, SharedEmbeddingService>,
    text: String,
    task: String,
) -> Result<Vec<f32>, String> {
    let service = embedding.lock();
    let service = service
        .as_ref()
        .ok_or("Embedding service not initialized")?;

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
    config: tauri::State<'_, crate::AppConfig>,
    embedding: tauri::State<'_, SharedEmbeddingService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<ProcessingResult, String> {
    ensure_embedding_initialized(&app, &config, embedding.inner()).await?;

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
    app: AppHandle,
    config: tauri::State<'_, crate::AppConfig>,
    embedding: tauri::State<'_, SharedEmbeddingService>,
    storage: tauri::State<'_, SharedStorageState>,
    query: String,
    limit: Option<usize>,
    meeting_id: Option<String>,
) -> Result<Vec<SemanticSearchResult>, String> {
    ensure_embedding_initialized(&app, &config, embedding.inner()).await?;

    // Generate query embedding
    let query_vector = {
        let guard = embedding.lock();
        let service = guard.as_ref().ok_or("Embedding service not initialized")?;
        service
            .embed(&query, EmbeddingTask::Search)
            .map_err(|e| e.to_string())?
    };

    // Build filter
    let filter = meeting_id.map(|id| {
        let escaped = id.replace('\'', "''");
        format!("meeting_id = '{}'", escaped)
    });

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

    // Collect unique meeting IDs for batch lookup
    let meeting_ids: std::collections::HashSet<String> =
        results.iter().map(|r| r.meeting_id.clone()).collect();

    // Fetch meeting titles from database
    let meeting_titles: std::collections::HashMap<String, String> = {
        let storage_guard = storage.lock();
        let repos = storage_guard.repositories();
        let mut titles = std::collections::HashMap::new();
        for mid in meeting_ids {
            if let Ok(Some(meeting)) = repos.meetings.get(&mid) {
                titles.insert(mid, meeting.title);
            }
        }
        titles
    };

    // Convert to response format with meeting titles
    let search_results: Vec<SemanticSearchResult> = results
        .into_iter()
        .map(|r| {
            let title = meeting_titles
                .get(&r.meeting_id)
                .cloned()
                .unwrap_or_else(|| "Unknown Meeting".to_string());
            SemanticSearchResult {
                id: r.id,
                meeting_id: r.meeting_id,
                meeting_title: title,
                chunk_type: r.chunk_type,
                text: r.text,
                start_ms: r.start_ms,
                end_ms: r.end_ms,
                chunk_index: r.chunk_index,
                similarity: r.similarity,
            }
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

/// Delete embedding model files (auto-unloads if loaded)
#[tauri::command]
pub async fn delete_embedding(
    config: tauri::State<'_, crate::AppConfig>,
    embedding: tauri::State<'_, SharedEmbeddingService>,
) -> Result<(), String> {
    // Auto-unload if currently loaded
    {
        let mut guard = embedding.lock();
        if guard.is_some() {
            info!("Unloading embedding model before deletion");
            *guard = None;
        }
    }

    // Get model directory
    let embedding_model = EmbeddingModel::default();
    let model_dir = config
        .models_dir
        .join("embedding")
        .join(embedding_model.model_dir_name());

    // Delete the model directory
    if model_dir.exists() {
        info!("Deleting embedding model directory: {:?}", model_dir);
        std::fs::remove_dir_all(&model_dir)
            .map_err(|e| format!("Failed to delete embedding model: {}", e))?;
        info!("Embedding model deleted successfully");
    }

    Ok(())
}

/// Hybrid search combining vector search and full-text search
///
/// Uses Reciprocal Rank Fusion (RRF) to merge results from both search methods.
#[tauri::command]
pub async fn hybrid_search(
    app: AppHandle,
    config: tauri::State<'_, crate::AppConfig>,
    embedding: tauri::State<'_, SharedEmbeddingService>,
    storage: tauri::State<'_, SharedStorageState>,
    query: String,
    limit: Option<usize>,
    meeting_id: Option<String>,
) -> Result<Vec<SemanticSearchResult>, String> {
    let limit = limit.unwrap_or(10);

    // Run semantic search (vector)
    let vector_results = semantic_search(
        app.clone(),
        config.clone(),
        embedding.clone(),
        storage.clone(),
        query.clone(),
        Some(limit * 2), // Get more results for fusion
        meeting_id.clone(),
    )
    .await?;

    // Run FTS search
    let fts_results = {
        let storage_guard = storage.lock();
        let search_limit = (limit * 2) as u32;

        if let Some(ref mid) = meeting_id {
            storage_guard
                .search
                .search_in_meeting(mid, &query, search_limit)
                .map_err(|e| e.to_string())?
        } else {
            storage_guard
                .search
                .search_transcripts(&query, search_limit)
                .map_err(|e| e.to_string())?
        }
    };

    // Merge results using Reciprocal Rank Fusion (RRF)
    // RRF score = sum(1 / (k + rank)) where k = 60 is a constant
    const RRF_K: f32 = 60.0;

    // Build a map of result key -> best merged result score.
    let mut result_map: std::collections::HashMap<String, HybridResult> =
        std::collections::HashMap::new();

    // Add vector results with their ranks
    for (rank, result) in vector_results.iter().enumerate() {
        let key = hybrid_result_key(
            &result.meeting_id,
            &result.chunk_type,
            result.start_ms,
            result.end_ms,
            result.chunk_index,
            &result.text,
        );
        let rrf_score = 1.0 / (RRF_K + rank as f32 + 1.0);

        result_map
            .entry(key)
            .or_insert_with(|| HybridResult {
                result: result.clone(),
                rrf_score: 0.0,
            })
            .rrf_score += rrf_score;
    }

    // Add FTS results with their ranks
    for (rank, fts_hit) in fts_results.iter().enumerate() {
        let key = hybrid_result_key(
            &fts_hit.meeting_id,
            "fts",
            Some(fts_hit.start_ms),
            Some(fts_hit.end_ms),
            Some(fts_hit.segment_id),
            &fts_hit.text,
        );
        let rrf_score = 1.0 / (RRF_K + rank as f32 + 1.0);

        result_map
            .entry(key.clone())
            .and_modify(|hr| hr.rrf_score += rrf_score)
            .or_insert_with(|| {
                // Convert FTS result to SemanticSearchResult format
                HybridResult {
                    result: SemanticSearchResult {
                        id: fts_hit.segment_id.to_string(),
                        meeting_id: fts_hit.meeting_id.clone(),
                        meeting_title: fts_hit.meeting_title.clone(),
                        chunk_type: "fts".to_string(),
                        text: fts_hit.text.clone(),
                        start_ms: Some(fts_hit.start_ms),
                        end_ms: Some(fts_hit.end_ms),
                        chunk_index: Some(fts_hit.segment_id),
                        similarity: 0.0, // FTS doesn't have similarity score
                    },
                    rrf_score,
                }
            });
    }

    // Sort by RRF score (higher is better)
    let mut merged: Vec<HybridResult> = result_map.into_values().collect();
    merged.sort_by(|a, b| b.rrf_score.total_cmp(&a.rrf_score));

    // Take top N results
    let final_results: Vec<SemanticSearchResult> =
        merged.into_iter().take(limit).map(|hr| hr.result).collect();

    debug!(
        "Hybrid search for '{}': {} vector + {} FTS -> {} merged results",
        query,
        vector_results.len(),
        fts_results.len(),
        final_results.len()
    );

    Ok(final_results)
}

/// Normalize text for deduplication
fn normalize_text(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn hybrid_result_key(
    meeting_id: &str,
    chunk_type: &str,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    chunk_index: Option<i64>,
    text: &str,
) -> String {
    if let Some(index) = chunk_index {
        return format!("{}::{}::idx::{}", meeting_id, chunk_type, index);
    }

    format!(
        "{}::{}::{}::{}::{}",
        meeting_id,
        chunk_type,
        start_ms.unwrap_or(-1),
        end_ms.unwrap_or(-1),
        normalize_text(text)
    )
}

/// Helper struct for RRF merging
struct HybridResult {
    result: SemanticSearchResult,
    rrf_score: f32,
}

/// Get list of meetings that don't have embeddings yet
#[tauri::command]
pub async fn get_unembedded_meetings(
    storage: tauri::State<'_, SharedStorageState>,
) -> Result<Vec<UnembeddedMeeting>, String> {
    use crate::storage::ListOptions;

    // Get all meetings (use high limit to get all)
    let meetings = {
        let storage_guard = storage.lock();
        let repos = storage_guard.repositories();
        let options = ListOptions::new().with_limit(10000); // High limit to get all
        repos.meetings.list(options).map_err(|e| e.to_string())?
    };

    // Get vector store
    let vectors = {
        let storage_guard = storage.lock();
        storage_guard.vectors.clone()
    };

    // Check which meetings have embeddings
    let mut unembedded = Vec::new();
    for meeting in meetings {
        let count = vectors.count_for_meeting(&meeting.id).await.unwrap_or(0);

        if count == 0 {
            // Check if meeting has a transcript
            let has_transcript = {
                let storage_guard = storage.lock();
                let repos = storage_guard.repositories();
                repos
                    .transcripts
                    .get_by_meeting(&meeting.id)
                    .map(|segments| !segments.is_empty())
                    .unwrap_or(false)
            };

            if has_transcript {
                unembedded.push(UnembeddedMeeting {
                    id: meeting.id,
                    title: meeting.title,
                    created_at: meeting.created_at.to_string(),
                });
            }
        }
    }

    Ok(unembedded)
}

/// Batch embed all meetings that don't have embeddings
#[tauri::command]
pub async fn batch_embed_meetings(
    app: AppHandle,
    config: tauri::State<'_, crate::AppConfig>,
    embedding: tauri::State<'_, SharedEmbeddingService>,
    storage: tauri::State<'_, SharedStorageState>,
) -> Result<BatchEmbedResult, String> {
    ensure_embedding_initialized(&app, &config, embedding.inner()).await?;

    // Get embedding service
    let embedding_service = {
        let guard = embedding.lock();
        guard
            .as_ref()
            .ok_or("Embedding service not initialized. Please download and load the embedding model first.")?
            .clone()
    };

    // Get unembedded meetings
    let unembedded = get_unembedded_meetings(storage.clone()).await?;

    if unembedded.is_empty() {
        return Ok(BatchEmbedResult {
            processed: 0,
            total: 0,
            failed: vec![],
        });
    }

    let total = unembedded.len();
    let mut processed = 0;
    let mut failed = Vec::new();

    // Emit initial progress
    let _ = app.emit(
        "batch-embed-progress",
        BatchEmbedProgress {
            current: 0,
            total,
            current_meeting: unembedded
                .first()
                .map(|m| m.title.clone())
                .unwrap_or_default(),
            status: "starting".to_string(),
        },
    );

    for (index, meeting) in unembedded.iter().enumerate() {
        // Emit progress
        let _ = app.emit(
            "batch-embed-progress",
            BatchEmbedProgress {
                current: index,
                total,
                current_meeting: meeting.title.clone(),
                status: "processing".to_string(),
            },
        );

        // Get transcript segments
        let segments = {
            let storage_guard = storage.lock();
            let repos = storage_guard.repositories();
            match repos.transcripts.get_by_meeting(&meeting.id) {
                Ok(segs) => segs
                    .iter()
                    .map(TranscriptSegmentInput::from_stored)
                    .collect::<Vec<_>>(),
                Err(e) => {
                    failed.push(FailedMeeting {
                        id: meeting.id.clone(),
                        title: meeting.title.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
            }
        };

        if segments.is_empty() {
            continue;
        }

        // Get vector store
        let vector_store = {
            let storage_guard = storage.lock();
            storage_guard.vectors.clone()
        };

        // Create pipeline and process
        let pipeline = EmbeddingPipeline::new(embedding_service.clone(), vector_store);

        match pipeline
            .process_transcript(&meeting.id, segments, |_| {})
            .await
        {
            Ok(_) => {
                processed += 1;
            }
            Err(e) => {
                failed.push(FailedMeeting {
                    id: meeting.id.clone(),
                    title: meeting.title.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // Emit completion
    let _ = app.emit(
        "batch-embed-progress",
        BatchEmbedProgress {
            current: total,
            total,
            current_meeting: String::new(),
            status: "complete".to_string(),
        },
    );

    Ok(BatchEmbedResult {
        processed,
        total,
        failed,
    })
}

// === Response Types ===

/// Meeting without embeddings
#[derive(Debug, Clone, Serialize)]
pub struct UnembeddedMeeting {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

/// Progress event for batch embedding
#[derive(Debug, Clone, Serialize)]
pub struct BatchEmbedProgress {
    pub current: usize,
    pub total: usize,
    pub current_meeting: String,
    pub status: String,
}

/// Result of batch embedding operation
#[derive(Debug, Clone, Serialize)]
pub struct BatchEmbedResult {
    pub processed: usize,
    pub total: usize,
    pub failed: Vec<FailedMeeting>,
}

/// Failed meeting during batch embedding
#[derive(Debug, Clone, Serialize)]
pub struct FailedMeeting {
    pub id: String,
    pub title: String,
    pub error: String,
}

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
    pub meeting_title: String,
    pub chunk_type: String,
    pub text: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub chunk_index: Option<i64>,
    pub similarity: f32,
}

#[cfg(test)]
mod tests {
    use super::hybrid_result_key;

    #[test]
    fn hybrid_result_key_is_scoped_by_meeting() {
        let key_a = hybrid_result_key(
            "meeting-a",
            "transcript",
            Some(1_000),
            Some(2_000),
            None,
            "same text",
        );
        let key_b = hybrid_result_key(
            "meeting-b",
            "transcript",
            Some(1_000),
            Some(2_000),
            None,
            "same text",
        );
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn hybrid_result_key_is_scoped_by_start_time() {
        let key_a = hybrid_result_key(
            "meeting-a",
            "transcript",
            Some(1_000),
            Some(2_000),
            None,
            "same text",
        );
        let key_b = hybrid_result_key(
            "meeting-a",
            "transcript",
            Some(2_000),
            Some(3_000),
            None,
            "same text",
        );
        assert_ne!(key_a, key_b);
    }
}
