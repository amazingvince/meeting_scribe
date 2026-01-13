//! Repository module
//!
//! Data access layer using the repository pattern.

mod meetings;
mod notes;
mod summaries;
mod transcripts;

pub use meetings::{ListOptions, MeetingRepository};
pub use notes::NotesRepository;
pub use summaries::SummariesRepository;
pub use transcripts::TranscriptRepository;

use crate::storage::sqlite::Database;

/// Container for all repositories
///
/// Provides a convenient way to access all repositories from a single database connection.
pub struct Repositories {
    /// Meeting repository
    pub meetings: MeetingRepository,
    /// Transcript repository
    pub transcripts: TranscriptRepository,
    /// Notes repository
    pub notes: NotesRepository,
    /// Summaries repository
    pub summaries: SummariesRepository,
}

impl Repositories {
    /// Create a new repositories container
    pub fn new(db: Database) -> Self {
        Self {
            meetings: MeetingRepository::new(db.clone()),
            transcripts: TranscriptRepository::new(db.clone()),
            notes: NotesRepository::new(db.clone()),
            summaries: SummariesRepository::new(db),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{Meeting, StoredSegment};
    use crate::inference::Speaker;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();
        (temp, db)
    }

    #[test]
    fn test_repositories_integration() {
        let (_temp, db) = setup_test_db();
        let repos = Repositories::new(db);

        // Create a meeting
        let meeting = Meeting::new("Integration Test");
        let meeting_id = meeting.id.clone();
        repos.meetings.create(&meeting).unwrap();

        // Add transcript segments
        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "Hello world", Speaker::You),
            StoredSegment::new(&meeting_id, 2500, 5000, "Hi there", Speaker::Others),
        ];
        repos.transcripts.insert_batch(&segments).unwrap();

        // Verify meeting exists
        let loaded = repos.meetings.get(&meeting_id).unwrap();
        assert!(loaded.is_some());

        // Verify segments exist
        let loaded_segments = repos.transcripts.get_by_meeting(&meeting_id).unwrap();
        assert_eq!(loaded_segments.len(), 2);

        // Delete meeting (should cascade to segments)
        repos.meetings.delete(&meeting_id).unwrap();

        // Verify segments deleted
        let remaining_segments = repos.transcripts.get_by_meeting(&meeting_id).unwrap();
        assert!(remaining_segments.is_empty());
    }
}
