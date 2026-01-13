//! Tauri commands - These functions are callable from the frontend via IPC

pub mod recording;
pub mod transcription;

use serde::{Deserialize, Serialize};

// Re-export recording commands
pub use recording::{
    get_recording_state, list_audio_devices, start_recording, stop_recording,
    AudioDevices, RecordingResult, RecordingStateResponse, RecordingSession,
    SharedRecordingSession,
};

// Re-export transcription commands
pub use transcription::{
    download_transcription_model, get_model_status, get_models_dir, get_transcription_config,
    init_transcription, is_model_downloaded, is_transcription_ready, process_meeting,
    transcribe_file, unload_transcription, DownloadProgressEvent, ModelStatusItem,
    ModelStatusResponse, SharedModelManager, SharedTranscriptionService,
};

/// Basic greeting command for testing IPC
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Meeting Scribe.", name)
}

/// Application info response
#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub platform: String,
}

/// Get application information
#[tauri::command]
pub fn get_app_info(config: tauri::State<crate::AppConfig>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: config.data_dir.display().to_string(),
        platform: std::env::consts::OS.to_string(),
    }
}
