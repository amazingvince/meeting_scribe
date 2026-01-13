//! Transcription-related Tauri commands

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::info;

use crate::inference::{
    ProcessingResult, TranscriptionConfig, TranscriptionService, TranscriptSegment,
};
use crate::models::{DownloadProgress, ModelManager, ModelStatus, TranscriptionBackend};

/// Shared transcription service type
pub type SharedTranscriptionService = Arc<TranscriptionService>;

/// Shared model manager type
pub type SharedModelManager = Arc<Mutex<ModelManager>>;

/// Response for model status query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatusResponse {
    pub models: Vec<ModelStatusItem>,
}

/// Individual model status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatusItem {
    pub id: String,
    pub name: String,
    pub status: ModelStatus,
    pub size: String,
    pub description: String,
    pub is_default: bool,
}

/// Download progress event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub model_id: String,
    pub stage: String,
    pub percent: f32,
    pub message: String,
}

/// Get status of all transcription models
#[tauri::command]
pub fn get_model_status(
    model_manager: tauri::State<'_, SharedModelManager>,
) -> Result<ModelStatusResponse, String> {
    let manager = model_manager.lock();

    let models: Vec<ModelStatusItem> = TranscriptionBackend::all()
        .iter()
        .map(|backend| {
            let info = backend.model_info();
            let status = manager.get_backend_status(*backend);

            ModelStatusItem {
                id: info.id.clone(),
                name: info.name.clone(),
                status,
                size: info.size_formatted(),
                description: info.description.clone(),
                is_default: *backend == TranscriptionBackend::default(),
            }
        })
        .collect();

    Ok(ModelStatusResponse { models })
}

/// Download a transcription model
#[tauri::command]
pub async fn download_transcription_model(
    app: AppHandle,
    model_manager: tauri::State<'_, SharedModelManager>,
    backend: TranscriptionBackend,
) -> Result<String, String> {
    let manager = {
        let guard = model_manager.lock();
        // Clone what we need - can't hold lock across await
        let models_dir = guard.models_dir().to_path_buf();
        models_dir
    };

    // Create a temporary manager for the download (can't hold state across await)
    let temp_manager = ModelManager::new(manager).map_err(|e| e.to_string())?;

    let model_info = backend.model_info();
    info!("Starting download of model: {}", model_info.name);

    // Create progress callback that emits events
    let app_handle = app.clone();
    let model_id = model_info.id.clone();

    let progress_callback = move |progress: DownloadProgress| {
        let event = DownloadProgressEvent {
            model_id: progress.model_id.clone(),
            stage: format!("{:?}", progress.stage),
            percent: progress.percent,
            message: match &progress.stage {
                crate::models::DownloadStage::Starting => "Starting download...".to_string(),
                crate::models::DownloadStage::Downloading => {
                    format!("Downloading... {:.0}%", progress.percent)
                }
                crate::models::DownloadStage::Extracting => "Extracting archive...".to_string(),
                crate::models::DownloadStage::Verifying => "Verifying files...".to_string(),
                crate::models::DownloadStage::Complete => "Download complete!".to_string(),
                crate::models::DownloadStage::Failed(msg) => format!("Failed: {}", msg),
            },
        };

        let _ = app_handle.emit("model-download-progress", &event);
    };

    // Perform download
    let result = temp_manager
        .download_model(backend, progress_callback)
        .await
        .map_err(|e| e.to_string())?;

    // Update the shared manager's status
    {
        let manager = model_manager.lock();
        manager.set_status(&model_id, ModelStatus::Ready);
    }

    info!("Model {} downloaded to {:?}", model_info.name, result);

    Ok(result.display().to_string())
}

/// Initialize the transcription engine
#[tauri::command]
pub fn init_transcription(
    transcription: tauri::State<'_, SharedTranscriptionService>,
    model_manager: tauri::State<'_, SharedModelManager>,
    backend: TranscriptionBackend,
) -> Result<(), String> {
    let manager = model_manager.lock();

    // Check if model is ready
    if !manager.is_model_ready(backend) {
        return Err(format!(
            "Model {} is not downloaded. Please download it first.",
            backend.model_info().name
        ));
    }

    let config = TranscriptionConfig {
        backend,
        ..Default::default()
    };

    transcription
        .initialize(&manager, config)
        .map_err(|e| e.to_string())?;

    info!("Transcription engine initialized with {:?}", backend);
    Ok(())
}

/// Check if transcription is ready
#[tauri::command]
pub fn is_transcription_ready(
    transcription: tauri::State<'_, SharedTranscriptionService>,
) -> bool {
    transcription.is_ready()
}

/// Get current transcription configuration
#[tauri::command]
pub fn get_transcription_config(
    transcription: tauri::State<'_, SharedTranscriptionService>,
) -> TranscriptionConfig {
    transcription.config()
}

