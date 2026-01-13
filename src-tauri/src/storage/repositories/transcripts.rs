//! Transcript repository
//!
//! CRUD operations for transcript segments.

use anyhow::Result;
use rusqlite::{params, Row};
use tracing::debug;

use crate::inference::Speaker;
use crate::storage::models::StoredSegment;
use crate::storage::sqlite::Database;

/// Repository for transcript segment operations
pub struct TranscriptRepository {
    db: Database,
}

impl TranscriptRepository {
    /// Create a new transcript repository
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Insert a single segment
    pub fn insert(&self, segment: &StoredSegment) -> Result<i64> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO transcript_segments (
                    meeting_id, start_ms, end_ms, text,
                    speaker, confidence, embedding_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    segment.meeting_id,
                    segment.start_ms,
                    segment.end_ms,
                    segment.text,
                    speaker_to_str(segment.speaker),
                    segment.confidence,
                    segment.embedding_id,
                ],
            )?;

            let id = conn.last_insert_rowid();
            debug!("Inserted segment {} for meeting {}", id, segment.meeting_id);
            Ok(id)
        })
    }

    /// Insert multiple segments in a batch (using a transaction)
    pub fn insert_batch(&self, segments: &[StoredSegment]) -> Result<Vec<i64>> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        self.db.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let mut ids = Vec::with_capacity(segments.len());

            {
                let mut stmt = tx.prepare(
                    r#"
                    INSERT INTO transcript_segments (
                        meeting_id, start_ms, end_ms, text,
                        speaker, confidence, embedding_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#,
                )?;

                for segment in segments {
                    stmt.execute(params![
                        segment.meeting_id,
                        segment.start_ms,
                        segment.end_ms,
                        segment.text,
                        speaker_to_str(segment.speaker),
                        segment.confidence,
                        segment.embedding_id,
                    ])?;

                    ids.push(tx.last_insert_rowid());
                }
            }

            tx.commit()?;

            debug!("Inserted {} segments in batch", segments.len());
            Ok(ids)
        })
    }

    /// Get all segments for a meeting, ordered by time
    pub fn get_by_meeting(&self, meeting_id: &str) -> Result<Vec<StoredSegment>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT * FROM transcript_segments
                WHERE meeting_id = ?
                ORDER BY start_ms ASC
                "#,
            )?;

            let segments = stmt
                .query_map([meeting_id], Self::row_to_segment)?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(segments)
        })
    }

    /// Get segments in a specific time range
    pub fn get_in_range(
        &self,
        meeting_id: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<StoredSegment>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT * FROM transcript_segments
                WHERE meeting_id = ?
                  AND start_ms >= ?
                  AND end_ms <= ?
                ORDER BY start_ms ASC
                "#,
            )?;

            let segments = stmt
                .query_map(params![meeting_id, start_ms, end_ms], Self::row_to_segment)?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(segments)
        })
    }

    /// Get segments by speaker
    pub fn get_by_speaker(&self, meeting_id: &str, speaker: Speaker) -> Result<Vec<StoredSegment>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT * FROM transcript_segments
                WHERE meeting_id = ?
                  AND speaker = ?
                ORDER BY start_ms ASC
                "#,
            )?;

            let segments = stmt
                .query_map(params![meeting_id, speaker_to_str(speaker)], Self::row_to_segment)?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(segments)
        })
    }

    /// Get full transcript text for a meeting
    pub fn get_full_text(&self, meeting_id: &str) -> Result<String> {
        let segments = self.get_by_meeting(meeting_id)?;

        let text = segments
            .iter()
            .map(|s| format!("[{}] {}", speaker_to_label(s.speaker), s.text))
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(text)
    }

    /// Update embedding ID for a segment
    pub fn update_embedding_id(&self, id: i64, embedding_id: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                "UPDATE transcript_segments SET embedding_id = ? WHERE id = ?",
                params![embedding_id, id],
            )?;
            Ok(())
        })
    }

    /// Update embedding IDs for multiple segments
    pub fn update_embedding_ids(&self, updates: &[(i64, String)]) -> Result<()> {
        self.db.with_conn_mut(|conn| {
            let tx = conn.transaction()?;

            {
                let mut stmt = tx.prepare(
                    "UPDATE transcript_segments SET embedding_id = ? WHERE id = ?"
                )?;

                for (id, embedding_id) in updates {
                    stmt.execute(params![embedding_id, id])?;
                }
            }

            tx.commit()?;
            debug!("Updated {} embedding IDs", updates.len());
            Ok(())
        })
    }

    /// Delete all segments for a meeting
    pub fn delete_by_meeting(&self, meeting_id: &str) -> Result<u64> {
        self.db.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM transcript_segments WHERE meeting_id = ?",
                [meeting_id],
            )?;

            debug!("Deleted {} segments for meeting {}", rows, meeting_id);
            Ok(rows as u64)
        })
    }

    /// Count segments for a meeting
    pub fn count(&self, meeting_id: &str) -> Result<u64> {
        self.db.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM transcript_segments WHERE meeting_id = ?",
                [meeting_id],
                |row| row.get(0),
            )?;

            Ok(count as u64)
        })
    }

    /// Get total duration of segments for a meeting
    pub fn total_duration(&self, meeting_id: &str) -> Result<i64> {
        self.db.with_conn(|conn| {
            let duration: i64 = conn.query_row(
                "SELECT COALESCE(SUM(end_ms - start_ms), 0) FROM transcript_segments WHERE meeting_id = ?",
                [meeting_id],
                |row| row.get(0),
            )?;

            Ok(duration)
        })
    }

    /// Get word count for a meeting
    pub fn word_count(&self, meeting_id: &str) -> Result<u64> {
        let segments = self.get_by_meeting(meeting_id)?;
        let count: usize = segments
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum();

        Ok(count as u64)
    }

    /// Convert database row to StoredSegment
    fn row_to_segment(row: &Row) -> rusqlite::Result<StoredSegment> {
        let speaker_str: Option<String> = row.get("speaker")?;
        let speaker = speaker_str
            .map(|s| str_to_speaker(&s))
            .unwrap_or(Speaker::Unknown);

        Ok(StoredSegment {
            id: Some(row.get("id")?),
            meeting_id: row.get("meeting_id")?,
            start_ms: row.get("start_ms")?,
            end_ms: row.get("end_ms")?,
            text: row.get("text")?,
            speaker,
            confidence: row.get("confidence")?,
            embedding_id: row.get("embedding_id")?,
        })
    }
}

