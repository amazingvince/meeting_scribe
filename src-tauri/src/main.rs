// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use meeting_scribe_lib::commands::{
    RecordingSession, SharedEmbeddingService, SharedModelManager, SharedRecordingSession,
    SharedStorageState, SharedTranscriptionService,
};
use meeting_scribe_lib::inference::TranscriptionService;
use meeting_scribe_lib::models::ModelManager;
use meeting_scribe_lib::storage::initialize_storage;
use meeting_scribe_lib::{commands, AppConfig};
use parking_lot::Mutex;
use std::sync::Arc;
#[cfg(debug_assertions)]
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
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

    // Initialize storage (SQLite + LanceDB)
    let storage_state = initialize_storage(&config.data_dir)
        .await
        .expect("Failed to initialize storage");
    let storage_state: SharedStorageState = Arc::new(Mutex::new(storage_state));

    // Initialize embedding service (lazy-loaded)
    let embedding_service: SharedEmbeddingService = Arc::new(Mutex::new(None));

    info!("Models directory: {:?}", config.models_dir);
    info!("Data directory: {:?}", config.data_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(config)
        .manage(recording_session)
        .manage(model_manager)
        .manage(transcription_service)
        .manage(storage_state)
        .manage(embedding_service)
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
            // Storage commands
            commands::storage::create_meeting,
            commands::storage::get_meeting,
            commands::storage::list_meetings,
            commands::storage::update_meeting,
            commands::storage::update_meeting_status,
            commands::storage::delete_meeting,
            commands::storage::count_meetings,
            commands::storage::get_transcript,
            commands::storage::save_transcript,
            commands::storage::get_transcript_text,
            commands::storage::delete_transcript,
            commands::storage::search_transcripts,
            commands::storage::search_transcripts_with_snippets,
            commands::storage::search_in_meeting,
            commands::storage::get_database_stats,
            commands::storage::get_storage_stats,
            // Embedding commands
            commands::embedding::initialize_embedding,
            commands::embedding::is_embedding_ready,
            commands::embedding::is_embedding_downloaded,
            commands::embedding::embed_text,
            commands::embedding::embed_meeting_transcript,
            commands::embedding::calculate_similarity,
            commands::embedding::get_embedding_info,
            commands::embedding::semantic_search,
            commands::embedding::unload_embedding,
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
