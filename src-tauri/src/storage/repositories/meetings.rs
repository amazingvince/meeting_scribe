//! Meeting repository
//!
//! CRUD operations for meetings.

use anyhow::Result;
use rusqlite::{params, Row};
use tracing::debug;

use crate::storage::models::{Meeting, MeetingStatus};
use crate::storage::sqlite::Database;

/// Repository for meeting operations
pub struct MeetingRepository {
    db: Database,
}

impl MeetingRepository {
    /// Create a new meeting repository
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create a new meeting
    pub fn create(&self, meeting: &Meeting) -> Result<()> {
        self.db.with_conn(|conn| {
            let tags_json = serde_json::to_string(&meeting.tags)?;

            conn.execute(
                r#"
                INSERT INTO meetings (
                    id, title, created_at, updated_at, duration_ms,
                    audio_path_you, audio_path_others,
                    status, error_message, tags
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    meeting.id,
                    meeting.title,
                    meeting.created_at,
                    meeting.updated_at,
                    meeting.duration_ms,
                    meeting.audio_path_you,
                    meeting.audio_path_others,
                    meeting.status.as_str(),
                    meeting.error_message,
                    tags_json,
                ],
            )?;

            debug!("Created meeting: {}", meeting.id);
            Ok(())
        })
    }

    /// Get a meeting by ID
    pub fn get(&self, id: &str) -> Result<Option<Meeting>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT * FROM meetings WHERE id = ?")?;

            let result = stmt.query_row([id], Self::row_to_meeting);

            match result {
                Ok(meeting) => Ok(Some(meeting)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    /// List meetings with pagination and filtering
    pub fn list(&self, options: ListOptions) -> Result<Vec<Meeting>> {
        self.db.with_conn(|conn| {
            let mut query = String::from("SELECT * FROM meetings WHERE 1=1");
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            // Apply status filter
            if let Some(ref status) = options.status {
                query.push_str(" AND status = ?");
                params_vec.push(Box::new(status.as_str().to_string()));
            }

            // Apply search filter (title search)
            if let Some(ref search) = options.search {
                query.push_str(" AND title LIKE ?");
                params_vec.push(Box::new(format!("%{}%", search)));
            }

            // Apply date range
            if let Some(after) = options.after {
                query.push_str(" AND created_at >= ?");
                params_vec.push(Box::new(after));
            }

            if let Some(before) = options.before {
                query.push_str(" AND created_at <= ?");
                params_vec.push(Box::new(before));
            }

            // Order and pagination
            query.push_str(" ORDER BY created_at DESC");
            query.push_str(&format!(" LIMIT {} OFFSET {}", options.limit, options.offset));

            let mut stmt = conn.prepare(&query)?;

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let meetings = stmt
                .query_map(params_refs.as_slice(), Self::row_to_meeting)?
                .collect::<Result<Vec<_>, _>>()?;

            debug!("Listed {} meetings", meetings.len());
            Ok(meetings)
        })
    }

    /// Update a meeting
    pub fn update(&self, meeting: &Meeting) -> Result<()> {
        self.db.with_conn(|conn| {
            let tags_json = serde_json::to_string(&meeting.tags)?;

            let rows = conn.execute(
                r#"
                UPDATE meetings SET
                    title = ?,
                    updated_at = ?,
                    duration_ms = ?,
                    audio_path_you = ?,
                    audio_path_others = ?,
                    status = ?,
                    error_message = ?,
                    tags = ?
                WHERE id = ?
                "#,
                params![
                    meeting.title,
                    meeting.updated_at,
                    meeting.duration_ms,
                    meeting.audio_path_you,
                    meeting.audio_path_others,
                    meeting.status.as_str(),
                    meeting.error_message,
                    tags_json,
                    meeting.id,
                ],
            )?;

            if rows == 0 {
                anyhow::bail!("Meeting not found: {}", meeting.id);
            }

            debug!("Updated meeting: {}", meeting.id);
            Ok(())
        })
    }

    /// Update meeting status
    pub fn update_status(
        &self,
        id: &str,
        status: MeetingStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        self.db.with_conn(|conn| {
            let now = chrono::Utc::now().timestamp_millis();

            let rows = conn.execute(
                r#"
                UPDATE meetings SET
                    status = ?,
                    error_message = ?,
                    updated_at = ?
                WHERE id = ?
                "#,
                params![status.as_str(), error_message, now, id],
            )?;

            if rows == 0 {
                anyhow::bail!("Meeting not found: {}", id);
            }

            debug!("Updated meeting {} status to {:?}", id, status);
            Ok(())
        })
    }

    /// Delete a meeting (cascades to segments, notes, summaries)
    pub fn delete(&self, id: &str) -> Result<bool> {
        self.db.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM meetings WHERE id = ?", [id])?;

            if rows > 0 {
                debug!("Deleted meeting: {}", id);
            }

            Ok(rows > 0)
        })
    }