/// Transcribe a single audio file
#[tauri::command]
pub fn transcribe_file(
    transcription: tauri::State<'_, SharedTranscriptionService>,
    audio_path: String,
) -> Result<Vec<TranscriptSegment>, String> {
    let path = PathBuf::from(&audio_path);

    if !path.exists() {
        return Err(format!("Audio file not found: {}", audio_path));
    }

    if !transcription.is_ready() {
        return Err("Transcription engine not initialized".to_string());
    }

    transcription
        .transcribe_file(&path)
        .map_err(|e| e.to_string())
}

/// Process a complete meeting (both mic and system audio)
#[tauri::command]
pub fn process_meeting(
    app: AppHandle,
    transcription: tauri::State<'_, SharedTranscriptionService>,
    meeting_id: String,
    mic_path: Option<String>,
    system_path: Option<String>,
) -> Result<ProcessingResult, String> {
    use crate::inference::{
        format_transcript, merge_transcripts, Speaker, TranscriptStats,
    };
    use std::time::Instant;

    if !transcription.is_ready() {
        return Err("Transcription engine not initialized".to_string());
    }

    let start_time = Instant::now();
    let backend = transcription.backend();

    // Emit progress: starting
    let _ = app.emit(
        "meeting-processing-progress",
        serde_json::json!({
            "meeting_id": meeting_id,
            "stage": "TranscribingMic",
            "percent": 10.0,
            "message": "Starting transcription..."
        }),
    );

    let mut mic_segments = Vec::new();
    let mut system_segments = Vec::new();
    let mut total_duration_ms: u64 = 0;

    // Transcribe microphone audio
    if let Some(ref path_str) = mic_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            let _ = app.emit(
                "meeting-processing-progress",
                serde_json::json!({
                    "meeting_id": meeting_id,
                    "stage": "TranscribingMic",
                    "percent": 20.0,
                    "message": "Transcribing microphone audio..."
                }),
            );

            mic_segments = transcription
                .transcribe_file_with_speaker(&path, Speaker::You)
                .map_err(|e| e.to_string())?;

            if let Some(last) = mic_segments.last() {
                total_duration_ms = total_duration_ms.max(last.end_ms);
            }

            info!("Mic transcription: {} segments", mic_segments.len());
        }
    }

    // Transcribe system audio
    if let Some(ref path_str) = system_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            let _ = app.emit(
                "meeting-processing-progress",
                serde_json::json!({
                    "meeting_id": meeting_id,
                    "stage": "TranscribingSystem",
                    "percent": 50.0,
                    "message": "Transcribing system audio..."
                }),
            );

            system_segments = transcription
                .transcribe_file_with_speaker(&path, Speaker::Others)
                .map_err(|e| e.to_string())?;

            if let Some(last) = system_segments.last() {
                total_duration_ms = total_duration_ms.max(last.end_ms);
            }

            info!("System transcription: {} segments", system_segments.len());
        }
    }

    // Merge transcripts
    let _ = app.emit(
        "meeting-processing-progress",
        serde_json::json!({
            "meeting_id": meeting_id,
            "stage": "Merging",
            "percent": 80.0,
            "message": "Merging transcripts..."
        }),
    );

    let transcript = merge_transcripts(mic_segments, system_segments);
    let formatted_text = format_transcript(&transcript);
    let stats = TranscriptStats::from_segments(&transcript);

    // Calculate speech ratio
    let speech_time: u64 = transcript.iter().map(|s| s.duration_ms()).sum();
    let speech_ratio = if total_duration_ms > 0 {
        speech_time as f32 / total_duration_ms as f32
    } else {
        0.0
    };

    let processing_time_ms = start_time.elapsed().as_millis() as u64;

    // Emit progress: complete
    let _ = app.emit(
        "meeting-processing-progress",
        serde_json::json!({
            "meeting_id": meeting_id,
            "stage": "Complete",
            "percent": 100.0,
            "message": "Processing complete"
        }),
    );

    info!(
        "Meeting {} processed: {} segments, {}ms",
        meeting_id,
        transcript.len(),
        processing_time_ms
    );

    Ok(ProcessingResult {
        meeting_id,
        transcript,
        formatted_text,
        duration_ms: total_duration_ms,
        processing_time_ms,
        speech_ratio,
        backend,
        stats: stats.into(),
    })
}

/// Unload the transcription engine
#[tauri::command]
pub fn unload_transcription(transcription: tauri::State<'_, SharedTranscriptionService>) {
    transcription.unload();
    info!("Transcription engine unloaded");
}

/// Get the models directory path
#[tauri::command]
pub fn get_models_dir(
    model_manager: tauri::State<'_, SharedModelManager>,
) -> Result<String, String> {
    let manager = model_manager.lock();
    Ok(manager.models_dir().display().to_string())
}

/// Check if a specific model is downloaded
#[tauri::command]
pub fn is_model_downloaded(
    model_manager: tauri::State<'_, SharedModelManager>,
    backend: TranscriptionBackend,
) -> bool {
    let manager = model_manager.lock();
    manager.is_model_downloaded(backend)
}
