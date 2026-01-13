// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use meeting_scribe_lib::commands::{
    RecordingSession, SharedModelManager, SharedRecordingSession, SharedTranscriptionService,
};
use meeting_scribe_lib::inference::TranscriptionService;
use meeting_scribe_lib::models::ModelManager;
use meeting_scribe_lib::{commands, AppConfig};
use parking_lot::Mutex;
use std::sync::Arc;
#[cfg(debug_assertions)]
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meeting_scribe=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Meeting Scribe...");

    // Initialize app config
    let config = AppConfig::new().expect("Failed to create app config");
    config.ensure_dirs().expect("Failed to create directories");

    // Initialize recording session
    let recording_session: SharedRecordingSession = Arc::new(Mutex::new(RecordingSession::new()));

    // Initialize model manager
    let model_manager = ModelManager::new(config.models_dir.clone())
        .expect("Failed to create model manager");
    model_manager.init_status();
    let model_manager: SharedModelManager = Arc::new(Mutex::new(model_manager));

    // Initialize transcription service
    let transcription_service: SharedTranscriptionService =
        Arc::new(TranscriptionService::new());

    info!("Models directory: {:?}", config.models_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(config)
        .manage(recording_session)
        .manage(model_manager)
        .manage(transcription_service)
        .invoke_handler(tauri::generate_handler![
            // Core commands
            commands::greet,
            commands::get_app_info,
            // Recording commands
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::get_recording_state,
            commands::recording::list_audio_devices,
            commands::recording::preprocess_meeting,
            // Transcription commands
            commands::transcription::get_model_status,
            commands::transcription::download_transcription_model,
            commands::transcription::init_transcription,
            commands::transcription::is_transcription_ready,
            commands::transcription::get_transcription_config,
            commands::transcription::transcribe_file,
            commands::transcription::process_meeting,
            commands::transcription::unload_transcription,
            commands::transcription::get_models_dir,
            commands::transcription::is_model_downloaded,
        ])
        .setup(|_app| {
            info!("Application setup complete");

            #[cfg(debug_assertions)]
            {
                let window = _app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
