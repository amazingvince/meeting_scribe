//! Notes repository
//!
//! CRUD operations for user notes attached to meetings.

use anyhow::Result;
use rusqlite::{params, OptionalExtension, Row};
use tracing::debug;

use crate::storage::models::Note;
use crate::storage::sqlite::Database;

/// Repository for note operations
pub struct NotesRepository {
    db: Database,
}

impl NotesRepository {
    /// Create a new notes repository
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create or update a note for a meeting
    ///
    /// If a note already exists for the meeting, it will be updated.
    /// Otherwise, a new note is created.
    pub fn upsert(&self, meeting_id: &str, content: &str) -> Result<Note> {
        let now = chrono::Utc::now().timestamp_millis();

        self.db.with_conn(|conn| {
            // Check if a note already exists
            let existing_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM notes WHERE meeting_id = ? LIMIT 1",
                    params![meeting_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(id) = existing_id {
                // Update existing note
                conn.execute(
                    "UPDATE notes SET content = ?, updated_at = ? WHERE id = ?",
                    params![content, now, id],
                )?;
                debug!("Updated note {} for meeting {}", id, meeting_id);

                Ok(Note {
                    id: Some(id),
                    meeting_id: meeting_id.to_string(),
                    content: content.to_string(),
                    created_at: now, // Note: this is a simplification, should fetch actual created_at
                    updated_at: now,
                    embedding_id: None,
                })
            } else {
                // Insert new note
                conn.execute(
                    r#"
                    INSERT INTO notes (meeting_id, content, created_at, updated_at)
                    VALUES (?, ?, ?, ?)
                    "#,
                    params![meeting_id, content, now, now],
                )?;

                let id = conn.last_insert_rowid();
                debug!("Created note {} for meeting {}", id, meeting_id);

                Ok(Note {
                    id: Some(id),
                    meeting_id: meeting_id.to_string(),
                    content: content.to_string(),
                    created_at: now,
                    updated_at: now,
                    embedding_id: None,
                })
            }
        })
    }

    /// Get all notes for a meeting
    pub fn get_by_meeting(&self, meeting_id: &str) -> Result<Vec<Note>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, meeting_id, content, created_at, updated_at, embedding_id
                FROM notes
                WHERE meeting_id = ?
                ORDER BY created_at ASC
                "#,
            )?;

            let notes = stmt
                .query_map(params![meeting_id], |row| Ok(row_to_note(row)))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(notes)
        })
    }

    /// Get the primary note for a meeting (most recent)
    pub fn get_primary(&self, meeting_id: &str) -> Result<Option<Note>> {
        self.db.with_conn(|conn| {
            Ok(conn
                .query_row(
                    r#"
                SELECT id, meeting_id, content, created_at, updated_at, embedding_id
                FROM notes
                WHERE meeting_id = ?
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                    params![meeting_id],
                    |row| Ok(row_to_note(row)),
                )
                .optional()?)
        })
    }

    /// Delete a note by ID
    pub fn delete(&self, id: i64) -> Result<bool> {
        self.db.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM notes WHERE id = ?", params![id])?;
            debug!("Deleted note {}: {} rows affected", id, rows);
            Ok(rows > 0)
        })
    }

    /// Delete all notes for a meeting
    pub fn delete_by_meeting(&self, meeting_id: &str) -> Result<usize> {
        self.db.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM notes WHERE meeting_id = ?",
                params![meeting_id],
            )?;
            debug!(
                "Deleted {} notes for meeting {}",
                rows, meeting_id
            );
            Ok(rows)
        })
    }
}

/// Convert a database row to a Note
fn row_to_note(row: &Row) -> Note {
    Note {
        id: row.get(0).ok(),
        meeting_id: row.get(1).unwrap_or_default(),
        content: row.get(2).unwrap_or_default(),
        created_at: row.get(3).unwrap_or(0),
        updated_at: row.get(4).unwrap_or(0),
        embedding_id: row.get(5).ok(),
    }
}
