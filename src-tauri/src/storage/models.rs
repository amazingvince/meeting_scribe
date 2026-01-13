//! Storage data models
//!
//! Types optimized for database storage and retrieval.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::inference::{Speaker, TranscriptSegment as InferenceSegment};

/// Meeting status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MeetingStatus {
    /// Currently recording
    #[default]
    Recording,
    /// Processing audio/transcription
    Processing,
    /// Ready for viewing
    Ready,
    /// Archived (audio compressed)
    Archived,
    /// Error occurred
    Error,
}

impl MeetingStatus {
    /// Convert to database string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Archived => "archived",
            Self::Error => "error",
        }
    }
}

impl std::str::FromStr for MeetingStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recording" => Ok(Self::Recording),
            "processing" => Ok(Self::Processing),
            "ready" => Ok(Self::Ready),
            "archived" => Ok(Self::Archived),
            "error" => Ok(Self::Error),
            _ => anyhow::bail!("Invalid meeting status: {}", s),
        }
    }
}

impl std::fmt::Display for MeetingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Summary type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryType {
    /// Key points bullet list
    KeyPoints,
    /// Action items extracted
    ActionItems,
    /// Full summary
    Full,
}

impl SummaryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KeyPoints => "key_points",
            Self::ActionItems => "action_items",
            Self::Full => "full",
        }
    }
}

impl std::str::FromStr for SummaryType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "key_points" => Ok(Self::KeyPoints),
            "action_items" => Ok(Self::ActionItems),
            "full" => Ok(Self::Full),
            _ => anyhow::bail!("Invalid summary type: {}", s),
        }
    }
}

/// Meeting entity for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    /// Unique meeting ID (UUID)
    pub id: String,
    /// Meeting title
    pub title: String,
    /// Creation timestamp (Unix ms)
    pub created_at: i64,
    /// Last update timestamp (Unix ms)
    pub updated_at: i64,
    /// Total duration in milliseconds
    pub duration_ms: Option<i64>,
    /// Path to "you" audio file
    pub audio_path_you: Option<String>,
    /// Path to "others" audio file
    pub audio_path_others: Option<String>,
    /// Processing status
    pub status: MeetingStatus,
    /// Error message if status is Error
    pub error_message: Option<String>,
    /// Tags as JSON array
    pub tags: Vec<String>,
}

impl Meeting {
    /// Create a new meeting with generated ID
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now().timestamp_millis();

        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            created_at: now,
            updated_at: now,
            duration_ms: None,
            audio_path_you: None,
            audio_path_others: None,
            status: MeetingStatus::Recording,
            error_message: None,
            tags: Vec::new(),
        }
    }

    /// Generate default title from current timestamp
    pub fn default_title() -> String {
        let now = chrono::Local::now();
        now.format("Meeting %Y-%m-%d %H:%M").to_string()
    }

    /// Update the updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now().timestamp_millis();
    }

    /// Set status and optionally error message
    pub fn set_status(&mut self, status: MeetingStatus, error: Option<String>) {
        self.status = status;
        self.error_message = error;
        self.touch();
    }
}

impl Default for Meeting {
    fn default() -> Self {
        Self::new(Self::default_title())
    }
}

/// Transcript segment for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSegment {
    /// Database row ID (auto-generated)
    pub id: Option<i64>,
    /// Meeting this segment belongs to
    pub meeting_id: String,
    /// Start time in milliseconds
    pub start_ms: i64,
    /// End time in milliseconds
    pub end_ms: i64,
    /// Transcribed text
    pub text: String,
    /// Speaker label
    pub speaker: Speaker,
    /// Confidence score (0.0 - 1.0)
    pub confidence: Option<f64>,
    /// Reference to embedding in vector store
    pub embedding_id: Option<String>,
}

impl StoredSegment {
    /// Create a new segment for a meeting
    pub fn new(
        meeting_id: impl Into<String>,
        start_ms: i64,
        end_ms: i64,
        text: impl Into<String>,
        speaker: Speaker,
    ) -> Self {
        Self {
            id: None,
            meeting_id: meeting_id.into(),
            start_ms,
            end_ms,
            text: text.into(),
            speaker,
            confidence: None,
            embedding_id: None,
        }
    }

    /// Duration in milliseconds
    pub fn duration_ms(&self) -> i64 {
        self.end_ms - self.start_ms
    }
}

/// Convert from inference segment to stored segment
impl StoredSegment {
    /// Create from inference segment with meeting ID
    pub fn from_inference(segment: &InferenceSegment, meeting_id: &str) -> Self {
        Self {
            id: None,
            meeting_id: meeting_id.to_string(),
            start_ms: segment.start_ms as i64,
            end_ms: segment.end_ms as i64,
            text: segment.text.clone(),
            speaker: segment.speaker,
            confidence: segment.confidence.map(|c| c as f64),
            embedding_id: None,
        }
    }

