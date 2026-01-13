//! Embedding pipeline for batch processing
//!
//! Combines chunking, embedding, and vector storage.

use crate::inference::chunking::{chunk_text, chunk_transcript, TranscriptSegmentInput, MAX_CHUNK_CHARS};
use crate::inference::embedding::{EmbeddingService, EmbeddingTask};
use crate::storage::vectors::{EmbeddingRecord, VectorStore};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;
use tracing::{debug, info};

/// Batch size for embedding generation
const EMBEDDING_BATCH_SIZE: usize = 8;

/// Pipeline for embedding generation and storage
pub struct EmbeddingPipeline {
    embedding_service: Arc<EmbeddingService>,
    vector_store: Arc<VectorStore>,
}

impl EmbeddingPipeline {
    /// Create a new embedding pipeline
    pub fn new(embedding_service: Arc<EmbeddingService>, vector_store: Arc<VectorStore>) -> Self {
        Self {
            embedding_service,
            vector_store,
        }
    }

    /// Process transcript segments and store embeddings
    ///
    /// Chunks the transcript, generates embeddings, and stores them in the vector database.
    pub async fn process_transcript<F>(
        &self,
        meeting_id: &str,
        segments: Vec<TranscriptSegmentInput>,
        mut progress_callback: F,
    ) -> Result<ProcessingResult>
    where
        F: FnMut(EmbeddingProgress),
    {
        info!("Processing transcript for meeting {}", meeting_id);

        // Chunk the transcript
        let chunks = chunk_transcript(&segments, MAX_CHUNK_CHARS);
        let total_chunks = chunks.len();

        debug!(
            "Created {} chunks from {} segments",
            total_chunks,
            segments.len()
        );

        progress_callback(EmbeddingProgress {
            stage: EmbeddingStage::Chunking,
            current: 0,
            total: total_chunks,
            message: format!("Created {} chunks", total_chunks),
        });

        if chunks.is_empty() {
            return Ok(ProcessingResult {
                meeting_id: meeting_id.to_string(),
                chunks_processed: 0,
                embeddings_stored: 0,
            });
        }

        // Generate embeddings in batches
        let mut records = Vec::new();

        for (batch_idx, batch) in chunks.chunks(EMBEDDING_BATCH_SIZE).enumerate() {
            let texts: Vec<&str> = batch.iter().map(|c| c.text.as_str()).collect();

            let embeddings = self
                .embedding_service
                .embed_batch(&texts, EmbeddingTask::Document)?;

            for (chunk, embedding) in batch.iter().zip(embeddings) {
                records.push(EmbeddingRecord::new_transcript(
                    meeting_id,
                    &chunk.text,
                    chunk.start_ms.unwrap_or(0),
                    embedding,
                ));
            }

            let processed = ((batch_idx + 1) * EMBEDDING_BATCH_SIZE).min(total_chunks);
            progress_callback(EmbeddingProgress {
                stage: EmbeddingStage::Embedding,
                current: processed,
                total: total_chunks,
                message: format!("Embedded {}/{} chunks", processed, total_chunks),
            });
        }

        // Store in vector database
        progress_callback(EmbeddingProgress {
            stage: EmbeddingStage::Storing,
            current: 0,
            total: records.len(),
            message: "Storing embeddings...".to_string(),
        });

        let embeddings_count = records.len();
        self.vector_store.add_embeddings(records).await?;

        progress_callback(EmbeddingProgress {
            stage: EmbeddingStage::Complete,
            current: total_chunks,
            total: total_chunks,
            message: format!("Stored {} embeddings", embeddings_count),
        });

        info!(
            "Stored {} embeddings for meeting {}",
            embeddings_count, meeting_id
        );

        Ok(ProcessingResult {
            meeting_id: meeting_id.to_string(),
            chunks_processed: total_chunks,
            embeddings_stored: embeddings_count,
        })
    }

    /// Process a note and store embedding(s)
    ///
    /// Returns the first embedding ID for reference.
    pub async fn process_note(
        &self,
        meeting_id: &str,
        _note_id: i64,
        content: &str,
    ) -> Result<String> {
        let chunks = chunk_text(content, MAX_CHUNK_CHARS);

        if chunks.is_empty() {
            return Ok(String::new());
        }

        let mut records = Vec::new();

        for chunk in &chunks {
            let embedding = self
                .embedding_service
                .embed(&chunk.text, EmbeddingTask::Document)?;

            records.push(EmbeddingRecord::new_note(meeting_id, &chunk.text, embedding));
        }

        let first_id = records.first().map(|r| r.id.clone()).unwrap_or_default();

        self.vector_store.add_embeddings(records).await?;

        Ok(first_id)
    }