    /// Count meetings matching filter
    pub fn count(&self, status: Option<MeetingStatus>) -> Result<u64> {
        self.db.with_conn(|conn| {
            let count: i64 = if let Some(status) = status {
                conn.query_row(
                    "SELECT COUNT(*) FROM meetings WHERE status = ?",
                    [status.as_str()],
                    |row| row.get(0),
                )?
            } else {
                conn.query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))?
            };

            Ok(count as u64)
        })
    }

    /// Get recent meetings
    pub fn recent(&self, limit: u32) -> Result<Vec<Meeting>> {
        self.list(ListOptions::new().with_limit(limit))
    }

    /// Convert a database row to a Meeting
    fn row_to_meeting(row: &Row) -> rusqlite::Result<Meeting> {
        let tags_json: String = row.get("tags")?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        let status_str: String = row.get("status")?;
        let status: MeetingStatus = status_str.parse().unwrap_or(MeetingStatus::Error);

        Ok(Meeting {
            id: row.get("id")?,
            title: row.get("title")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            duration_ms: row.get("duration_ms")?,
            audio_path_you: row.get("audio_path_you")?,
            audio_path_others: row.get("audio_path_others")?,
            status,
            error_message: row.get("error_message")?,
            tags,
        })
    }
}

/// Options for listing meetings
#[derive(Debug, Default, Clone)]
pub struct ListOptions {
    /// Filter by status
    pub status: Option<MeetingStatus>,
    /// Search in title
    pub search: Option<String>,
    /// Created after timestamp
    pub after: Option<i64>,
    /// Created before timestamp
    pub before: Option<i64>,
    /// Maximum results
    pub limit: u32,
    /// Offset for pagination
    pub offset: u32,
}

impl ListOptions {
    /// Create default options
    pub fn new() -> Self {
        Self {
            limit: 50,
            offset: 0,
            ..Default::default()
        }
    }

    /// Filter by status
    pub fn with_status(mut self, status: MeetingStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Search in title
    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    /// Set pagination
    pub fn with_pagination(mut self, limit: u32, offset: u32) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }

    /// Set limit only
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    /// Filter by date range
    pub fn with_date_range(mut self, after: Option<i64>, before: Option<i64>) -> Self {
        self.after = after;
        self.before = before;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        db.initialize().unwrap();
        (temp, db)
    }

    #[test]
    fn test_create_and_get_meeting() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        let meeting = Meeting::new("Test Meeting");
        let meeting_id = meeting.id.clone();

        repo.create(&meeting).unwrap();

        let loaded = repo.get(&meeting_id).unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, meeting_id);
        assert_eq!(loaded.title, "Test Meeting");
        assert_eq!(loaded.status, MeetingStatus::Recording);
    }

    #[test]
    fn test_update_meeting() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        let mut meeting = Meeting::new("Original Title");
        repo.create(&meeting).unwrap();

        meeting.title = "Updated Title".to_string();
        meeting.touch();
        repo.update(&meeting).unwrap();

        let loaded = repo.get(&meeting.id).unwrap().unwrap();
        assert_eq!(loaded.title, "Updated Title");
    }

    #[test]
    fn test_update_status() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        let meeting = Meeting::new("Test Meeting");
        repo.create(&meeting).unwrap();

        repo.update_status(&meeting.id, MeetingStatus::Ready, None)
            .unwrap();

        let loaded = repo.get(&meeting.id).unwrap().unwrap();
        assert_eq!(loaded.status, MeetingStatus::Ready);
    }

    #[test]
    fn test_delete_meeting() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        let meeting = Meeting::new("To Delete");
        repo.create(&meeting).unwrap();

        let deleted = repo.delete(&meeting.id).unwrap();
        assert!(deleted);

        let loaded = repo.get(&meeting.id).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_list_meetings() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        // Create multiple meetings
        for i in 0..5 {
            let meeting = Meeting::new(format!("Meeting {}", i));
            repo.create(&meeting).unwrap();
        }

        // List all
        let all = repo.list(ListOptions::new()).unwrap();
        assert_eq!(all.len(), 5);

        // List with limit
        let limited = repo.list(ListOptions::new().with_limit(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_list_by_status() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        let meeting1 = Meeting::new("Recording");
        repo.create(&meeting1).unwrap();

        let mut meeting2 = Meeting::new("Ready");
        meeting2.status = MeetingStatus::Ready;
        repo.create(&meeting2).unwrap();

        let ready = repo
            .list(ListOptions::new().with_status(MeetingStatus::Ready))
            .unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].title, "Ready");
    }

    #[test]
    fn test_search_meetings() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        repo.create(&Meeting::new("Team Standup")).unwrap();
        repo.create(&Meeting::new("Client Call")).unwrap();
        repo.create(&Meeting::new("Team Retrospective")).unwrap();

        let team_meetings = repo
            .list(ListOptions::new().with_search("Team"))
            .unwrap();
        assert_eq!(team_meetings.len(), 2);
    }

    #[test]
    fn test_count_meetings() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        repo.create(&Meeting::new("Meeting 1")).unwrap();
        repo.create(&Meeting::new("Meeting 2")).unwrap();

        let count = repo.count(None).unwrap();
        assert_eq!(count, 2);

        let recording_count = repo.count(Some(MeetingStatus::Recording)).unwrap();
        assert_eq!(recording_count, 2);
    }

    #[test]
    fn test_meeting_with_tags() {
        let (_temp, db) = setup_test_db();
        let repo = MeetingRepository::new(db);

        let mut meeting = Meeting::new("Tagged Meeting");
        meeting.tags = vec!["important".to_string(), "project-x".to_string()];
        repo.create(&meeting).unwrap();

        let loaded = repo.get(&meeting.id).unwrap().unwrap();
        assert_eq!(loaded.tags.len(), 2);
        assert!(loaded.tags.contains(&"important".to_string()));
    }
}