    /// Convert back to inference segment
    pub fn to_inference(&self) -> InferenceSegment {
        InferenceSegment {
            start_ms: self.start_ms as u64,
            end_ms: self.end_ms as u64,
            text: self.text.clone(),
            speaker: self.speaker,
            confidence: self.confidence.map(|c| c as f32),
        }
    }
}

/// User note attached to a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Database row ID
    pub id: Option<i64>,
    /// Meeting this note belongs to
    pub meeting_id: String,
    /// Note content
    pub content: String,
    /// Creation timestamp (Unix ms)
    pub created_at: i64,
    /// Last update timestamp (Unix ms)
    pub updated_at: i64,
    /// Reference to embedding in vector store
    pub embedding_id: Option<String>,
}

impl Note {
    /// Create a new note for a meeting
    pub fn new(meeting_id: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Utc::now().timestamp_millis();

        Self {
            id: None,
            meeting_id: meeting_id.into(),
            content: content.into(),
            created_at: now,
            updated_at: now,
            embedding_id: None,
        }
    }

    /// Update the note content
    pub fn update_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
        self.updated_at = Utc::now().timestamp_millis();
    }
}

/// Generated summary for a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Database row ID
    pub id: Option<i64>,
    /// Meeting this summary belongs to
    pub meeting_id: String,
    /// Type of summary
    pub summary_type: SummaryType,
    /// Summary content
    pub content: String,
    /// Model used to generate summary
    pub model_used: Option<String>,
    /// Creation timestamp (Unix ms)
    pub created_at: i64,
    /// Reference to embedding in vector store
    pub embedding_id: Option<String>,
}

impl Summary {
    /// Create a new summary
    pub fn new(
        meeting_id: impl Into<String>,
        summary_type: SummaryType,
        content: impl Into<String>,
        model_used: Option<String>,
    ) -> Self {
        Self {
            id: None,
            meeting_id: meeting_id.into(),
            summary_type,
            content: content.into(),
            model_used,
            created_at: Utc::now().timestamp_millis(),
            embedding_id: None,
        }
    }
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    /// Total number of meetings
    pub meeting_count: u64,
    /// Total number of transcript segments
    pub segment_count: u64,
    /// Total duration of all meetings in milliseconds
    pub total_duration_ms: u64,
    /// Total number of notes
    pub note_count: u64,
}

/// Storage usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Database file size in bytes
    pub database_bytes: u64,
    /// Vector store size in bytes
    pub vectors_bytes: u64,
    /// Audio files size in bytes
    pub audio_bytes: u64,
    /// Total storage used
    pub total_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meeting_status_roundtrip() {
        for status in [
            MeetingStatus::Recording,
            MeetingStatus::Processing,
            MeetingStatus::Ready,
            MeetingStatus::Archived,
            MeetingStatus::Error,
        ] {
            let s = status.as_str();
            let parsed: MeetingStatus = s.parse().unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_meeting_creation() {
        let meeting = Meeting::new("Test Meeting");
        assert!(!meeting.id.is_empty());
        assert_eq!(meeting.title, "Test Meeting");
        assert_eq!(meeting.status, MeetingStatus::Recording);
        assert!(meeting.created_at > 0);
    }

    #[test]
    fn test_stored_segment_conversion() {
        let inference_segment = InferenceSegment {
            start_ms: 1000,
            end_ms: 5000,
            text: "Hello world".to_string(),
            speaker: Speaker::You,
            confidence: Some(0.95),
        };

        let stored = StoredSegment::from_inference(&inference_segment, "meeting-123");

        assert_eq!(stored.meeting_id, "meeting-123");
        assert_eq!(stored.start_ms, 1000);
        assert_eq!(stored.end_ms, 5000);
        assert_eq!(stored.text, "Hello world");
        assert_eq!(stored.speaker, Speaker::You);

        // Convert back
        let back = stored.to_inference();
        assert_eq!(back.start_ms, 1000);
        assert_eq!(back.end_ms, 5000);
        assert_eq!(back.text, "Hello world");
    }

    #[test]
    fn test_note_creation() {
        let note = Note::new("meeting-123", "Important point discussed");
        assert!(note.id.is_none());
        assert_eq!(note.meeting_id, "meeting-123");
        assert_eq!(note.content, "Important point discussed");
    }

    #[test]
    fn test_summary_types() {
        assert_eq!(SummaryType::KeyPoints.as_str(), "key_points");
        assert_eq!(SummaryType::ActionItems.as_str(), "action_items");
        assert_eq!(SummaryType::Full.as_str(), "full");
    }
}
