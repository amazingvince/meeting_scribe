//! Recording-related Tauri commands

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::audio::buffer::AudioBufferManager;
use crate::audio::capture::{load_wav, AudioRecorder};
use crate::audio::pipeline::{AudioPipeline, PipelineConfig};
use crate::audio::vad::SpeechSegment;
use crate::audio::waveform::calculate_waveform;
use crate::audio::{RecordingState, WAVEFORM_UPDATE_MS, WHISPER_SAMPLE_RATE};
use crate::AppConfig;

/// Recording session state (only thread-safe data)
pub struct RecordingSession {
    pub id: String,
    pub state: RecordingState,
    pub start_time: Option<Instant>,
    pub mic_recorder: Option<AudioRecorder>,
    pub mic_clean_recorder: Option<AudioRecorder>,
    pub system_recorder: Option<AudioRecorder>,
    pub buffers: Arc<AudioBufferManager>,
    /// Channel to signal stop to capture threads
    pub stop_tx: Option<oneshot::Sender<()>>,
}

impl RecordingSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            state: RecordingState::Idle,
            start_time: None,
            mic_recorder: None,
            mic_clean_recorder: None,
            system_recorder: None,
            buffers: Arc::new(AudioBufferManager::new()),
            stop_tx: None,
        }
    }
}

impl Default for RecordingSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared recording state type
pub type SharedRecordingSession = Arc<Mutex<RecordingSession>>;

/// Capture initialization result
struct CaptureInitResult {
    /// Whether microphone capture started successfully
    mic_ok: bool,
    /// Whether system audio capture started (optional, platform-dependent)
    system_ok: bool,
    /// Error message if system audio failed to initialize/start
    system_error: Option<String>,
    /// Error message if mic failed
    error: Option<String>,
}

/// System-audio capture preferences.
/// `backend` is interpreted on macOS only; `loopback_device` can be used as a
/// monitor/loopback device hint on supported platforms.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MacSystemAudioSettings {
    /// Backend selection: `auto` | `process_tap` | `loopback`
    pub backend: Option<String>,
    /// Optional preferred loopback device name (used by loopback fallback/backend).
    pub loopback_device: Option<String>,
}

/// Options accepted when starting a recording session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StartRecordingOptions {
    /// Optional macOS system-audio settings.
    pub mac_system_audio: Option<MacSystemAudioSettings>,
}

