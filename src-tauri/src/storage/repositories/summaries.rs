//! Summaries repository
//!
//! CRUD operations for generated meeting summaries.

use anyhow::Result;
use rusqlite::{params, OptionalExtension, Row};
use tracing::debug;

use crate::storage::models::{Summary, SummaryType};
use crate::storage::sqlite::Database;

/// Repository for summary operations
pub struct SummariesRepository {
    db: Database,
}

impl SummariesRepository {
    /// Create a new summaries repository
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Save or update a summary for a meeting
    ///
    /// If a summary of the same type already exists for the meeting, it will be updated.
    /// Otherwise, a new summary is created.
    pub fn upsert(
        &self,
        meeting_id: &str,
        summary_type: SummaryType,
        content: &str,
        model_used: Option<&str>,
    ) -> Result<Summary> {
        let now = chrono::Utc::now().timestamp_millis();
        let type_str = summary_type.as_str();

        self.db.with_conn(|conn| {
            // Check if a summary of this type already exists
            let existing_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM summaries WHERE meeting_id = ? AND summary_type = ? LIMIT 1",
                    params![meeting_id, type_str],
                    |row| row.get(0),
                )
                .ok();

            if let Some(id) = existing_id {
                // Update existing summary
                conn.execute(
                    "UPDATE summaries SET content = ?, model_used = ?, created_at = ? WHERE id = ?",
                    params![content, model_used, now, id],
                )?;
                debug!(
                    "Updated {} summary {} for meeting {}",
                    type_str, id, meeting_id
                );

                Ok(Summary {
                    id: Some(id),
                    meeting_id: meeting_id.to_string(),
                    summary_type,
                    content: content.to_string(),
                    model_used: model_used.map(String::from),
                    created_at: now,
                    embedding_id: None,
                })
            } else {
                // Insert new summary
                conn.execute(
                    r#"
                    INSERT INTO summaries (meeting_id, summary_type, content, model_used, created_at)
                    VALUES (?, ?, ?, ?, ?)
                    "#,
                    params![meeting_id, type_str, content, model_used, now],
                )?;

                let id = conn.last_insert_rowid();
                debug!(
                    "Created {} summary {} for meeting {}",
                    type_str, id, meeting_id
                );

                Ok(Summary {
                    id: Some(id),
                    meeting_id: meeting_id.to_string(),
                    summary_type,
                    content: content.to_string(),
                    model_used: model_used.map(String::from),
                    created_at: now,
                    embedding_id: None,
                })
            }
        })
    }

    /// Get all summaries for a meeting
    pub fn get_by_meeting(&self, meeting_id: &str) -> Result<Vec<Summary>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, meeting_id, summary_type, content, model_used, created_at, embedding_id
                FROM summaries
                WHERE meeting_id = ?
                ORDER BY created_at DESC
                "#,
            )?;

            let summaries = stmt
                .query_map(params![meeting_id], |row| Ok(row_to_summary(row)))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(summaries)
        })
    }

    /// Get a specific summary by meeting and type
    pub fn get_by_type(
        &self,
        meeting_id: &str,
        summary_type: SummaryType,
    ) -> Result<Option<Summary>> {
        let type_str = summary_type.as_str();

        self.db.with_conn(|conn| {
            Ok(conn
                .query_row(
                    r#"
                SELECT id, meeting_id, summary_type, content, model_used, created_at, embedding_id
                FROM summaries
                WHERE meeting_id = ? AND summary_type = ?
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                    params![meeting_id, type_str],
                    |row| Ok(row_to_summary(row)),
                )
                .optional()?)
        })
    }

    /// Delete a summary by ID
    pub fn delete(&self, id: i64) -> Result<bool> {
        self.db.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM summaries WHERE id = ?", params![id])?;
            debug!("Deleted summary {}: {} rows affected", id, rows);
            Ok(rows > 0)
        })
    }

    /// Delete all summaries for a meeting
    pub fn delete_by_meeting(&self, meeting_id: &str) -> Result<usize> {
        self.db.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM summaries WHERE meeting_id = ?",
                params![meeting_id],
            )?;
            debug!("Deleted {} summaries for meeting {}", rows, meeting_id);
            Ok(rows)
        })
    }
}

/// Convert a database row to a Summary
fn row_to_summary(row: &Row) -> Summary {
    let type_str: String = row.get(2).unwrap_or_default();
    let summary_type = type_str.parse().unwrap_or(SummaryType::Full);

    Summary {
        id: row.get(0).ok(),
        meeting_id: row.get(1).unwrap_or_default(),
        summary_type,
        content: row.get(3).unwrap_or_default(),
        model_used: row.get(4).ok(),
        created_at: row.get(5).unwrap_or(0),
        embedding_id: row.get(6).ok(),
    }
}