    /// Process a summary and store embedding(s)
    ///
    /// Returns the first embedding ID for reference.
    pub async fn process_summary(
        &self,
        meeting_id: &str,
        _summary_id: i64,
        content: &str,
    ) -> Result<String> {
        let chunks = chunk_text(content, MAX_CHUNK_CHARS);

        if chunks.is_empty() {
            return Ok(String::new());
        }

        let mut records = Vec::new();

        for chunk in &chunks {
            let embedding = self
                .embedding_service
                .embed(&chunk.text, EmbeddingTask::Document)?;

            records.push(EmbeddingRecord::new_summary(
                meeting_id,
                &chunk.text,
                embedding,
            ));
        }

        let first_id = records.first().map(|r| r.id.clone()).unwrap_or_default();

        self.vector_store.add_embeddings(records).await?;

        Ok(first_id)
    }

    /// Embed a query for search
    ///
    /// Uses the Search task type for optimal search performance.
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embedding_service.embed(query, EmbeddingTask::Search)
    }

    /// Embed a question for QA
    ///
    /// Uses the QuestionAnswering task type for QA scenarios.
    pub fn embed_question(&self, question: &str) -> Result<Vec<f32>> {
        self.embedding_service
            .embed(question, EmbeddingTask::QuestionAnswering)
    }

    /// Delete all embeddings for a meeting
    pub async fn delete_meeting_embeddings(&self, meeting_id: &str) -> Result<u64> {
        self.vector_store.delete_meeting_embeddings(meeting_id).await
    }

    /// Get the embedding service for direct access
    pub fn embedding_service(&self) -> &EmbeddingService {
        &self.embedding_service
    }

    /// Get the vector store for direct access
    pub fn vector_store(&self) -> &VectorStore {
        &self.vector_store
    }
}

/// Processing stage for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingStage {
    /// Splitting text into chunks
    Chunking,
    /// Generating embeddings
    Embedding,
    /// Storing in vector database
    Storing,
    /// Processing complete
    Complete,
}

impl std::fmt::Display for EmbeddingStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chunking => write!(f, "chunking"),
            Self::Embedding => write!(f, "embedding"),
            Self::Storing => write!(f, "storing"),
            Self::Complete => write!(f, "complete"),
        }
    }
}

/// Progress information for embedding generation
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingProgress {
    /// Current processing stage
    pub stage: EmbeddingStage,
    /// Number of items processed
    pub current: usize,
    /// Total items to process
    pub total: usize,
    /// Human-readable message
    pub message: String,
}

impl EmbeddingProgress {
    /// Calculate percentage complete
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f32 / self.total as f32) * 100.0
        }
    }

    /// Check if processing is complete
    pub fn is_complete(&self) -> bool {
        self.stage == EmbeddingStage::Complete
    }
}

/// Result of processing operation
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingResult {
    /// Meeting ID that was processed
    pub meeting_id: String,
    /// Number of chunks created
    pub chunks_processed: usize,
    /// Number of embeddings stored
    pub embeddings_stored: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_progress_percentage() {
        let progress = EmbeddingProgress {
            stage: EmbeddingStage::Embedding,
            current: 50,
            total: 100,
            message: "test".to_string(),
        };

        assert!((progress.percentage() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_embedding_progress_zero_total() {
        let progress = EmbeddingProgress {
            stage: EmbeddingStage::Chunking,
            current: 0,
            total: 0,
            message: "test".to_string(),
        };

        assert_eq!(progress.percentage(), 0.0);
    }

    #[test]
    fn test_embedding_stage_display() {
        assert_eq!(EmbeddingStage::Chunking.to_string(), "chunking");
        assert_eq!(EmbeddingStage::Embedding.to_string(), "embedding");
        assert_eq!(EmbeddingStage::Storing.to_string(), "storing");
        assert_eq!(EmbeddingStage::Complete.to_string(), "complete");
    }

    #[test]
    fn test_is_complete() {
        let in_progress = EmbeddingProgress {
            stage: EmbeddingStage::Embedding,
            current: 50,
            total: 100,
            message: "test".to_string(),
        };
        assert!(!in_progress.is_complete());

        let complete = EmbeddingProgress {
            stage: EmbeddingStage::Complete,
            current: 100,
            total: 100,
            message: "test".to_string(),
        };
        assert!(complete.is_complete());
    }
}
