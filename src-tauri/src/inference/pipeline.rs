//! Meeting processing pipeline
//!
//! Combines audio preprocessing and transcription into a single workflow.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::audio::pipeline::{AudioPipeline, PipelineConfig};
use crate::models::TranscriptionBackend;

use super::speaker::{format_transcript, merge_transcripts, TranscriptStats};
use super::transcription::{Speaker, TranscriptSegment, TranscriptionService};

/// Result of processing a complete meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    /// Unique meeting identifier
    pub meeting_id: String,
    /// Merged transcript segments
    pub transcript: Vec<TranscriptSegment>,
    /// Formatted transcript text
    pub formatted_text: String,
    /// Total meeting duration in milliseconds
    pub duration_ms: u64,
    /// Time spent processing in milliseconds
    pub processing_time_ms: u64,
    /// Ratio of speech to silence (0.0 - 1.0)
    pub speech_ratio: f32,
    /// Transcription backend used
    pub backend: TranscriptionBackend,
    /// Statistics about the transcript
    pub stats: TranscriptStatsDto,
}

/// DTO for transcript statistics (serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptStatsDto {
    pub duration_ms: u64,
    pub segment_count: usize,
    pub you_segments: usize,
    pub others_segments: usize,
    pub word_count: usize,
    pub you_words: usize,
    pub others_words: usize,
    pub you_talk_ratio: f32,
}

impl From<TranscriptStats> for TranscriptStatsDto {
    fn from(stats: TranscriptStats) -> Self {
        Self {
            duration_ms: stats.duration_ms,
            segment_count: stats.segment_count,
            you_segments: stats.you_segments,
            others_segments: stats.others_segments,
            word_count: stats.word_count,
            you_words: stats.you_words,
            others_words: stats.others_words,
            you_talk_ratio: stats.you_talk_ratio(),
        }
    }
}

/// Progress update during processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingProgress {
    /// Current processing stage
    pub stage: ProcessingStage,
    /// Progress within current stage (0-100)
    pub percent: f32,
    /// Human-readable status message
    pub message: String,
}

/// Stages of meeting processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessingStage {
    /// Loading audio files
    Loading,
    /// Preprocessing audio (VAD + denoising)
    Preprocessing,
    /// Transcribing microphone audio
    TranscribingMic,
    /// Transcribing system audio
    TranscribingSystem,
    /// Merging transcripts
    Merging,
    /// Processing complete
    Complete,
    /// Processing failed
    Error(String),
}

/// Meeting processor combining audio pipeline and transcription
pub struct MeetingProcessor {
    _audio_pipeline: AudioPipeline,
    transcription: Arc<TranscriptionService>,
}

impl MeetingProcessor {
    /// Create a new meeting processor
    pub fn new(transcription: Arc<TranscriptionService>) -> Result<Self> {
        let audio_pipeline = AudioPipeline::new(PipelineConfig::default())?;

        Ok(Self {
            _audio_pipeline: audio_pipeline,
            transcription,
        })
    }

    /// Check if the processor is ready (transcription engine loaded)
    pub fn is_ready(&self) -> bool {
        self.transcription.is_ready()
    }

    /// Process a meeting from audio files
    ///
    /// Takes paths to preprocessed microphone and system audio files,
    /// transcribes them, and merges the results.
    pub async fn process_meeting(
        &self,
        meeting_id: &str,
        mic_path: Option<&Path>,
        system_path: Option<&Path>,
        progress_tx: Option<mpsc::Sender<ProcessingProgress>>,
    ) -> Result<ProcessingResult> {
        let start_time = Instant::now();
        let backend = self.transcription.backend();

        // Helper to send progress updates
        let send_progress = |stage: ProcessingStage, percent: f32, message: &str| {
            if let Some(ref tx) = progress_tx {
                let progress = ProcessingProgress {
                    stage,
                    percent,
                    message: message.to_string(),
                };
                let _ = tx.try_send(progress);
            }
        };

        send_progress(ProcessingStage::Loading, 0.0, "Loading audio files...");

        // Validate inputs
        if mic_path.is_none() && system_path.is_none() {
            anyhow::bail!("At least one audio file (mic or system) is required");
        }

        let mut mic_segments = Vec::new();
        let mut system_segments = Vec::new();
        let mut total_duration_ms: u64 = 0;

        // Process microphone audio
        if let Some(path) = mic_path {
            if path.exists() {
                send_progress(
                    ProcessingStage::TranscribingMic,
                    20.0,
                    "Transcribing microphone audio...",
                );

                debug!("Transcribing mic audio from {:?}", path);

                mic_segments = self
                    .transcription
                    .transcribe_file_with_speaker(path, Speaker::You)
                    .context("Failed to transcribe microphone audio")?;

                info!("Mic transcription: {} segments", mic_segments.len());

                // Calculate duration from segments
                if let Some(last) = mic_segments.last() {
                    total_duration_ms = total_duration_ms.max(last.end_ms);
                }
            } else {
                warn!("Mic audio file not found: {:?}", path);
            }
        }

        // Process system audio
        if let Some(path) = system_path {
            if path.exists() {
                send_progress(
                    ProcessingStage::TranscribingSystem,
                    50.0,
                    "Transcribing system audio...",
                );

                debug!("Transcribing system audio from {:?}", path);

                system_segments = self
                    .transcription
                    .transcribe_file_with_speaker(path, Speaker::Others)
                    .context("Failed to transcribe system audio")?;

                info!("System transcription: {} segments", system_segments.len());

                // Update duration
                if let Some(last) = system_segments.last() {
                    total_duration_ms = total_duration_ms.max(last.end_ms);
                }
            } else {
                warn!("System audio file not found: {:?}", path);
            }
        }

        // Merge transcripts
        send_progress(ProcessingStage::Merging, 80.0, "Merging transcripts...");

        let transcript = merge_transcripts(mic_segments, system_segments);
        let formatted_text = format_transcript(&transcript);
        let stats = TranscriptStats::from_segments(&transcript);

        // Calculate speech ratio
        let speech_time: u64 = transcript.iter().map(|s| s.duration_ms()).sum();
        let speech_ratio = if total_duration_ms > 0 {
            speech_time as f32 / total_duration_ms as f32
        } else {
            0.0
        };

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        send_progress(ProcessingStage::Complete, 100.0, "Processing complete");

        info!(
            "Meeting {} processed in {}ms: {} segments, {:.1}% speech",
            meeting_id,
            processing_time_ms,
            transcript.len(),
            speech_ratio * 100.0
        );

        Ok(ProcessingResult {
            meeting_id: meeting_id.to_string(),
            transcript,
            formatted_text,
            duration_ms: total_duration_ms,
            processing_time_ms,
            speech_ratio,
            backend,
            stats: stats.into(),
        })
    }

