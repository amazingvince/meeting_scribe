//! ML inference module
//!
//! Contains transcription, embedding, and LLM engines.

pub mod chunking;
pub mod embedding;
pub mod embedding_pipeline;
pub mod llm;
pub mod pipeline;
pub mod prompts;
pub mod speaker;
pub mod summarization;
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

// Embedding exports
pub use chunking::{chunk_text, chunk_transcript, TextChunk, TranscriptSegmentInput};
pub use embedding::{cosine_similarity, EmbeddingService, EmbeddingTask, EMBEDDING_DIM, MAX_TOKENS};
pub use embedding_pipeline::{EmbeddingPipeline, EmbeddingProgress, EmbeddingStage};

// LLM exports
pub use llm::{prepare_transcript_for_llm, GenerationConfig, LlmService};
pub use summarization::{ActionItem, Priority, SummarizationService, SummaryType};