/// Start recording
#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    session: tauri::State<'_, SharedRecordingSession>,
    config: tauri::State<'_, AppConfig>,
    options: Option<StartRecordingOptions>,
) -> Result<String, String> {
    let session_arc = session.inner().clone();

    // Check if already recording (brief lock)
    {
        let session_guard = session_arc.lock();
        if session_guard.state != RecordingState::Idle {
            return Err(format!(
                "Cannot start recording while session is {:?}",
                session_guard.state
            ));
        }
    }

    // Create new session
    let meeting_id = Uuid::new_v4().to_string();
    let meeting_dir = config.audio_dir.join(&meeting_id);
    std::fs::create_dir_all(&meeting_dir).map_err(|e| e.to_string())?;

    // Initialize recorders
    let mic_recorder =
        AudioRecorder::new(meeting_dir.join("you.wav")).map_err(|e| e.to_string())?;
    let mic_clean_recorder =
        AudioRecorder::new(meeting_dir.join("you_clean.wav")).map_err(|e| e.to_string())?;
    let system_recorder =
        AudioRecorder::new(meeting_dir.join("others.wav")).map_err(|e| e.to_string())?;

    // Create new buffers
    let buffers = Arc::new(AudioBufferManager::new());

    // Create stop channel
    let (stop_tx, stop_rx) = oneshot::channel();

    // Create channel for capture thread to report initialization result
    let (init_tx, init_rx) = mpsc::channel::<CaptureInitResult>();

    // Spawn capture thread (runs audio capture in a separate thread to handle non-Send streams)
    let buffers_for_capture = buffers.clone();
    let capture_options = options.clone();
    std::thread::spawn(move || {
        run_capture_thread(buffers_for_capture, stop_rx, init_tx, capture_options);
    });

    // Wait for capture initialization result (with timeout)
    let init_result = init_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "Capture thread initialization timed out".to_string())?;

    // Check if microphone capture started successfully
    if !init_result.mic_ok {
        // Clean up the meeting directory since recording failed
        let _ = std::fs::remove_dir_all(&meeting_dir);
        return Err(init_result
            .error
            .unwrap_or_else(|| "Failed to start microphone capture".to_string()));
    }

    // If user explicitly selected a specific macOS backend, treat system-audio
    // startup failure as fatal so they get immediate actionable feedback.
    if !init_result.system_ok {
        #[cfg(target_os = "macos")]
        {
            let explicit_backend = options
                .as_ref()
                .and_then(|o| o.mac_system_audio.as_ref())
                .and_then(|s| s.backend.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_ascii_lowercase);

            let requires_system_audio = matches!(
                explicit_backend.as_deref(),
                Some("process_tap")
                    | Some("process-tap")
                    | Some("tap")
                    | Some("loopback")
                    | Some("loopback_input")
                    | Some("loopback-input")
            );
            let auto_backend = matches!(explicit_backend.as_deref(), None | Some("auto"));

            if requires_system_audio || auto_backend {
                let _ = stop_tx.send(());
                let _ = std::fs::remove_dir_all(&meeting_dir);
                return Err(init_result.system_error.unwrap_or_else(|| {
                    "System audio capture failed to start for selected backend".to_string()
                }));
            }
        }

        warn!("System audio capture not available - only microphone will be recorded");
        if let Some(system_error) = init_result.system_error {
            warn!("System audio failure detail: {}", system_error);
        }
    }

    // Update session state (only after successful capture init)
    {
        let mut session_guard = session_arc.lock();
        session_guard.id = meeting_id.clone();
        session_guard.state = RecordingState::Recording;
        session_guard.start_time = Some(Instant::now());
        session_guard.mic_recorder = Some(mic_recorder);
        session_guard.mic_clean_recorder = Some(mic_clean_recorder);
        session_guard.system_recorder = Some(system_recorder);
        session_guard.buffers = buffers.clone();
        session_guard.stop_tx = Some(stop_tx);
    }

    // Start waveform emission task
    let session_clone = session_arc.clone();
    let app_clone = app.clone();

    tokio::spawn(async move {
        emit_waveform_loop(app_clone, session_clone).await;
    });

    info!("Recording started: {}", meeting_id);
    let _ = app.emit(
        "recording-state-changed",
        RecordingStateChangedEvent {
            state: RecordingState::Recording,
            meeting_id: Some(meeting_id.clone()),
            duration_ms: 0,
        },
    );
    Ok(meeting_id)
}

