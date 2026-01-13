//! ML inference module
//!
//! Contains transcription, embedding, and LLM engines.

pub mod pipeline;
pub mod speaker;
pub mod transcription;

// Re-export key types
pub use pipeline::{
    create_progress_channel, MeetingProcessor, ProcessingProgress, ProcessingResult,
    ProcessingStage, TranscriptStatsDto,
};
pub use speaker::{format_transcript, format_transcript_compact, merge_transcripts, TranscriptStats};
pub use transcription::{
    format_duration, format_timestamp, Speaker, TranscriptionConfig, TranscriptionService,
    TranscriptSegment,
};
