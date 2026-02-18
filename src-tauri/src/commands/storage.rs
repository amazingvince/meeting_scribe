//! Storage-related Tauri commands
//!
//! Commands for meeting CRUD, transcript storage, and search.

use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{info, warn};

use crate::inference::TranscriptSegment;
use crate::storage::{
    DatabaseStats, ListOptions, Meeting, MeetingStatus, Note, SearchHit, SearchHitWithSnippet,
    StorageStats, StoredSegment, Summary, SummaryType,
};

/// Shared storage state type
pub type SharedStorageState = Arc<Mutex<crate::storage::StorageState>>;

// ============================================
// MEETING COMMANDS
// ============================================

/// Create a new meeting
#[tauri::command]
pub fn create_meeting(
    storage: tauri::State<'_, SharedStorageState>,
    title: Option<String>,
) -> Result<Meeting, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let meeting = if let Some(title) = title {
        Meeting::new(title)
    } else {
        Meeting::default()
    };

    repos.meetings.create(&meeting).map_err(|e| e.to_string())?;

    info!("Created meeting: {} ({})", meeting.title, meeting.id);
    Ok(meeting)
}

/// Create a meeting with a specific ID (used after recording)
#[tauri::command]
pub fn create_meeting_with_id(
    storage: tauri::State<'_, SharedStorageState>,
    meeting: Meeting,
) -> Result<Meeting, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos.meetings.create(&meeting).map_err(|e| e.to_string())?;

    info!(
        "Created meeting with ID: {} ({})",
        meeting.title, meeting.id
    );
    Ok(meeting)
}

/// Get a meeting by ID
#[tauri::command]
pub fn get_meeting(
    storage: tauri::State<'_, SharedStorageState>,
    id: String,
) -> Result<Option<Meeting>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos.meetings.get(&id).map_err(|e| e.to_string())
}

/// List meetings with optional filters
#[tauri::command]
pub fn list_meetings(
    storage: tauri::State<'_, SharedStorageState>,
    status: Option<String>,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Meeting>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let mut options = ListOptions::new();

    if let Some(status_str) = status {
        let status: MeetingStatus = status_str
            .parse()
            .map_err(|e: anyhow::Error| e.to_string())?;
        options = options.with_status(status);
    }

    if let Some(search_str) = search {
        options = options.with_search(search_str);
    }

    if let Some(l) = limit {
        options.limit = l;
    }

    if let Some(o) = offset {
        options.offset = o;
    }

    repos.meetings.list(options).map_err(|e| e.to_string())
}

/// Update a meeting
#[tauri::command]
pub fn update_meeting(
    storage: tauri::State<'_, SharedStorageState>,
    meeting: Meeting,
) -> Result<(), String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos.meetings.update(&meeting).map_err(|e| e.to_string())?;

    info!("Updated meeting: {}", meeting.id);
    Ok(())
}

/// Update meeting status
#[tauri::command]
pub fn update_meeting_status(
    storage: tauri::State<'_, SharedStorageState>,
    id: String,
    status: String,
    error_message: Option<String>,
) -> Result<(), String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let status: MeetingStatus = status.parse().map_err(|e: anyhow::Error| e.to_string())?;

    repos
        .meetings
        .update_status(&id, status, error_message.as_deref())
        .map_err(|e| e.to_string())?;

    info!("Updated meeting {} status to {:?}", id, status);
    Ok(())
}