/// Run audio capture in a dedicated thread (because cpal streams are not Send)
fn run_capture_thread(
    buffers: Arc<AudioBufferManager>,
    mut stop_rx: oneshot::Receiver<()>,
    init_tx: mpsc::Sender<CaptureInitResult>,
    options: Option<StartRecordingOptions>,
) {
    use crate::audio::aec::{EchoCancellationBackend, RealtimeEchoCanceller};
    use crate::audio::capture::AudioCapture;
    use crate::audio::platform::{system_audio_backend_capabilities, SystemAudioCapture};

    let system_audio_caps = system_audio_backend_capabilities();
    apply_system_audio_device_override(options.as_ref());

    #[cfg(target_os = "macos")]
    if let Some(mac_settings) = options.as_ref().and_then(|o| o.mac_system_audio.as_ref()) {
        apply_macos_system_audio_settings(mac_settings);
    } else {
        clear_macos_system_audio_settings();
    }

    // Initialize microphone capture
    let mut mic_capture = match AudioCapture::new_microphone() {
        Ok(c) => Some(c),
        Err(e) => {
            error!("Failed to initialize microphone: {}", e);
            // Report failure and exit thread
            let _ = init_tx.send(CaptureInitResult {
                mic_ok: false,
                system_ok: false,
                system_error: None,
                error: Some(format!("Failed to initialize microphone: {}", e)),
            });
            return;
        }
    };

    // Initialize system audio capture (optional)
    let mut system_error: Option<String> = None;
    let mut system_capture = match SystemAudioCapture::new() {
        Ok(c) => Some(c),
        Err(e) => {
            let message = format!(
                "System audio capture unavailable (backend={}, supported={}): {}. {}",
                system_audio_caps.backend_name,
                system_audio_caps.supported,
                e,
                system_audio_caps.requirements
            );
            info!(
                "System audio capture not available (backend={}, supported={}): {}. {}",
                system_audio_caps.backend_name,
                system_audio_caps.supported,
                e,
                system_audio_caps.requirements
            );
            system_error = Some(message);
            None
        }
    };

    // Start microphone capture
    if let Some(ref mut mic) = mic_capture {
        if let Err(e) = mic.start() {
            error!("Failed to start microphone capture: {}", e);
            // Report failure and exit thread
            let _ = init_tx.send(CaptureInitResult {
                mic_ok: false,
                system_ok: false,
                system_error: None,
                error: Some(format!("Failed to start microphone capture: {}", e)),
            });
            return;
        }
    }

    // Start system audio capture (optional)
    let system_ok = if let Some(ref mut system) = system_capture {
        match system.start() {
            Ok(()) => true,
            Err(e) => {
                system_error = Some(format!(
                    "System audio capture failed to start (backend={}): {}",
                    system_audio_caps.backend_name, e
                ));
                info!(
                    "System audio capture failed to start (backend={}): {}",
                    system_audio_caps.backend_name, e
                );
                false
            }
        }
    } else {
        false
    };

    // Report successful initialization
    let _ = init_tx.send(CaptureInitResult {
        mic_ok: true,
        system_ok,
        system_error,
        error: None,
    });

    let requested_echo_backend = {
        #[cfg(target_os = "macos")]
        {
            std::env::var("MEETING_SCRIBE_REALTIME_ECHO_BACKEND")
                .ok()
                .and_then(|value| EchoCancellationBackend::parse(&value))
                .unwrap_or(EchoCancellationBackend::WebRtcAec3)
        }
        #[cfg(not(target_os = "macos"))]
        {
            crate::audio::aec::resolve_echo_backend(None)
        }
    };
    let mut realtime_aec = match RealtimeEchoCanceller::new(
        WHISPER_SAMPLE_RATE,
        requested_echo_backend,
    ) {
        Ok(processor) => {
            info!(
                "Real-time mic cleanup enabled: requested_backend={}, used_backend={}, fallback={}",
                requested_echo_backend.as_str(),
                processor.backend_used().as_str(),
                processor.fallback_used()
            );
            Some(processor)
        }
        Err(e) => {
            warn!(
                "Real-time mic cleanup unavailable (requested_backend={}): {}. Falling back to mic passthrough.",
                requested_echo_backend.as_str(),
                e
            );
            None
        }
    };

    // Run capture loop - transfer samples from capture buffers to shared buffers
    let transfer_interval = std::time::Duration::from_millis(20);
    loop {
        // Check if stop signal received
        match stop_rx.try_recv() {
            Ok(_) | Err(oneshot::error::TryRecvError::Closed) => break,
            Err(oneshot::error::TryRecvError::Empty) => {}
        }

        let mic_samples = if let Some(ref mic) = mic_capture {
            mic.buffer().drain_samples()
        } else {
            Vec::new()
        };

        let system_samples = if let Some(ref system) = system_capture {
            system.buffer().drain_samples()
        } else {
            Vec::new()
        };

        if !mic_samples.is_empty() {
            buffers.mic.push_samples(&mic_samples);
        }

        if !system_samples.is_empty() {
            buffers.system.push_samples(&system_samples);
            buffers.system_preview.push_samples(&system_samples);
        }

        if !mic_samples.is_empty() {
            let cleaned_samples = if let Some(ref mut processor) = realtime_aec {
                processor.process_chunk(&mic_samples, &system_samples)
            } else {
                mic_samples.clone()
            };

            if !cleaned_samples.is_empty() {
                buffers.mic_clean.push_samples(&cleaned_samples);
                buffers.mic_preview.push_samples(&cleaned_samples);
            }
        }

        std::thread::sleep(transfer_interval);
    }

    if let Some(ref mut processor) = realtime_aec {
        let flushed = processor.flush();
        if !flushed.is_empty() {
            buffers.mic_clean.push_samples(&flushed);
            buffers.mic_preview.push_samples(&flushed);
        }
    }

    // Stop captures
    if let Some(ref mut mic) = mic_capture {
        mic.stop();
    }
    if let Some(ref mut system) = system_capture {
        system.stop();
    }

    info!("Capture thread stopped");
}