    /// Process a meeting with preprocessing (from raw audio)
    ///
    /// This method first preprocesses the audio (VAD + denoising) before transcription.
    pub async fn process_meeting_with_preprocessing(
        &self,
        meeting_id: &str,
        raw_mic_path: Option<&Path>,
        raw_system_path: Option<&Path>,
        progress_tx: Option<mpsc::Sender<ProcessingProgress>>,
    ) -> Result<ProcessingResult> {
        let send_progress = |stage: ProcessingStage, percent: f32, message: &str| {
            if let Some(ref tx) = progress_tx {
                let progress = ProcessingProgress {
                    stage,
                    percent,
                    message: message.to_string(),
                };
                let _ = tx.try_send(progress);
            }
        };

        send_progress(
            ProcessingStage::Preprocessing,
            10.0,
            "Preprocessing audio...",
        );

        // Preprocess audio files
        let (preprocessed_mic, preprocessed_system) =
            self.preprocess_audio(raw_mic_path, raw_system_path).await?;

        // Continue with regular processing
        self.process_meeting(
            meeting_id,
            preprocessed_mic.as_deref(),
            preprocessed_system.as_deref(),
            progress_tx,
        )
        .await
    }

    /// Preprocess raw audio files
    async fn preprocess_audio(
        &self,
        mic_path: Option<&Path>,
        system_path: Option<&Path>,
    ) -> Result<(Option<std::path::PathBuf>, Option<std::path::PathBuf>)> {
        let mut preprocessed_mic = None;
        let mut preprocessed_system = None;

        // TODO: Implement preprocessing using AudioPipeline
        // For now, just pass through the paths
        // The preprocessing should:
        // 1. Apply VAD to detect speech segments
        // 2. Apply denoising
        // 3. Save processed audio to temp files

        if let Some(path) = mic_path {
            preprocessed_mic = Some(path.to_path_buf());
        }

        if let Some(path) = system_path {
            preprocessed_system = Some(path.to_path_buf());
        }

        Ok((preprocessed_mic, preprocessed_system))
    }
}

/// Create a progress channel for processing updates
pub fn create_progress_channel() -> (
    mpsc::Sender<ProcessingProgress>,
    mpsc::Receiver<ProcessingProgress>,
) {
    mpsc::channel(32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_progress() {
        let progress = ProcessingProgress {
            stage: ProcessingStage::TranscribingMic,
            percent: 50.0,
            message: "Test".to_string(),
        };

        assert_eq!(progress.stage, ProcessingStage::TranscribingMic);
        assert_eq!(progress.percent, 50.0);
    }

    #[test]
    fn test_transcript_stats_dto() {
        let stats = TranscriptStats {
            duration_ms: 60000,
            segment_count: 10,
            you_segments: 5,
            others_segments: 5,
            word_count: 100,
            you_words: 60,
            others_words: 40,
        };

        let dto: TranscriptStatsDto = stats.into();
        assert_eq!(dto.duration_ms, 60000);
        assert_eq!(dto.word_count, 100);
        // Use approximate comparison for floating point
        assert!((dto.you_talk_ratio - 60.0).abs() < 0.01);
    }
}