/// Delete a meeting (cascades to transcripts, notes, embeddings, etc.)
#[tauri::command]
pub async fn delete_meeting(
    storage: tauri::State<'_, SharedStorageState>,
    config: tauri::State<'_, crate::AppConfig>,
    id: String,
) -> Result<bool, String> {
    // Get vector store and delete from SQLite (release lock before await)
    let (deleted, vectors) = {
        let storage = storage.lock();
        let repos = storage.repositories();

        let deleted = repos.meetings.delete(&id).map_err(|e| e.to_string())?;
        (deleted, storage.vectors.clone())
    };

    // Also delete embeddings from LanceDB
    if deleted {
        vectors
            .delete_meeting_embeddings(&id)
            .await
            .map_err(|e| e.to_string())?;

        // Best-effort cleanup of on-disk audio files.
        let meeting_audio_dir = config.audio_dir.join(&id);
        if meeting_audio_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&meeting_audio_dir) {
                warn!(
                    "Meeting {} deleted but failed to remove audio dir {:?}: {}",
                    id, meeting_audio_dir, e
                );
            } else {
                info!("Deleted audio directory for meeting {}", id);
            }
        }

        info!("Deleted meeting and embeddings: {}", id);
    }

    Ok(deleted)
}

/// Count meetings
#[tauri::command]
pub fn count_meetings(
    storage: tauri::State<'_, SharedStorageState>,
    status: Option<String>,
) -> Result<u64, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let status_filter = if let Some(s) = status {
        Some(s.parse().map_err(|e: anyhow::Error| e.to_string())?)
    } else {
        None
    };

    repos
        .meetings
        .count(status_filter)
        .map_err(|e| e.to_string())
}

// ============================================
// TRANSCRIPT COMMANDS
// ============================================

/// Get transcript for a meeting
#[tauri::command]
pub fn get_transcript(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<Vec<TranscriptSegment>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let stored = repos
        .transcripts
        .get_by_meeting(&meeting_id)
        .map_err(|e| e.to_string())?;

    // Convert to inference segments
    let segments: Vec<TranscriptSegment> = stored.iter().map(|s| s.to_inference()).collect();

    Ok(segments)
}

/// Save transcript segments for a meeting
#[tauri::command]
pub fn save_transcript(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    segments: Vec<TranscriptSegment>,
) -> Result<u64, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    // Convert from inference segments to stored segments
    let stored: Vec<StoredSegment> = segments
        .iter()
        .map(|s| StoredSegment::from_inference(s, &meeting_id))
        .collect();

    let ids = repos
        .transcripts
        .insert_batch(&stored)
        .map_err(|e| e.to_string())?;

    info!(
        "Saved {} transcript segments for meeting {}",
        ids.len(),
        meeting_id
    );

    Ok(ids.len() as u64)
}

/// Get full transcript text
#[tauri::command]
pub fn get_transcript_text(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<String, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos
        .transcripts
        .get_full_text(&meeting_id)
        .map_err(|e| e.to_string())
}