fn apply_system_audio_device_override(options: Option<&StartRecordingOptions>) {
    const DEVICE_ENV: &str = "MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE";

    if let Some(device) = options
        .and_then(|o| o.mac_system_audio.as_ref())
        .and_then(|s| s.loopback_device.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        std::env::set_var(DEVICE_ENV, device);
    } else {
        std::env::remove_var(DEVICE_ENV);
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_system_audio_settings(settings: &MacSystemAudioSettings) {
    const BACKEND_ENV: &str = "MEETING_SCRIBE_MACOS_SYSTEM_AUDIO_BACKEND";

    if let Some(backend) = settings
        .backend
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        std::env::set_var(BACKEND_ENV, backend);
    } else {
        std::env::remove_var(BACKEND_ENV);
    }
}

#[cfg(target_os = "macos")]
fn clear_macos_system_audio_settings() {
    const BACKEND_ENV: &str = "MEETING_SCRIBE_MACOS_SYSTEM_AUDIO_BACKEND";

    std::env::remove_var(BACKEND_ENV);
}

/// Stop recording
#[tauri::command]
pub async fn stop_recording(
    app: AppHandle,
    session: tauri::State<'_, SharedRecordingSession>,
) -> Result<RecordingResult, String> {
    let session_arc = session.inner().clone();

    // Take ownership of resources without holding the lock across blocking work.
    let (
        meeting_id,
        start_time,
        buffers,
        mut mic_recorder,
        mut mic_clean_recorder,
        mut system_recorder,
        stop_tx,
    ) = {
        let mut session_guard = session_arc.lock();

        if session_guard.state != RecordingState::Recording {
            return Err("Not currently recording".to_string());
        }

        // Mark as processing so the waveform loop exits promptly.
        session_guard.state = RecordingState::Processing;

        (
            session_guard.id.clone(),
            session_guard.start_time,
            session_guard.buffers.clone(),
            session_guard.mic_recorder.take(),
            session_guard.mic_clean_recorder.take(),
            session_guard.system_recorder.take(),
            session_guard.stop_tx.take(),
        )
    };

    let reset_session = || {
        let mut session_guard = session_arc.lock();
        session_guard.state = RecordingState::Idle;
        session_guard.start_time = None;
        session_guard.mic_clean_recorder = None;
        session_guard.buffers = Arc::new(AudioBufferManager::new());
    };

    // Signal capture thread to stop
    if let Some(stop_tx) = stop_tx {
        let _ = stop_tx.send(());
    }

    // Give capture thread time to finish transferring samples
    std::thread::sleep(Duration::from_millis(100));

    // Drain buffers to recorders (drain twice to catch trailing samples)
    let mic_samples = buffers.mic.drain_samples();
    if let Some(ref mut recorder) = mic_recorder {
        recorder.write_samples(&mic_samples).map_err(|e| {
            reset_session();
            e.to_string()
        })?;
    }

    let mic_clean_samples = buffers.mic_clean.drain_samples();
    if let Some(ref mut recorder) = mic_clean_recorder {
        recorder.write_samples(&mic_clean_samples).map_err(|e| {
            reset_session();
            e.to_string()
        })?;
    }

    let system_samples = buffers.system.drain_samples();
    if let Some(ref mut recorder) = system_recorder {
        recorder.write_samples(&system_samples).map_err(|e| {
            reset_session();
            e.to_string()
        })?;
    }

    std::thread::sleep(Duration::from_millis(20));

    let mic_samples = buffers.mic.drain_samples();
    if let Some(ref mut recorder) = mic_recorder {
        recorder.write_samples(&mic_samples).map_err(|e| {
            reset_session();
            e.to_string()
        })?;
    }

    let mic_clean_samples = buffers.mic_clean.drain_samples();
    if let Some(ref mut recorder) = mic_clean_recorder {
        recorder.write_samples(&mic_clean_samples).map_err(|e| {
            reset_session();
            e.to_string()
        })?;
    }

    let system_samples = buffers.system.drain_samples();
    if let Some(ref mut recorder) = system_recorder {
        recorder.write_samples(&system_samples).map_err(|e| {
            reset_session();
            e.to_string()
        })?;
    }

    // Finalize WAV files
    let mic_path: Option<PathBuf> = mic_recorder
        .take()
        .map(|r| r.finalize())
        .transpose()
        .map_err(|e| {
            reset_session();
            e.to_string()
        })?;

    let mic_clean_path: Option<PathBuf> = mic_clean_recorder
        .take()
        .map(|r| r.finalize())
        .transpose()
        .map_err(|e| {
            reset_session();
            e.to_string()
        })?;

    let system_path: Option<PathBuf> = system_recorder
        .take()
        .map(|r| r.finalize())
        .transpose()
        .map_err(|e| {
            reset_session();
            e.to_string()
        })?;

    let cleaned_has_audio = mic_clean_path
        .as_ref()
        .and_then(|path| std::fs::metadata(path).ok())
        .map(|meta| meta.len() > 44)
        .unwrap_or(false);
    let playback_mic_path = if cleaned_has_audio {
        mic_clean_path.clone()
    } else {
        if let Some(path) = mic_clean_path.as_ref() {
            let _ = std::fs::remove_file(path);
        }
        mic_path.clone()
    };

    let duration_ms = start_time
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);

    let result = RecordingResult {
        meeting_id: meeting_id.clone(),
        duration_ms,
        mic_path: playback_mic_path.map(|p| p.display().to_string()),
        system_path: system_path.map(|p| p.display().to_string()),
    };

    // Reset session after successful finalize
    reset_session();

    info!(
        "Recording stopped: {} ({}ms)",
        result.meeting_id, duration_ms
    );
    let _ = app.emit(
        "recording-state-changed",
        RecordingStateChangedEvent {
            state: RecordingState::Idle,
            meeting_id: None,
            duration_ms: 0,
        },
    );
    Ok(result)
}

