//! Transcription-related Tauri commands

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use crate::inference::{
    ProcessingResult, TranscriptSegment, TranscriptionConfig, TranscriptionService,
};
use crate::models::{DownloadProgress, ModelManager, ModelStatus, TranscriptionBackend};
use crate::storage::{MeetingStatus, StoredSegment};

use super::recording::SharedRecordingSession;
use super::storage::SharedStorageState;

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

/// Optional processing controls for meeting transcription.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMeetingOptions {
    /// Optional echo cancellation backend override (`webrtc_aec3` or `speex`).
    pub echo_backend: Option<crate::audio::aec::EchoCancellationBackend>,
}

/// Event emitted when background meeting processing finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingProcessingFinishedEvent {
    pub meeting_id: String,
    pub success: bool,
    pub segment_count: Option<usize>,
    pub processing_time_ms: Option<u64>,
    pub error_message: Option<String>,
}

/// Controls for live transcript preview while recording.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LivePreviewOptions {
    /// Tail window (in seconds) to transcribe from rolling buffers.
    pub window_seconds: Option<u32>,
    /// Include system-audio channel in preview merge.
    pub include_system_audio: Option<bool>,
}

/// Live transcript preview response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveTranscriptPreview {
    pub meeting_id: String,
    pub duration_ms: u64,
    pub window_start_ms: u64,
    pub segments: Vec<TranscriptSegment>,
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

    let init_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        transcription.initialize(&manager, config)
    }));

    match init_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e.to_string()),
        Err(panic_payload) => {
            let panic_msg = if let Some(message) = panic_payload.downcast_ref::<&str>() {
                (*message).to_string()
            } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                message.clone()
            } else {
                "unknown panic payload".to_string()
            };
            return Err(format!(
                "Transcription initialization panicked: {}. Ensure ONNX Runtime is installed and ORT_DYLIB_PATH is valid.",
                panic_msg
            ));
        }
    }

    info!("Transcription engine initialized with {:?}", backend);
    Ok(())
}

