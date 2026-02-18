//! Full-text search service
//!
//! SQLite FTS5-based transcript search with highlighting support.

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::sqlite::Database;

/// Full-text search service for transcripts
pub struct SearchService {
    db: Database,
}

impl SearchService {
    /// Create a new search service
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Search transcripts using FTS5
    pub fn search_transcripts(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>> {
        self.db.with_conn(|conn| {
            let Some(fts_query) = Self::sanitize_fts_query(query) else {
                debug!(
                    "FTS search for '{}' returned 0 results (empty query after sanitization)",
                    query
                );
                return Ok(Vec::new());
            };

            let mut stmt = conn.prepare(
                r#"
                SELECT
                    ts.id,
                    ts.meeting_id,
                    ts.start_ms,
                    ts.end_ms,
                    ts.text,
                    ts.speaker,
                    m.title as meeting_title,
                    m.created_at as meeting_date,
                    bm25(transcript_fts) as rank
                FROM transcript_fts
                JOIN transcript_segments ts ON transcript_fts.rowid = ts.id
                JOIN meetings m ON ts.meeting_id = m.id
                WHERE transcript_fts MATCH ?
                ORDER BY rank
                LIMIT ?
                "#,
            )?;

            let results = stmt
                .query_map(params![fts_query, limit], |row| {
                    Ok(SearchHit {
                        segment_id: row.get("id")?,
                        meeting_id: row.get("meeting_id")?,
                        meeting_title: row.get("meeting_title")?,
                        meeting_date: row.get("meeting_date")?,
                        start_ms: row.get("start_ms")?,
                        end_ms: row.get("end_ms")?,
                        text: row.get("text")?,
                        speaker: row.get("speaker")?,
                        rank: row.get("rank")?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            debug!(
                "FTS search for '{}' returned {} results",
                query,
                results.len()
            );
            Ok(results)
        })
    }

    /// Search with snippet highlighting
    pub fn search_with_snippets(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SearchHitWithSnippet>> {
        self.db.with_conn(|conn| {
            let Some(fts_query) = Self::sanitize_fts_query(query) else {
                debug!(
                    "FTS snippet search for '{}' returned 0 results (empty query after sanitization)",
                    query
                );
                return Ok(Vec::new());
            };

            let mut stmt = conn.prepare(
                r#"
                SELECT
                    ts.id,
                    ts.meeting_id,
                    ts.start_ms,
                    ts.end_ms,
                    ts.speaker,
                    m.title as meeting_title,
                    m.created_at as meeting_date,
                    snippet(transcript_fts, 0, '<mark>', '</mark>', '...', 32) as snippet,
                    bm25(transcript_fts) as rank
                FROM transcript_fts
                JOIN transcript_segments ts ON transcript_fts.rowid = ts.id
                JOIN meetings m ON ts.meeting_id = m.id
                WHERE transcript_fts MATCH ?
                ORDER BY rank
                LIMIT ?
                "#,
            )?;

            let results = stmt
                .query_map(params![fts_query, limit], |row| {
                    Ok(SearchHitWithSnippet {
                        segment_id: row.get("id")?,
                        meeting_id: row.get("meeting_id")?,
                        meeting_title: row.get("meeting_title")?,
                        meeting_date: row.get("meeting_date")?,
                        start_ms: row.get("start_ms")?,
                        end_ms: row.get("end_ms")?,
                        speaker: row.get("speaker")?,
                        snippet: row.get("snippet")?,
                        rank: row.get("rank")?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            debug!(
                "FTS search with snippets for '{}' returned {} results",
                query,
                results.len()
            );
            Ok(results)
        })
    }

    /// Search within a specific meeting
    pub fn search_in_meeting(
        &self,
        meeting_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SearchHit>> {
        self.db.with_conn(|conn| {
            let Some(fts_query) = Self::sanitize_fts_query(query) else {
                debug!(
                    "FTS in-meeting search for '{}' returned 0 results (empty query after sanitization)",
                    query
                );
                return Ok(Vec::new());
            };

            let mut stmt = conn.prepare(
                r#"
                SELECT
                    ts.id,
                    ts.meeting_id,
                    ts.start_ms,
                    ts.end_ms,
                    ts.text,
                    ts.speaker,
                    m.title as meeting_title,
                    m.created_at as meeting_date,
                    bm25(transcript_fts) as rank
                FROM transcript_fts
                JOIN transcript_segments ts ON transcript_fts.rowid = ts.id
                JOIN meetings m ON ts.meeting_id = m.id
                WHERE transcript_fts MATCH ?
                  AND ts.meeting_id = ?
                ORDER BY rank
                LIMIT ?
                "#,
            )?;

            let results = stmt
                .query_map(params![fts_query, meeting_id, limit], |row| {
                    Ok(SearchHit {
                        segment_id: row.get("id")?,
                        meeting_id: row.get("meeting_id")?,
                        meeting_title: row.get("meeting_title")?,
                        meeting_date: row.get("meeting_date")?,
                        start_ms: row.get("start_ms")?,
                        end_ms: row.get("end_ms")?,
                        text: row.get("text")?,
                        speaker: row.get("speaker")?,
                        rank: row.get("rank")?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(results)
        })
    }

    /// Get recent segments matching a query (for autocomplete)
    pub fn autocomplete(&self, query: &str, limit: u32) -> Result<Vec<String>> {
        let normalized = Self::normalize_query(query);
        if normalized.len() < 2 {
            return Ok(Vec::new());
        }

        self.db.with_conn(|conn| {
            // Use prefix search for autocomplete
            let fts_query = format!("{}*", normalized);

            let mut stmt = conn.prepare(
                r#"
                SELECT DISTINCT ts.text
                FROM transcript_fts
                JOIN transcript_segments ts ON transcript_fts.rowid = ts.id
                WHERE transcript_fts MATCH ?
                LIMIT ?
                "#,
            )?;

            let results = stmt
                .query_map(params![fts_query, limit], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(results)
        })
    }

    /// Rebuild the FTS index (useful after bulk imports)
    pub fn rebuild_index(&self) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute_batch("INSERT INTO transcript_fts(transcript_fts) VALUES('rebuild')")?;
            debug!("Rebuilt FTS index");
            Ok(())
        })
    }

    /// Optimize the FTS index
    pub fn optimize_index(&self) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute_batch("INSERT INTO transcript_fts(transcript_fts) VALUES('optimize')")?;
            debug!("Optimized FTS index");
            Ok(())
        })
    }

    /// Sanitize query for FTS5 (escape special characters)
    fn sanitize_fts_query(query: &str) -> Option<String> {
        let normalized = Self::normalize_query(query);
        if normalized.is_empty() {
            return None;
        }

        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.is_empty() {
            return None;
        }

        // Use AND + prefix matching for multi-word queries to improve recall while
        // keeping operator semantics explicit and sanitized.
        let prefixed_tokens = tokens.iter().map(|token| format!("{}*", token));
        Some(prefixed_tokens.collect::<Vec<_>>().join(" AND "))
    }

    /// Normalize query text after escaping:
    /// - trim leading/trailing space
    /// - collapse repeated internal whitespace
    /// - drop FTS special operators/symbols
    fn normalize_query(query: &str) -> String {
        let escaped = Self::escape_special_chars(query.trim());
        escaped.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Escape FTS5 special characters
    fn escape_special_chars(s: &str) -> String {
        s.replace('"', "\"\"")
            .replace(['*', '?', '[', ']', '(', ')', '+'], "")
            .replace('-', " ") // Replace hyphen with space
    }
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Segment database ID
    pub segment_id: i64,
    /// Meeting ID
    pub meeting_id: String,
    /// Meeting title
    pub meeting_title: String,
    /// Meeting date (timestamp ms)
    pub meeting_date: i64,
    /// Start time in meeting (ms)
    pub start_ms: i64,
    /// End time in meeting (ms)
    pub end_ms: i64,
    /// Full text of the segment
    pub text: String,
    /// Speaker label
    pub speaker: String,
    /// BM25 ranking score (lower is better)
    pub rank: f64,
}

/// Search result with highlighted snippet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHitWithSnippet {
    /// Segment database ID
    pub segment_id: i64,
    /// Meeting ID
    pub meeting_id: String,
    /// Meeting title
    pub meeting_title: String,
    /// Meeting date (timestamp ms)
    pub meeting_date: i64,
    /// Start time in meeting (ms)
    pub start_ms: i64,
    /// End time in meeting (ms)
    pub end_ms: i64,
    /// Speaker label
    pub speaker: String,
    /// Highlighted snippet with <mark> tags
    pub snippet: String,
    /// BM25 ranking score (lower is better)
    pub rank: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::Speaker;
    use crate::storage::models::{Meeting, StoredSegment};
    use crate::storage::repositories::{MeetingRepository, TranscriptRepository};
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();
        (temp, db)
    }

    fn create_test_data(db: &Database) -> String {
        let meeting_repo = MeetingRepository::new(db.clone());
        let transcript_repo = TranscriptRepository::new(db.clone());

        let meeting = Meeting::new("Test Meeting About AI");
        let meeting_id = meeting.id.clone();
        meeting_repo.create(&meeting).unwrap();

        let segments = vec![
            StoredSegment::new(
                &meeting_id,
                0,
                5000,
                "Let's discuss artificial intelligence and machine learning",
                Speaker::You,
            ),
            StoredSegment::new(
                &meeting_id,
                5500,
                10000,
                "Neural networks are fascinating technology",
                Speaker::Others,
            ),
            StoredSegment::new(
                &meeting_id,
                10500,
                15000,
                "We should implement a deep learning solution",
                Speaker::You,
            ),
        ];

        transcript_repo.insert_batch(&segments).unwrap();
        meeting_id
    }

    #[test]
    fn test_basic_search() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_data(&db);
        let search = SearchService::new(db);

        // Search for "artificial"
        let results = search.search_transcripts("artificial", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].meeting_id, meeting_id);
    }

    #[test]
    fn test_phrase_search() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        // Search for phrase
        let results = search.search_transcripts("machine learning", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_prefix_search() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        // Prefix search (should match "neural")
        let results = search.search_transcripts("neur", 10).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_with_snippets() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        let results = search.search_with_snippets("learning", 10).unwrap();
        assert!(!results.is_empty());

        // Snippet should contain <mark> tags
        assert!(results[0].snippet.contains("<mark>"));
    }

    #[test]
    fn test_search_in_meeting() {
        let (_temp, db) = setup_test_db();
        let meeting_id = create_test_data(&db);

        // Create another meeting
        let meeting_repo = MeetingRepository::new(db.clone());
        let transcript_repo = TranscriptRepository::new(db.clone());

        let meeting2 = Meeting::new("Other Meeting");
        let meeting2_id = meeting2.id.clone();
        meeting_repo.create(&meeting2).unwrap();

        transcript_repo
            .insert(&StoredSegment::new(
                &meeting2_id,
                0,
                5000,
                "This also mentions learning",
                Speaker::You,
            ))
            .unwrap();

        let search = SearchService::new(db);

        // Search in first meeting only
        let results = search
            .search_in_meeting(&meeting_id, "learning", 10)
            .unwrap();
        assert_eq!(results.len(), 2); // Both "machine learning" and "deep learning"

        // Search in second meeting
        let results2 = search
            .search_in_meeting(&meeting2_id, "learning", 10)
            .unwrap();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_no_results() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        let results = search
            .search_transcripts("nonexistentword12345", 10)
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_autocomplete() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        // Should find words starting with "art"
        let results = search.autocomplete("art", 5).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_sanitize_query() {
        // Single word becomes prefix search
        assert_eq!(
            SearchService::sanitize_fts_query("test"),
            Some("test*".to_string())
        );

        // Multiple words become AND-prefix search
        assert_eq!(
            SearchService::sanitize_fts_query("hello world"),
            Some("hello* AND world*".to_string())
        );

        // Special characters are escaped
        let sanitized = SearchService::sanitize_fts_query("test*?[]()");
        assert_eq!(sanitized, Some("test*".to_string()));
    }

    #[test]
    fn test_sanitize_empty_or_symbol_only_query() {
        assert_eq!(SearchService::sanitize_fts_query(""), None);
        assert_eq!(SearchService::sanitize_fts_query("   "), None);
        assert_eq!(SearchService::sanitize_fts_query("***"), None);
        assert_eq!(SearchService::sanitize_fts_query("()[]+?"), None);
    }

    #[test]
    fn test_search_with_symbol_only_query_returns_empty() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        // Should return empty result set instead of surfacing an FTS syntax error.
        let results = search.search_transcripts("***", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_rebuild_index() {
        let (_temp, db) = setup_test_db();
        create_test_data(&db);
        let search = SearchService::new(db);

        // Should not error
        search.rebuild_index().unwrap();
        search.optimize_index().unwrap();
    }
}