/// Get current recording state
#[tauri::command]
pub fn get_recording_state(
    session: tauri::State<'_, SharedRecordingSession>,
) -> RecordingStateResponse {
    let session_guard = session.inner().lock();

    let duration_ms = session_guard
        .start_time
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);

    RecordingStateResponse {
        state: session_guard.state,
        meeting_id: if session_guard.state != RecordingState::Idle {
            Some(session_guard.id.clone())
        } else {
            None
        },
        duration_ms,
    }
}

/// List available audio devices
#[tauri::command]
pub fn list_audio_devices() -> Result<AudioDevices, String> {
    let input_devices = crate::audio::capture::list_input_devices().map_err(|e| e.to_string())?;
    let output_devices = crate::audio::capture::list_output_devices().map_err(|e| e.to_string())?;

    Ok(AudioDevices {
        input_devices,
        output_devices,
    })
}

/// Preprocess a recorded meeting (VAD + optional denoising)
#[tauri::command]
pub async fn preprocess_meeting(
    meeting_id: String,
    denoise: Option<bool>,
    config: tauri::State<'_, AppConfig>,
) -> Result<PreprocessingInfo, String> {
    let meeting_dir = config.audio_dir.join(&meeting_id);

    if !meeting_dir.exists() {
        return Err(format!("Meeting not found: {}", meeting_id));
    }

    // Load audio files
    let mic_path = meeting_dir.join("you.wav");
    let system_path = meeting_dir.join("others.wav");

    let mut results = PreprocessingInfo {
        meeting_id: meeting_id.clone(),
        mic_segments: Vec::new(),
        system_segments: Vec::new(),
        mic_speech_ratio: 0.0,
        system_speech_ratio: 0.0,
        mic_duration_ms: 0,
        system_duration_ms: 0,
    };

    // Create pipeline with config
    let pipeline_config = PipelineConfig {
        denoise_enabled: denoise.unwrap_or(true),
        ..PipelineConfig::default()
    };

    let mut pipeline = AudioPipeline::new(pipeline_config).map_err(|e| e.to_string())?;

    // Process mic audio
    if mic_path.exists() {
        match load_wav(&mic_path) {
            Ok(samples) => {
                let result = pipeline.process(&samples, WHISPER_SAMPLE_RATE);
                results.mic_speech_ratio = result.speech_ratio();
                results.mic_duration_ms = result.duration_ms;
                results.mic_segments = result.segments;
                pipeline.reset();
            }
            Err(e) => {
                error!("Failed to load mic audio: {}", e);
            }
        }
    }

    // Process system audio
    if system_path.exists() {
        match load_wav(&system_path) {
            Ok(samples) => {
                let result = pipeline.process(&samples, WHISPER_SAMPLE_RATE);
                results.system_speech_ratio = result.speech_ratio();
                results.system_duration_ms = result.duration_ms;
                results.system_segments = result.segments;
            }
            Err(e) => {
                error!("Failed to load system audio: {}", e);
            }
        }
    }

    info!(
        "Preprocessed meeting {}: mic={} segments ({:.1}%), system={} segments ({:.1}%)",
        meeting_id,
        results.mic_segments.len(),
        results.mic_speech_ratio * 100.0,
        results.system_segments.len(),
        results.system_speech_ratio * 100.0
    );

    Ok(results)
}