/// Check if transcription is ready
#[tauri::command]
pub fn is_transcription_ready(transcription: tauri::State<'_, SharedTranscriptionService>) -> bool {
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
///
/// When both mic and system audio exist, AEC is applied to remove echo
/// from the microphone input using system audio as reference.
#[tauri::command]
pub async fn process_meeting(
    app: AppHandle,
    transcription: tauri::State<'_, SharedTranscriptionService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    mic_path: Option<String>,
    system_path: Option<String>,
    options: Option<ProcessMeetingOptions>,
) -> Result<ProcessingResult, String> {
    let transcription = transcription.inner().clone();
    let storage = storage.inner().clone();
    let app_handle = app.clone();

    tokio::task::spawn_blocking(move || {
        process_meeting_blocking(
            &app_handle,
            &transcription,
            &storage,
            meeting_id,
            mic_path,
            system_path,
            options,
        )
    })
    .await
    .map_err(|e| format!("Meeting processing task failed: {}", e))?
}

fn process_meeting_blocking(
    app: &AppHandle,
    transcription: &SharedTranscriptionService,
    storage: &SharedStorageState,
    meeting_id: String,
    mic_path: Option<String>,
    system_path: Option<String>,
    options: Option<ProcessMeetingOptions>,
) -> Result<ProcessingResult, String> {
    use crate::audio::{
        aec::{
            align_reference_for_aec, process_echo_cancellation, resolve_echo_backend,
            suppress_residual_echo,
        },
        capture::{load_wav, save_wav},
        WHISPER_SAMPLE_RATE,
    };
    use crate::inference::{format_transcript, merge_transcripts, Speaker, TranscriptStats};
    use std::time::Instant;

    if !transcription.is_ready() {
        return Err("Transcription engine not initialized".to_string());
    }

    let mic_path = mic_path.map(PathBuf::from);
    let system_path = system_path.map(PathBuf::from);

    if mic_path.is_none() && system_path.is_none() {
        return Err("No audio paths were provided for transcription".to_string());
    }

    if let Some(path) = mic_path.as_ref() {
        if !path.exists() {
            return Err(format!(
                "Microphone audio file not found: {}",
                path.display()
            ));
        }
    }

    if let Some(path) = system_path.as_ref() {
        if !path.exists() {
            return Err(format!("System audio file not found: {}", path.display()));
        }
    }

    let start_time = Instant::now();
    let backend = transcription.backend();
    let requested_echo_backend = options.and_then(|o| o.echo_backend);
    let echo_backend = resolve_echo_backend(requested_echo_backend);
    info!(
        "Echo cancellation backend selected: {}",
        echo_backend.as_str()
    );

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
    let mut cleaned_mic_playback_path: Option<PathBuf> = None;

    // Load system audio for AEC reference (if exists)
    let system_samples: Option<Vec<f32>> =
        system_path.as_ref().and_then(|path| match load_wav(path) {
            Ok(samples) if !samples.is_empty() => {
                info!(
                    "Loaded system audio for AEC: {} samples ({:.1}s)",
                    samples.len(),
                    samples.len() as f32 / WHISPER_SAMPLE_RATE as f32
                );
                Some(samples)
            }
            Ok(_) => {
                info!("System audio file was empty; skipping AEC reference");
                None
            }
            Err(e) => {
                info!("Could not load system audio for AEC: {}", e);
                None
            }
        });

    // Transcribe microphone audio (with AEC if system audio available)
    if let Some(requested_mic_path) = mic_path.as_ref() {
        let mic_processing_path = resolve_mic_processing_path(requested_mic_path);
        let mic_source_is_clean = is_clean_mic_path(&mic_processing_path);

        let _ = app.emit(
            "meeting-processing-progress",
            serde_json::json!({
                "meeting_id": meeting_id,
                "stage": "TranscribingMic",
                "percent": 20.0,
                "message": if mic_source_is_clean {
                    "Transcribing cleaned microphone audio..."
                } else if system_samples.is_some() {
                    "Applying echo cancellation and transcribing microphone audio..."
                } else {
                    "Transcribing microphone audio..."
                }
            }),
        );

        if mic_processing_path != *requested_mic_path {
            info!(
                "Using alternate mic source for processing: requested={}, actual={}",
                requested_mic_path.display(),
                mic_processing_path.display()
            );
        }

        // Load mic audio
        let mic_samples = load_wav(&mic_processing_path)
            .map_err(|e| format!("Failed to load mic audio: {}", e))?;

        // Apply AEC if we have system audio as reference
        let processed_mic = if !mic_source_is_clean {
            if let Some(ref sys_samples) = system_samples {
                info!(
                    "Applying AEC: {} mic samples, {} reference samples",
                    mic_samples.len(),
                    sys_samples.len()
                );

                let (aligned_reference, alignment) =
                    align_reference_for_aec(&mic_samples, sys_samples, WHISPER_SAMPLE_RATE);
                info!(
                    "AEC reference alignment: shift={} samples ({:.1}ms), corr={:.3}",
                    alignment.shift_samples,
                    alignment.shift_ms(WHISPER_SAMPLE_RATE),
                    alignment.correlation
                );

                let (processed, echo_info) = process_echo_cancellation(
                    &mic_samples,
                    &aligned_reference,
                    WHISPER_SAMPLE_RATE,
                    echo_backend,
                );
                info!(
                    "Echo cancellation backend result: requested={}, used={}, fallback={}",
                    echo_info.requested_backend.as_str(),
                    echo_info.backend_used.as_str(),
                    echo_info.fallback_used
                );
                let processed =
                    suppress_residual_echo(&processed, &aligned_reference, WHISPER_SAMPLE_RATE);

                let input_rms = rms(&mic_samples);
                let output_rms = rms(&processed);
                let attenuation_db = if input_rms > 0.0 && output_rms > 0.0 {
                    20.0 * (output_rms / input_rms).log10()
                } else {
                    0.0
                };
                info!(
                    "AEC + residual suppression RMS: in={:.5}, out={:.5}, attenuation={:.2}dB",
                    input_rms, output_rms, attenuation_db
                );

                info!(
                    "AEC complete: {} input samples -> {} output samples",
                    mic_samples.len(),
                    processed.len()
                );
                processed
            } else {
                mic_samples
            }
        } else {
            info!(
                "Using cleaned microphone source directly for transcription: {}",
                mic_processing_path.display()
            );
            mic_samples
        };

        if mic_source_is_clean {
            cleaned_mic_playback_path = Some(mic_processing_path.clone());
        } else if system_samples.is_some() {
            if let Some(cleaned_path) = derive_cleaned_mic_path(requested_mic_path) {
                match save_wav(&cleaned_path, &processed_mic) {
                    Ok(()) => {
                        info!("Saved cleaned mic audio to {}", cleaned_path.display());
                        cleaned_mic_playback_path = Some(cleaned_path);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to save cleaned mic audio to {}: {}",
                            cleaned_path.display(),
                            e
                        );
                    }
                }
            }
        }

        // Transcribe the (possibly AEC'd) mic audio
        mic_segments = transcription
            .transcribe_samples_with_speaker(processed_mic, Speaker::You)
            .map_err(|e| e.to_string())?;

        if let Some(last) = mic_segments.last() {
            total_duration_ms = total_duration_ms.max(last.end_ms);
        }

        info!("Mic transcription: {} segments", mic_segments.len());
    }

    // Transcribe system audio (no AEC needed - this is the reference)
    if let Some(path) = system_path.as_ref() {
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
            .transcribe_file_with_speaker(path, Speaker::Others)
            .map_err(|e| e.to_string())?;

        if let Some(last) = system_segments.last() {
            total_duration_ms = total_duration_ms.max(last.end_ms);
        }

        info!("System transcription: {} segments", system_segments.len());
    }

    if mic_segments.is_empty() && system_segments.is_empty() {
        return Err(format!(
            "No transcribable audio content found for meeting {}",
            meeting_id
        ));
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

    info!(
        "Merged transcript has {} segments (before save)",
        transcript.len()
    );

    // Save transcript to database
    {
        let storage = storage.lock();
        let repos = storage.repositories();

        // Delete existing transcript segments first (for regeneration)
        repos
            .transcripts
            .delete_by_meeting(&meeting_id)
            .map_err(|e| format!("Failed to clear existing transcript: {}", e))?;
        info!("Cleared existing transcript for meeting {}", meeting_id);

        let stored_segments: Vec<StoredSegment> = transcript
            .iter()
            .map(|s| StoredSegment::from_inference(s, &meeting_id))
            .collect();
        repos
            .transcripts
            .insert_batch(&stored_segments)
            .map_err(|e| e.to_string())?;
        info!(
            "Saved {} transcript segments for meeting {}",
            stored_segments.len(),
            meeting_id
        );

        if let Some(cleaned_mic_path) = cleaned_mic_playback_path.as_ref() {
            let cleaned_path_str = cleaned_mic_path.display().to_string();
            match repos.meetings.get(&meeting_id) {
                Ok(Some(mut meeting)) => {
                    let needs_update =
                        meeting.audio_path_you.as_deref() != Some(cleaned_path_str.as_str());
                    if needs_update {
                        meeting.audio_path_you = Some(cleaned_path_str);
                        meeting.touch();
                        if let Err(e) = repos.meetings.update(&meeting) {
                            warn!(
                                "Failed to update meeting {} mic playback path to cleaned file: {}",
                                meeting_id, e
                            );
                        } else {
                            info!(
                                "Updated meeting {} mic playback path to cleaned audio",
                                meeting_id
                            );
                        }
                    }
                }
                Ok(None) => {
                    warn!(
                        "Meeting {} not found while setting cleaned mic playback path",
                        meeting_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to load meeting {} while setting cleaned mic playback path: {}",
                        meeting_id, e
                    );
                }
            }
        }
    }

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

/// Start processing a meeting in the background and return immediately.
///
/// Progress is emitted via `meeting-processing-progress`; completion/failure is emitted via
/// `meeting-processing-finished`.
#[tauri::command]
pub async fn start_meeting_processing(
    app: AppHandle,
    transcription: tauri::State<'_, SharedTranscriptionService>,
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    mic_path: Option<String>,
    system_path: Option<String>,
    options: Option<ProcessMeetingOptions>,
) -> Result<(), String> {
    let transcription = transcription.inner().clone();
    let storage = storage.inner().clone();

    if !transcription.is_ready() {
        return Err("Transcription engine not initialized".to_string());
    }

    {
        let storage_guard = storage.lock();
        let repos = storage_guard.repositories();
        let exists = repos.meetings.get(&meeting_id).map_err(|e| e.to_string())?;
        if exists.is_none() {
            return Err(format!("Meeting not found: {}", meeting_id));
        }
        repos
            .meetings
            .update_status(&meeting_id, MeetingStatus::Processing, None)
            .map_err(|e| e.to_string())?;
    }

    let meeting_id_for_task = meeting_id.clone();
    let app_for_task = app.clone();
    tokio::spawn(async move {
        let processing_outcome = tokio::task::spawn_blocking({
            let app_for_worker = app_for_task.clone();
            let transcription_for_worker = transcription.clone();
            let storage_for_worker = storage.clone();
            let meeting_id_for_worker = meeting_id_for_task.clone();
            let mic_path_for_worker = mic_path.clone();
            let system_path_for_worker = system_path.clone();
            let options_for_worker = options.clone();

            move || {
                process_meeting_blocking(
                    &app_for_worker,
                    &transcription_for_worker,
                    &storage_for_worker,
                    meeting_id_for_worker,
                    mic_path_for_worker,
                    system_path_for_worker,
                    options_for_worker,
                )
            }
        })
        .await;

        match processing_outcome {
            Ok(Ok(result)) => {
                if let Err(e) = update_meeting_status(&storage, &meeting_id_for_task, MeetingStatus::Ready, None)
                {
                    warn!(
                        "Failed to update meeting {} status to ready after processing: {}",
                        meeting_id_for_task, e
                    );
                }

                emit_processing_finished(
                    &app_for_task,
                    MeetingProcessingFinishedEvent {
                        meeting_id: meeting_id_for_task,
                        success: true,
                        segment_count: Some(result.transcript.len()),
                        processing_time_ms: Some(result.processing_time_ms),
                        error_message: None,
                    },
                );
            }
            Ok(Err(err_msg)) => {
                if let Err(e) = update_meeting_status(
                    &storage,
                    &meeting_id_for_task,
                    MeetingStatus::Error,
                    Some(err_msg.clone()),
                ) {
                    warn!(
                        "Failed to update meeting {} status to error: {}",
                        meeting_id_for_task, e
                    );
                }
                emit_processing_finished(
                    &app_for_task,
                    MeetingProcessingFinishedEvent {
                        meeting_id: meeting_id_for_task,
                        success: false,
                        segment_count: None,
                        processing_time_ms: None,
                        error_message: Some(err_msg),
                    },
                );
            }
            Err(join_err) => {
                let err_msg = format!("Background processing task failed: {}", join_err);
                error!("{}", err_msg);

                if let Err(e) = update_meeting_status(
                    &storage,
                    &meeting_id_for_task,
                    MeetingStatus::Error,
                    Some(err_msg.clone()),
                ) {
                    warn!(
                        "Failed to update meeting {} status after join error: {}",
                        meeting_id_for_task, e
                    );
                }
                emit_processing_finished(
                    &app_for_task,
                    MeetingProcessingFinishedEvent {
                        meeting_id: meeting_id_for_task,
                        success: false,
                        segment_count: None,
                        processing_time_ms: None,
                        error_message: Some(err_msg),
                    },
                );
            }
        }
    });

    Ok(())
}

/// Fetch a rolling transcript preview while recording (semi-realtime).
#[tauri::command]
pub async fn get_live_transcription_preview(
    session: tauri::State<'_, SharedRecordingSession>,
    transcription: tauri::State<'_, SharedTranscriptionService>,
    meeting_id: String,
    options: Option<LivePreviewOptions>,
) -> Result<LiveTranscriptPreview, String> {
    use crate::audio::{RecordingState, WHISPER_SAMPLE_RATE};
    use crate::inference::{merge_transcripts, Speaker};

    if !transcription.is_ready() {
        return Err("Transcription engine not initialized".to_string());
    }

    let window_seconds = options
        .as_ref()
        .and_then(|opts| opts.window_seconds)
        .unwrap_or(14)
        .clamp(4, 30);
    let include_system_audio = options
        .as_ref()
        .and_then(|opts| opts.include_system_audio)
        .unwrap_or(true);
    let max_samples = window_seconds as usize * WHISPER_SAMPLE_RATE as usize;

    let (duration_ms, mic_samples, system_samples) = {
        let session_guard = session.inner().lock();
        if session_guard.state != RecordingState::Recording {
            return Err("Live preview is only available while recording".to_string());
        }
        if session_guard.id != meeting_id {
            return Err("Meeting is not the active recording session".to_string());
        }

        let duration_ms = session_guard
            .start_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let mic_samples = session_guard
            .buffers
            .mic_preview
            .peek_latest_samples(max_samples);
        let system_samples = if include_system_audio {
            session_guard
                .buffers
                .system_preview
                .peek_latest_samples(max_samples)
        } else {
            Vec::new()
        };

        (duration_ms, mic_samples, system_samples)
    };

    let window_samples = mic_samples.len().max(system_samples.len());
    let window_ms = ((window_samples as f32 / WHISPER_SAMPLE_RATE as f32) * 1000.0).round() as u64;
    let window_start_ms = duration_ms.saturating_sub(window_ms);

    if mic_samples.is_empty() && system_samples.is_empty() {
        return Ok(LiveTranscriptPreview {
            meeting_id,
            duration_ms,
            window_start_ms,
            segments: Vec::new(),
        });
    }

    let transcription = transcription.inner().clone();
    let (mut mic_segments, mut system_segments) = tokio::task::spawn_blocking(move || {
        let mic_segments = if mic_samples.is_empty() {
            Vec::new()
        } else {
            transcription
                .transcribe_samples_with_speaker(mic_samples, Speaker::You)
                .map_err(|e| e.to_string())?
        };

        let system_segments = if system_samples.is_empty() {
            Vec::new()
        } else {
            transcription
                .transcribe_samples_with_speaker(system_samples, Speaker::Others)
                .map_err(|e| e.to_string())?
        };

        Ok::<(Vec<TranscriptSegment>, Vec<TranscriptSegment>), String>((mic_segments, system_segments))
    })
    .await
    .map_err(|e| format!("Live preview task failed: {}", e))??;

    apply_segment_offset(&mut mic_segments, window_start_ms);
    apply_segment_offset(&mut system_segments, window_start_ms);
    let merged = merge_transcripts(mic_segments, system_segments);

    Ok(LiveTranscriptPreview {
        meeting_id,
        duration_ms,
        window_start_ms,
        segments: merged,
    })
}

fn update_meeting_status(
    storage: &SharedStorageState,
    meeting_id: &str,
    status: MeetingStatus,
    error_message: Option<String>,
) -> Result<(), String> {
    let storage_guard = storage.lock();
    storage_guard
        .repositories()
        .meetings
        .update_status(meeting_id, status, error_message.as_deref())
        .map_err(|e| e.to_string())
}

fn emit_processing_finished(app: &AppHandle, payload: MeetingProcessingFinishedEvent) {
    if let Err(e) = app.emit("meeting-processing-finished", &payload) {
        warn!("Failed to emit meeting-processing-finished event: {}", e);
    }
}

fn apply_segment_offset(segments: &mut [TranscriptSegment], offset_ms: u64) {
    if offset_ms == 0 {
        return;
    }
    for segment in segments.iter_mut() {
        segment.start_ms = segment.start_ms.saturating_add(offset_ms);
        segment.end_ms = segment.end_ms.saturating_add(offset_ms);
    }
}

fn derive_cleaned_mic_path(requested_mic_path: &Path) -> Option<PathBuf> {
    let extension = requested_mic_path.extension()?.to_str()?;
    if !extension.eq_ignore_ascii_case("wav") {
        return None;
    }

    let stem = requested_mic_path.file_stem()?.to_str()?;
    if stem.ends_with("_clean") {
        return Some(requested_mic_path.to_path_buf());
    }

    let cleaned_file_name = format!("{stem}_clean.wav");
    Some(requested_mic_path.with_file_name(cleaned_file_name))
}

fn resolve_mic_processing_input_path(requested_mic_path: &Path) -> PathBuf {
    let Some(extension) = requested_mic_path.extension().and_then(|ext| ext.to_str()) else {
        return requested_mic_path.to_path_buf();
    };
    if !extension.eq_ignore_ascii_case("wav") {
        return requested_mic_path.to_path_buf();
    }

    let Some(stem) = requested_mic_path
        .file_stem()
        .and_then(|value| value.to_str())
    else {
        return requested_mic_path.to_path_buf();
    };

    if !stem.ends_with("_clean") {
        return requested_mic_path.to_path_buf();
    }

    let raw_stem = stem.trim_end_matches("_clean");
    if raw_stem.is_empty() {
        return requested_mic_path.to_path_buf();
    }

    let raw_candidate = requested_mic_path.with_file_name(format!("{raw_stem}.wav"));
    if raw_candidate.exists() {
        return raw_candidate;
    }

    requested_mic_path.to_path_buf()
}

fn resolve_existing_clean_mic_path(requested_mic_path: &Path) -> Option<PathBuf> {
    let requested_exists = requested_mic_path.exists();
    if requested_exists && is_clean_mic_path(requested_mic_path) {
        return Some(requested_mic_path.to_path_buf());
    }

    let candidate = derive_cleaned_mic_path(requested_mic_path)?;
    if candidate.exists() {
        return Some(candidate);
    }

    None
}

fn resolve_mic_processing_path(requested_mic_path: &Path) -> PathBuf {
    let preferred_path = resolve_mic_processing_input_path(requested_mic_path);
    if preferred_path.exists() {
        return preferred_path;
    }
    resolve_existing_clean_mic_path(requested_mic_path).unwrap_or(preferred_path)
}

fn is_clean_mic_path(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    if !extension.eq_ignore_ascii_case("wav") {
        return false;
    }

    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    stem.ends_with("_clean")
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|v| v * v).sum();
    (sum_sq / samples.len() as f32).sqrt()
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

/// Delete a transcription model (auto-unloads if currently loaded)
#[tauri::command]
pub fn delete_transcription_model(
    transcription: tauri::State<'_, SharedTranscriptionService>,
    model_manager: tauri::State<'_, SharedModelManager>,
    backend: TranscriptionBackend,
) -> Result<(), String> {
    // Auto-unload if this backend is currently loaded
    if transcription.is_ready() && transcription.backend() == backend {
        info!(
            "Unloading transcription model before deletion: {:?}",
            backend
        );
        transcription.unload();
    }

    // Get the model path
    let model_path = {
        let manager = model_manager.lock();
        manager.get_model_path(backend)
    };

    // Delete the model directory
    if model_path.exists() {
        info!("Deleting transcription model at {:?}", model_path);
        std::fs::remove_dir_all(&model_path)
            .map_err(|e| format!("Failed to delete model: {}", e))?;
    }

    // Update status in model manager
    {
        let manager = model_manager.lock();
        let model_id = backend.model_info().id;
        manager.set_status(&model_id, ModelStatus::NotDownloaded);
    }

    info!("Transcription model {:?} deleted successfully", backend);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_segment_offset, derive_cleaned_mic_path, resolve_existing_clean_mic_path,
        resolve_mic_processing_input_path, resolve_mic_processing_path,
    };
    use crate::inference::Speaker;
    use crate::inference::TranscriptSegment;
    use tempfile::TempDir;

    #[test]
    fn apply_segment_offset_shifts_timestamps() {
        let mut segments = vec![
            TranscriptSegment {
                start_ms: 100,
                end_ms: 400,
                text: "hello".to_string(),
                speaker: Speaker::You,
                confidence: Some(0.9),
            },
            TranscriptSegment {
                start_ms: 500,
                end_ms: 900,
                text: "world".to_string(),
                speaker: Speaker::Others,
                confidence: Some(0.8),
            },
        ];

        apply_segment_offset(&mut segments, 250);
        assert_eq!(segments[0].start_ms, 350);
        assert_eq!(segments[0].end_ms, 650);
        assert_eq!(segments[1].start_ms, 750);
        assert_eq!(segments[1].end_ms, 1150);
    }

    #[test]
    fn derive_cleaned_path_from_raw_wav() {
        let raw = std::path::Path::new("/tmp/meeting/you.wav");
        let cleaned = derive_cleaned_mic_path(raw).unwrap();
        assert_eq!(cleaned, std::path::Path::new("/tmp/meeting/you_clean.wav"));
    }

    #[test]
    fn derive_cleaned_path_keeps_existing_clean_name() {
        let cleaned = std::path::Path::new("/tmp/meeting/you_clean.wav");
        let derived = derive_cleaned_mic_path(cleaned).unwrap();
        assert_eq!(derived, cleaned);
    }

    #[test]
    fn resolve_processing_path_prefers_raw_when_requested_is_clean() {
        let temp = TempDir::new().unwrap();
        let raw_path = temp.path().join("you.wav");
        let clean_path = temp.path().join("you_clean.wav");
        std::fs::write(&raw_path, b"raw").unwrap();
        std::fs::write(&clean_path, b"clean").unwrap();

        let resolved = resolve_mic_processing_input_path(&clean_path);
        assert_eq!(resolved, raw_path);
    }

    #[test]
    fn resolve_processing_path_uses_requested_when_raw_missing() {
        let temp = TempDir::new().unwrap();
        let clean_path = temp.path().join("you_clean.wav");
        std::fs::write(&clean_path, b"clean").unwrap();

        let resolved = resolve_mic_processing_input_path(&clean_path);
        assert_eq!(resolved, clean_path);
    }

    #[test]
    fn resolve_existing_clean_path_uses_requested_clean_file() {
        let temp = TempDir::new().unwrap();
        let clean_path = temp.path().join("you_clean.wav");
        std::fs::write(&clean_path, b"clean").unwrap();

        let resolved = resolve_existing_clean_mic_path(&clean_path).unwrap();
        assert_eq!(resolved, clean_path);
    }

    #[test]
    fn resolve_existing_clean_path_finds_sibling_clean_file() {
        let temp = TempDir::new().unwrap();
        let raw_path = temp.path().join("you.wav");
        let clean_path = temp.path().join("you_clean.wav");
        std::fs::write(&raw_path, b"raw").unwrap();
        std::fs::write(&clean_path, b"clean").unwrap();

        let resolved = resolve_existing_clean_mic_path(&raw_path).unwrap();
        assert_eq!(resolved, clean_path);
    }

    #[test]
    fn resolve_existing_clean_path_returns_none_without_clean_file() {
        let temp = TempDir::new().unwrap();
        let raw_path = temp.path().join("you.wav");
        std::fs::write(&raw_path, b"raw").unwrap();

        let resolved = resolve_existing_clean_mic_path(&raw_path);
        assert!(resolved.is_none());
    }

    #[test]
    fn resolve_processing_path_prefers_raw_when_both_exist() {
        let temp = TempDir::new().unwrap();
        let raw_path = temp.path().join("you.wav");
        let clean_path = temp.path().join("you_clean.wav");
        std::fs::write(&raw_path, b"raw").unwrap();
        std::fs::write(&clean_path, b"clean").unwrap();

        let resolved = resolve_mic_processing_path(&raw_path);
        assert_eq!(resolved, raw_path);
    }

    #[test]
    fn resolve_processing_path_falls_back_to_clean_when_raw_missing() {
        let temp = TempDir::new().unwrap();
        let raw_path = temp.path().join("you.wav");
        let clean_path = temp.path().join("you_clean.wav");
        std::fs::write(&clean_path, b"clean").unwrap();

        let resolved = resolve_mic_processing_path(&raw_path);
        assert_eq!(resolved, clean_path);
    }
}