/// Convert Speaker enum to database string
fn speaker_to_str(speaker: Speaker) -> &'static str {
    match speaker {
        Speaker::You => "you",
        Speaker::Others => "others",
        Speaker::Unknown => "unknown",
    }
}

/// Convert database string to Speaker enum
fn str_to_speaker(s: &str) -> Speaker {
    match s {
        "you" => Speaker::You,
        "others" => Speaker::Others,
        _ => Speaker::Unknown,
    }
}

/// Convert Speaker enum to display label
fn speaker_to_label(speaker: Speaker) -> &'static str {
    match speaker {
        Speaker::You => "YOU",
        Speaker::Others => "OTHERS",
        Speaker::Unknown => "SPEAKER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Meeting;
    use crate::storage::repositories::MeetingRepository;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();
        (temp, db)
    }

    fn create_test_meeting(db: &Database) -> String {
        let meeting_repo = MeetingRepository::new(db.clone());
        let meeting = Meeting::new("Test Meeting");
        let meeting_id = meeting.id.clone();
        meeting_repo.create(&meeting).unwrap();
        meeting_id
    }

    #[test]
    fn test_insert_and_get_segment() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segment = StoredSegment::new(&meeting_id, 0, 5000, "Hello world", Speaker::You);

        let id = repo.insert(&segment).unwrap();
        assert!(id > 0);

        let segments = repo.get_by_meeting(&meeting_id).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello world");
        assert_eq!(segments[0].speaker, Speaker::You);
    }

    #[test]
    fn test_insert_batch() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "First segment", Speaker::You),
            StoredSegment::new(&meeting_id, 2500, 5000, "Second segment", Speaker::Others),
            StoredSegment::new(&meeting_id, 5500, 8000, "Third segment", Speaker::You),
        ];

        let ids = repo.insert_batch(&segments).unwrap();
        assert_eq!(ids.len(), 3);

        let loaded = repo.get_by_meeting(&meeting_id).unwrap();
        assert_eq!(loaded.len(), 3);
    }

    #[test]
    fn test_get_by_speaker() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "You speaking", Speaker::You),
            StoredSegment::new(&meeting_id, 2500, 5000, "Others speaking", Speaker::Others),
            StoredSegment::new(&meeting_id, 5500, 8000, "You again", Speaker::You),
        ];

        repo.insert_batch(&segments).unwrap();

        let you_segments = repo.get_by_speaker(&meeting_id, Speaker::You).unwrap();
        assert_eq!(you_segments.len(), 2);

        let others_segments = repo.get_by_speaker(&meeting_id, Speaker::Others).unwrap();
        assert_eq!(others_segments.len(), 1);
    }

    #[test]
    fn test_get_in_range() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "Segment 1", Speaker::You),
            StoredSegment::new(&meeting_id, 3000, 5000, "Segment 2", Speaker::Others),
            StoredSegment::new(&meeting_id, 6000, 8000, "Segment 3", Speaker::You),
        ];

        repo.insert_batch(&segments).unwrap();

        let range_segments = repo.get_in_range(&meeting_id, 2000, 6000).unwrap();
        assert_eq!(range_segments.len(), 1);
        assert_eq!(range_segments[0].text, "Segment 2");
    }

    #[test]
    fn test_get_full_text() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "Hello", Speaker::You),
            StoredSegment::new(&meeting_id, 2500, 5000, "Hi there", Speaker::Others),
        ];

        repo.insert_batch(&segments).unwrap();

        let text = repo.get_full_text(&meeting_id).unwrap();
        assert!(text.contains("[YOU] Hello"));
        assert!(text.contains("[OTHERS] Hi there"));
    }

    #[test]
    fn test_update_embedding_id() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segment = StoredSegment::new(&meeting_id, 0, 5000, "Hello", Speaker::You);
        let id = repo.insert(&segment).unwrap();

        repo.update_embedding_id(id, "emb-123").unwrap();

        let loaded = repo.get_by_meeting(&meeting_id).unwrap();
        assert_eq!(loaded[0].embedding_id, Some("emb-123".to_string()));
    }

    #[test]
    fn test_delete_by_meeting() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "Segment 1", Speaker::You),
            StoredSegment::new(&meeting_id, 3000, 5000, "Segment 2", Speaker::Others),
        ];

        repo.insert_batch(&segments).unwrap();

        let deleted = repo.delete_by_meeting(&meeting_id).unwrap();
        assert_eq!(deleted, 2);

        let remaining = repo.get_by_meeting(&meeting_id).unwrap();
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_count_and_stats() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db);

        let segments = vec![
            StoredSegment::new(&meeting_id, 0, 2000, "Hello world", Speaker::You),
            StoredSegment::new(&meeting_id, 3000, 5000, "Hi there friend", Speaker::Others),
        ];

        repo.insert_batch(&segments).unwrap();

        let count = repo.count(&meeting_id).unwrap();
        assert_eq!(count, 2);

        let duration = repo.total_duration(&meeting_id).unwrap();
        assert_eq!(duration, 4000); // (2000-0) + (5000-3000)

        let words = repo.word_count(&meeting_id).unwrap();
        assert_eq!(words, 5); // "Hello world" + "Hi there friend"
    }

    #[test]
    fn test_fts_triggered_on_insert() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_meeting(&db);
        let repo = TranscriptRepository::new(db.clone());

        let segment = StoredSegment::new(
            &meeting_id,
            0,
            5000,
            "unique searchable text content",
            Speaker::You,
        );

        repo.insert(&segment).unwrap();

        // Verify FTS index has the content
        db.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM transcript_fts WHERE transcript_fts MATCH ?",
                ["unique"],
                |row| row.get(0),
            )?;

            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    }
}