// Waveform emission loop
async fn emit_waveform_loop(app: AppHandle, session: SharedRecordingSession) {
    let mut interval = interval(Duration::from_millis(WAVEFORM_UPDATE_MS));

    loop {
        interval.tick().await;

        let waveform = {
            let mut session_guard = session.lock();

            if session_guard.state != RecordingState::Recording {
                break;
            }

            let duration_ms = session_guard
                .start_time
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(0);

            // Read from preview buffers so meter data remains stable and independent
            // of recorder drain timing.
            let mic_samples = session_guard
                .buffers
                .mic_preview
                .peek_latest_samples(WHISPER_SAMPLE_RATE as usize / 20);

            let system_samples = session_guard
                .buffers
                .system_preview
                .peek_latest_samples(WHISPER_SAMPLE_RATE as usize / 20);

            // Also write accumulated samples to recorders periodically
            let mic_to_write = session_guard.buffers.mic.drain_samples();
            if let Some(ref mut recorder) = session_guard.mic_recorder {
                if let Err(e) = recorder.write_samples(&mic_to_write) {
                    error!("Failed to write mic samples: {}", e);
                }
            }

            let mic_clean_to_write = session_guard.buffers.mic_clean.drain_samples();
            if let Some(ref mut recorder) = session_guard.mic_clean_recorder {
                if let Err(e) = recorder.write_samples(&mic_clean_to_write) {
                    error!("Failed to write cleaned mic samples: {}", e);
                }
            }

            let system_to_write = session_guard.buffers.system.drain_samples();
            if let Some(ref mut recorder) = session_guard.system_recorder {
                if let Err(e) = recorder.write_samples(&system_to_write) {
                    error!("Failed to write system samples: {}", e);
                }
            }

            calculate_waveform(&mic_samples, &system_samples, duration_ms, duration_ms)
        };

        // Emit to frontend
        if let Err(e) = app.emit("waveform-update", &waveform) {
            error!("Failed to emit waveform: {}", e);
        }
    }
}

// Response types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub meeting_id: String,
    pub duration_ms: u64,
    pub mic_path: Option<String>,
    pub system_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStateResponse {
    pub state: RecordingState,
    pub meeting_id: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStateChangedEvent {
    pub state: RecordingState,
    pub meeting_id: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevices {
    pub input_devices: Vec<String>,
    pub output_devices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingInfo {
    pub meeting_id: String,
    pub mic_segments: Vec<SpeechSegment>,
    pub system_segments: Vec<SpeechSegment>,
    pub mic_speech_ratio: f32,
    pub system_speech_ratio: f32,
    pub mic_duration_ms: u64,
    pub system_duration_ms: u64,
}
