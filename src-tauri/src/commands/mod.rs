//! Tauri commands - These functions are callable from the frontend via IPC

pub mod embedding;
pub mod llm;
pub mod recording;
pub mod storage;
pub mod transcription;

use serde::{Deserialize, Serialize};

// Re-export recording commands
pub use recording::{
    get_recording_state, list_audio_devices, start_recording, stop_recording, AudioDevices,
    RecordingResult, RecordingSession, RecordingStateResponse, SharedRecordingSession,
};

// Re-export transcription commands
pub use transcription::{
    download_transcription_model, get_model_status, get_models_dir, get_transcription_config,
    get_live_transcription_preview, init_transcription, is_model_downloaded,
    is_transcription_ready, process_meeting, start_meeting_processing, transcribe_file,
    unload_transcription, DownloadProgressEvent, LivePreviewOptions, LiveTranscriptPreview,
    MeetingProcessingFinishedEvent, ModelStatusItem, ModelStatusResponse, SharedModelManager,
    SharedTranscriptionService,
};

// Re-export storage commands
pub use storage::{
    count_meetings, create_meeting, delete_meeting, delete_transcript, get_database_stats,
    get_meeting, get_note, get_notes, get_storage_stats, get_summaries, get_summary,
    get_transcript, get_transcript_text, list_meetings, save_note, save_summary, save_transcript,
    search_in_meeting, search_transcripts, search_transcripts_with_snippets, update_meeting,
    update_meeting_status, SharedStorageState,
};

// Re-export embedding commands
pub use embedding::{
    batch_embed_meetings, calculate_similarity, delete_embedding, embed_meeting_transcript,
    embed_text, get_embedding_info, get_unembedded_meetings, hybrid_search, initialize_embedding,
    is_embedding_downloaded, is_embedding_ready, semantic_search, unload_embedding,
    BatchEmbedProgress, BatchEmbedResult, EmbeddingDownloadProgress, EmbeddingInfo,
    SemanticSearchResult, SharedEmbeddingService, UnembeddedMeeting,
};

// Re-export LLM commands
pub use llm::{
    ask_meeting_question, count_tokens, download_llm, extract_action_items, generate_meeting_title,
    generate_summary, generate_text, get_llm_status, initialize_llm, is_llm_model_downloaded,
    list_llm_models, load_llm_model, stream_meeting_question, unload_llm_model, ChatTokenEvent,
    LlmModelInfo, LlmStatus, SharedLlmService,
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
