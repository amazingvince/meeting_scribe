//! Data persistence module
//!
//! SQLite for structured data, LanceDB for vector embeddings.
//!
//! ## Architecture
//!
//! - `sqlite`: SQLite database connection manager
//! - `models`: Data models for storage (Meeting, StoredSegment, Note, etc.)
//! - `repositories`: Data access layer (MeetingRepository, TranscriptRepository)
//! - `vectors`: LanceDB vector store for semantic search
//! - `search`: Full-text search using FTS5

pub mod models;
pub mod repositories;
pub mod search;
pub mod sqlite;
pub mod vectors;

// Re-export key types
pub use models::{
    DatabaseStats, Meeting, MeetingStatus, Note, StorageStats, StoredSegment, Summary, SummaryType,
};
pub use repositories::{ListOptions, MeetingRepository, NotesRepository, Repositories, SummariesRepository, TranscriptRepository};
pub use search::{SearchHit, SearchHitWithSnippet, SearchService};
pub use sqlite::Database;
pub use vectors::{EmbeddingRecord, SearchResult as VectorSearchResult, VectorStore, EMBEDDING_DIM};

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Initialize all storage components
///
/// Creates and initializes SQLite database, LanceDB vector store, and search service.
pub async fn initialize_storage(data_dir: impl AsRef<Path>) -> Result<StorageState> {
    let data_dir = data_dir.as_ref();

    // Create data subdirectory
    let data_path = data_dir.join("data");
    std::fs::create_dir_all(&data_path).context("Failed to create data directory")?;

    // SQLite database
    let db_path = data_path.join("meetings.db");
    let db = Database::open(&db_path).context("Failed to open SQLite database")?;
    db.initialize().context("Failed to initialize database schema")?;

    // Vector store
    let vectors_path = data_path.join("vectors");
    let vectors = VectorStore::open(&vectors_path)
        .await
        .context("Failed to open vector store")?;
    vectors
        .initialize()
        .await
        .context("Failed to initialize vector store")?;

    // Search service
    let search = SearchService::new(db.clone());

    info!("Storage initialized at {:?}", data_dir);

    Ok(StorageState {
        db,
        vectors: Arc::new(vectors),
        search,
    })
}

/// Combined storage state for the application
///
/// Contains all storage components needed by the application.
pub struct StorageState {
    /// SQLite database connection
    pub db: Database,
    /// LanceDB vector store (wrapped in Arc for async sharing)
    pub vectors: Arc<VectorStore>,
    /// Full-text search service
    pub search: SearchService,
}

impl StorageState {
    /// Get repositories for data access
    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.db.clone())
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DatabaseStats> {
        self.db.stats()
    }

    /// Get storage statistics (disk usage)
    pub async fn storage_stats(&self, data_dir: &Path, models_dir: &Path) -> Result<StorageStats> {
        let db_size = self.db.file_size()?;
        let vectors_size = dir_size(&data_dir.join("data").join("vectors"))?;
        let audio_size = dir_size(&data_dir.join("audio"))?;
        let models_size = dir_size(models_dir)?;

        Ok(StorageStats {
            database_bytes: db_size,
            vectors_bytes: vectors_size,
            audio_bytes: audio_size,
            models_bytes: models_size,
            total_bytes: db_size + vectors_size + audio_size + models_size,
        })
    }
}

/// Calculate directory size recursively
fn dir_size(path: &Path) -> Result<u64> {
    let mut size = 0;

    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap_or_else(|_| {
            // Return empty iterator if directory doesn't exist
            std::fs::read_dir(".").unwrap()
        }) {
            if let Ok(entry) = entry {
                let metadata = entry.metadata()?;

                if metadata.is_file() {
                    size += metadata.len();
                } else if metadata.is_dir() {
                    size += dir_size(&entry.path())?;
                }
            }
        }
    }

    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::Speaker;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_initialize_storage() {
        let temp: TempDir = TempDir::new().unwrap();
        let storage: StorageState = initialize_storage(temp.path()).await.unwrap();

        // Verify database is initialized
        let stats = storage.stats().unwrap();
        assert_eq!(stats.meeting_count, 0);

        // Verify vector store is initialized
        let count: u64 = storage.vectors.count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_full_workflow() {
        let temp: TempDir = TempDir::new().unwrap();
        let storage: StorageState = initialize_storage(temp.path()).await.unwrap();
        let repos = storage.repositories();

        // Create a meeting
        let meeting = Meeting::new("Integration Test Meeting");
        let meeting_id = meeting.id.clone();
        repos.meetings.create(&meeting).unwrap();

        // Add transcript segments
        let segments = vec![
            StoredSegment::new(
                &meeting_id,
                0,
                5000,
                "Hello everyone, welcome to the meeting",
                Speaker::You,
            ),
            StoredSegment::new(
                &meeting_id,
                5500,
                10000,
                "Thanks for joining today",
                Speaker::Others,
            ),
        ];
        repos.transcripts.insert_batch(&segments).unwrap();

        // Verify data was stored
        let loaded = repos.meetings.get(&meeting_id).unwrap().unwrap();
        assert_eq!(loaded.title, "Integration Test Meeting");

        let loaded_segments = repos.transcripts.get_by_meeting(&meeting_id).unwrap();
        assert_eq!(loaded_segments.len(), 2);

        // Test search
        let results = storage.search.search_transcripts("welcome", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].text.contains("welcome"));

        // Clean up
        repos.meetings.delete(&meeting_id).unwrap();

        // Verify cascade delete worked
        let remaining = repos.transcripts.get_by_meeting(&meeting_id).unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_storage_stats() {
        let temp: TempDir = TempDir::new().unwrap();
        let storage: StorageState = initialize_storage(temp.path()).await.unwrap();

        let stats: StorageStats = storage.storage_stats(temp.path()).await.unwrap();

        // Database should have some size (at least the schema)
        assert!(stats.database_bytes > 0);
        assert!(stats.total_bytes > 0);
    }
}