/// Delete transcript for a meeting (also removes embeddings)
#[tauri::command]
pub async fn delete_transcript(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<u64, String> {
    // Delete from SQLite and get vector store (release lock before await)
    let (count, vectors) = {
        let storage = storage.lock();
        let repos = storage.repositories();

        let count = repos
            .transcripts
            .delete_by_meeting(&meeting_id)
            .map_err(|e| e.to_string())?;
        (count, storage.vectors.clone())
    };

    // Also delete embeddings from LanceDB
    if count > 0 {
        vectors
            .delete_meeting_embeddings(&meeting_id)
            .await
            .map_err(|e| e.to_string())?;
    }

    info!(
        "Deleted {} transcript segments and embeddings for meeting {}",
        count, meeting_id
    );
    Ok(count)
}

// ============================================
// SEARCH COMMANDS
// ============================================

/// Search transcripts using full-text search
#[tauri::command]
pub fn search_transcripts(
    storage: tauri::State<'_, SharedStorageState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SearchHit>, String> {
    let storage = storage.lock();

    storage
        .search
        .search_transcripts(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

/// Search transcripts with highlighted snippets
#[tauri::command]
pub fn search_transcripts_with_snippets(
    storage: tauri::State<'_, SharedStorageState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SearchHitWithSnippet>, String> {
    let storage = storage.lock();

    storage
        .search
        .search_with_snippets(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

/// Search within a specific meeting
#[tauri::command]
pub fn search_in_meeting(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SearchHit>, String> {
    let storage = storage.lock();

    storage
        .search
        .search_in_meeting(&meeting_id, &query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

// ============================================
// NOTES COMMANDS
// ============================================

/// Save or update a note for a meeting
#[tauri::command]
pub fn save_note(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    content: String,
) -> Result<Note, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let note = repos
        .notes
        .upsert(&meeting_id, &content)
        .map_err(|e| e.to_string())?;

    info!("Saved note for meeting {}", meeting_id);
    Ok(note)
}

/// Get notes for a meeting
#[tauri::command]
pub fn get_notes(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<Vec<Note>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos
        .notes
        .get_by_meeting(&meeting_id)
        .map_err(|e| e.to_string())
}

/// Get the primary note for a meeting (most recent)
#[tauri::command]
pub fn get_note(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<Option<Note>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos
        .notes
        .get_primary(&meeting_id)
        .map_err(|e| e.to_string())
}

// ============================================
// SUMMARY COMMANDS
// ============================================

/// Save or update a summary for a meeting
#[tauri::command]
pub fn save_summary(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    summary_type: String,
    content: String,
    model_used: Option<String>,
) -> Result<Summary, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let summary_type: SummaryType = summary_type
        .parse()
        .map_err(|e: anyhow::Error| e.to_string())?;

    let summary = repos
        .summaries
        .upsert(&meeting_id, summary_type, &content, model_used.as_deref())
        .map_err(|e| e.to_string())?;

    info!(
        "Saved {} summary for meeting {}",
        summary_type.as_str(),
        meeting_id
    );
    Ok(summary)
}

/// Get all summaries for a meeting
#[tauri::command]
pub fn get_summaries(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
) -> Result<Vec<Summary>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    repos
        .summaries
        .get_by_meeting(&meeting_id)
        .map_err(|e| e.to_string())
}

/// Get a specific summary by type
#[tauri::command]
pub fn get_summary(
    storage: tauri::State<'_, SharedStorageState>,
    meeting_id: String,
    summary_type: String,
) -> Result<Option<Summary>, String> {
    let storage = storage.lock();
    let repos = storage.repositories();

    let summary_type: SummaryType = summary_type
        .parse()
        .map_err(|e: anyhow::Error| e.to_string())?;

    repos
        .summaries
        .get_by_type(&meeting_id, summary_type)
        .map_err(|e| e.to_string())
}

// ============================================
// STATS COMMANDS
// ============================================

/// Get database statistics
#[tauri::command]
pub fn get_database_stats(
    storage: tauri::State<'_, SharedStorageState>,
) -> Result<DatabaseStats, String> {
    let storage = storage.lock();
    storage.stats().map_err(|e| e.to_string())
}

/// Get storage statistics (disk usage)
#[tauri::command]
pub fn get_storage_stats(
    storage: tauri::State<'_, SharedStorageState>,
    config: tauri::State<'_, crate::AppConfig>,
) -> Result<StorageStats, String> {
    // Calculate sizes synchronously to avoid holding lock across await
    let storage = storage.lock();
    let db_size = storage.db.file_size().map_err(|e| e.to_string())?;

    let data_dir = &config.data_dir;
    let vectors_size = dir_size(&data_dir.join("data").join("vectors"));
    let audio_size = dir_size(&data_dir.join("audio"));
    let models_size = dir_size(&config.models_dir);

    Ok(StorageStats {
        database_bytes: db_size,
        vectors_bytes: vectors_size,
        audio_bytes: audio_size,
        models_bytes: models_size,
        total_bytes: db_size + vectors_size + audio_size + models_size,
    })
}

/// Calculate directory size recursively
fn dir_size(path: &std::path::Path) -> u64 {
    let mut size = 0;

    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        size += metadata.len();
                    } else if metadata.is_dir() {
                        size += dir_size(&entry.path());
                    }
                }
            }
        }
    }

    size
}
