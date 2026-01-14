//! Transcription service using transcribe-rs
//!
//! Provides speech-to-text transcription using Parakeet (default), Whisper, or Moonshine.

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use transcribe_rs::engines::parakeet::{
    ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity,
};
use transcribe_rs::{TranscriptionEngine, TranscriptionResult as TrResult};

use crate::audio::WHISPER_SAMPLE_RATE;
use crate::models::{ModelManager, TranscriptionBackend};

/// A transcribed segment with timing and speaker information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// The transcribed text
    pub text: String,
    /// Speaker label
    pub speaker: Speaker,
    /// Confidence score (0.0 - 1.0) if available
    pub confidence: Option<f32>,
}

impl TranscriptSegment {
    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Format as a timestamped line
    pub fn format_line(&self) -> String {
        format!(
            "[{} {}] {}",
            format_timestamp(self.start_ms),
            self.speaker,
            self.text
        )
    }
}

/// Speaker identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Speaker {
    /// The local user (from microphone)
    You,
    /// Other participants (from system audio)
    Others,
    /// Unknown speaker
    #[default]
    Unknown,
}

impl std::fmt::Display for Speaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Speaker::You => write!(f, "You"),
            Speaker::Others => write!(f, "Others"),
            Speaker::Unknown => write!(f, "Speaker"),
        }
    }
}

/// Configuration for transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Which transcription backend to use
    pub backend: TranscriptionBackend,
    /// Language code (e.g., "en" for English)
    pub language: String,
    /// Whether to include word-level timestamps
    pub word_timestamps: bool,
    /// Whether to attempt GPU acceleration
    pub use_gpu: bool,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            backend: TranscriptionBackend::Parakeet,
            language: "en".to_string(),
            word_timestamps: false,
            use_gpu: true,
        }
    }
}

/// Transcription service state
enum EngineState {
    /// No engine loaded
    Unloaded,
    /// Parakeet engine loaded
    Parakeet(ParakeetEngine),
    // Future: Add Whisper and Moonshine variants
}

impl Default for EngineState {
    fn default() -> Self {
        EngineState::Unloaded
    }
}

/// Transcription service wrapping transcribe-rs engines
pub struct TranscriptionService {
    engine: Arc<Mutex<EngineState>>,
    config: Arc<Mutex<TranscriptionConfig>>,
    model_path: Arc<Mutex<Option<PathBuf>>>,
}

impl Default for TranscriptionService {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptionService {
    /// Create a new transcription service
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(EngineState::Unloaded)),
            config: Arc::new(Mutex::new(TranscriptionConfig::default())),
            model_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if the engine is ready for transcription
    pub fn is_ready(&self) -> bool {
        !matches!(*self.engine.lock(), EngineState::Unloaded)
    }

    /// Get the current configuration
    pub fn config(&self) -> TranscriptionConfig {
        self.config.lock().clone()
    }

    /// Get the current backend
    pub fn backend(&self) -> TranscriptionBackend {
        self.config.lock().backend
    }

    /// Initialize the transcription engine with a model
    pub fn initialize(&self, model_manager: &ModelManager, config: TranscriptionConfig) -> Result<()> {
        let backend = config.backend;

        // Check if model is downloaded
        if !model_manager.is_model_ready(backend) {
            anyhow::bail!(
                "Model {} is not ready. Please download it first.",
                backend.model_info().name
            );
        }

        let model_path = model_manager.get_model_path(backend);
        info!("Initializing {} engine from {:?}", backend, model_path);
        info!("ORT_DYLIB_PATH env: {:?}", std::env::var("ORT_DYLIB_PATH"));

        match backend {
            TranscriptionBackend::Parakeet => {
                let mut engine = ParakeetEngine::new();

                // Use int8 quantization for better performance
                let params = ParakeetModelParams::int8();
                engine
                    .load_model_with_params(&model_path, params)
                    .map_err(|e| {
                        error!("Failed to load Parakeet model: {:?}", e);
                        error!("Model path: {:?}", model_path);
                        error!("Check if ONNX Runtime DLL version matches ort crate requirements");
                        anyhow::anyhow!("Failed to load Parakeet model: {:?}", e)
                    })?;

                *self.engine.lock() = EngineState::Parakeet(engine);
            }
            TranscriptionBackend::Whisper => {
                // TODO: Implement when whisper feature is added
                anyhow::bail!("Whisper backend not yet implemented");
            }
            TranscriptionBackend::Moonshine => {
                // TODO: Implement when moonshine feature is added
                anyhow::bail!("Moonshine backend not yet implemented");
            }
        }

        *self.config.lock() = config;
        *self.model_path.lock() = Some(model_path);

        info!("{} engine initialized successfully", backend);
        Ok(())
    }

    /// Unload the current engine
    pub fn unload(&self) {
        let mut engine = self.engine.lock();
        if let EngineState::Parakeet(ref mut e) = *engine {
            e.unload_model();
        }
        *engine = EngineState::Unloaded;
        *self.model_path.lock() = None;
        info!("Transcription engine unloaded");
    }

    /// Transcribe audio samples (f32, 16kHz, mono)
    ///
    /// Returns segments with timing information.
    /// The speaker field will be set to Unknown - use speaker labeling to assign speakers.
    pub fn transcribe_samples(&self, samples: Vec<f32>) -> Result<Vec<TranscriptSegment>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let config = self.config.lock().clone();
        let mut engine = self.engine.lock();

        match &mut *engine {
            EngineState::Unloaded => {
                anyhow::bail!("Transcription engine not initialized");
            }
            EngineState::Parakeet(engine) => {
                let granularity = if config.word_timestamps {
                    TimestampGranularity::Word
                } else {
                    TimestampGranularity::Segment
                };

                let params = ParakeetInferenceParams {
                    timestamp_granularity: granularity,
                };

                let result = engine
                    .transcribe_samples(samples, Some(params))
                    .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

                Ok(convert_result(result))
            }
        }
    }

    /// Maximum chunk duration in seconds for Parakeet (avoid attention overflow)
    const MAX_CHUNK_SECONDS: f32 = 30.0;
    /// Overlap between chunks in seconds
    const CHUNK_OVERLAP_SECONDS: f32 = 2.0;

    /// Transcribe a WAV file
    ///
    /// The WAV file should be 16kHz, mono, 16-bit PCM.
    /// Long files are automatically chunked to avoid memory issues.
    pub fn transcribe_file(&self, wav_path: &Path) -> Result<Vec<TranscriptSegment>> {
        if !wav_path.exists() {
            anyhow::bail!("Audio file not found: {:?}", wav_path);
        }

        // Load the audio file to check duration
        let samples = crate::audio::capture::load_wav(wav_path)?;
        let duration_secs = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;

        info!(
            "Audio file {:?}: {:.1}s ({} samples)",
            wav_path,
            duration_secs,
            samples.len()
        );

        // If audio is short enough, transcribe directly
        if duration_secs <= Self::MAX_CHUNK_SECONDS {
            return self.transcribe_samples_internal(samples, 0.0);
        }

        // For longer audio, chunk it
        info!(
            "Audio too long ({:.1}s), chunking into {:.0}s segments",
            duration_secs,
            Self::MAX_CHUNK_SECONDS
        );

        let chunk_samples = (Self::MAX_CHUNK_SECONDS * WHISPER_SAMPLE_RATE as f32) as usize;
        let overlap_samples = (Self::CHUNK_OVERLAP_SECONDS * WHISPER_SAMPLE_RATE as f32) as usize;
        let step_samples = chunk_samples - overlap_samples;

        let mut all_segments: Vec<TranscriptSegment> = Vec::new();
        let mut chunk_start = 0usize;
        let mut chunk_index = 0;

        while chunk_start < samples.len() {
            let chunk_end = (chunk_start + chunk_samples).min(samples.len());
            let chunk: Vec<f32> = samples[chunk_start..chunk_end].to_vec();

            let time_offset_secs = chunk_start as f32 / WHISPER_SAMPLE_RATE as f32;

            info!(
                "Transcribing chunk {} ({:.1}s - {:.1}s)",
                chunk_index,
                time_offset_secs,
                time_offset_secs + chunk.len() as f32 / WHISPER_SAMPLE_RATE as f32
            );

            match self.transcribe_samples_internal(chunk, time_offset_secs) {
                Ok(mut segments) => {
                    // Filter out segments that fall in the overlap region (except for first chunk)
                    if chunk_index > 0 {
                        let overlap_end_ms = (time_offset_secs + Self::CHUNK_OVERLAP_SECONDS) * 1000.0;
                        segments.retain(|s| s.start_ms as f32 >= overlap_end_ms - 500.0);
                    }
                    all_segments.extend(segments);
                }
                Err(e) => {
                    warn!("Chunk {} transcription failed: {}", chunk_index, e);
                    // Continue with next chunk instead of failing completely
                }
            }

            chunk_start += step_samples;
            chunk_index += 1;
        }

        info!(
            "Chunked transcription complete: {} total segments from {} chunks",
            all_segments.len(),
            chunk_index
        );

        // Sort by start time and deduplicate overlapping segments
        all_segments.sort_by_key(|s| s.start_ms);
        Ok(all_segments)
    }

    /// Internal method to transcribe samples with a time offset
    fn transcribe_samples_internal(
        &self,
        samples: Vec<f32>,
        time_offset_secs: f32,
    ) -> Result<Vec<TranscriptSegment>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let config = self.config.lock().clone();
        let mut engine = self.engine.lock();

        match &mut *engine {
            EngineState::Unloaded => {
                anyhow::bail!("Transcription engine not initialized");
            }
            EngineState::Parakeet(engine) => {
                let granularity = if config.word_timestamps {
                    TimestampGranularity::Word
                } else {
                    TimestampGranularity::Segment
                };

                let params = ParakeetInferenceParams {
                    timestamp_granularity: granularity,
                };

                debug!("Transcribing {} samples", samples.len());

                let result = engine
                    .transcribe_samples(samples, Some(params))
                    .map_err(|e| {
                        error!("Transcription error details: {:?}", e);
                        error!("ORT_DYLIB_PATH env: {:?}", std::env::var("ORT_DYLIB_PATH"));
                        anyhow::anyhow!("Transcription failed: {:?}", e)
                    })?;

                info!(
                    "Raw transcription result - text len: {}, segments: {:?}",
                    result.text.len(),
                    result.segments.as_ref().map(|s| s.len())
                );

                // Convert and adjust timestamps
                let mut segments = convert_result(result);

                // Add time offset to all segments
                let offset_ms = (time_offset_secs * 1000.0) as u64;
                for segment in &mut segments {
                    segment.start_ms += offset_ms;
                    segment.end_ms += offset_ms;
                }

                Ok(segments)
            }
        }
    }

    /// Transcribe with a specific speaker label
    pub fn transcribe_file_with_speaker(
        &self,
        wav_path: &Path,
        speaker: Speaker,
    ) -> Result<Vec<TranscriptSegment>> {
        let mut segments = self.transcribe_file(wav_path)?;

        for segment in &mut segments {
            segment.speaker = speaker;
        }

        Ok(segments)
    }

    /// Transcribe raw samples with a specific speaker label
    ///
    /// This is useful when you've pre-processed the audio (e.g., with AEC)
    /// and want to transcribe the processed samples directly.
    pub fn transcribe_samples_with_speaker(
        &self,
        samples: Vec<f32>,
        speaker: Speaker,
    ) -> Result<Vec<TranscriptSegment>> {
        let duration_secs = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;

        info!(
            "Transcribing {} samples ({:.1}s) for speaker {:?}",
            samples.len(),
            duration_secs,
            speaker
        );

        // If audio is short enough, transcribe directly
        let mut segments = if duration_secs <= Self::MAX_CHUNK_SECONDS {
            self.transcribe_samples_internal(samples, 0.0)?
        } else {
            // For longer audio, chunk it (same logic as transcribe_file)
            let chunk_samples = (Self::MAX_CHUNK_SECONDS * WHISPER_SAMPLE_RATE as f32) as usize;
            let overlap_samples =
                (Self::CHUNK_OVERLAP_SECONDS * WHISPER_SAMPLE_RATE as f32) as usize;
            let step_samples = chunk_samples - overlap_samples;

            let mut all_segments: Vec<TranscriptSegment> = Vec::new();
            let mut chunk_start = 0usize;
            let mut chunk_index = 0;

            while chunk_start < samples.len() {
                let chunk_end = (chunk_start + chunk_samples).min(samples.len());
                let chunk: Vec<f32> = samples[chunk_start..chunk_end].to_vec();

                let time_offset_secs = chunk_start as f32 / WHISPER_SAMPLE_RATE as f32;

                info!(
                    "Transcribing chunk {} ({:.1}s-{:.1}s)",
                    chunk_index,
                    time_offset_secs,
                    time_offset_secs + chunk.len() as f32 / WHISPER_SAMPLE_RATE as f32
                );

                match self.transcribe_samples_internal(chunk, time_offset_secs) {
                    Ok(mut chunk_segments) => {
                        all_segments.append(&mut chunk_segments);
                    }
                    Err(e) => {
                        warn!("Failed to transcribe chunk {}: {}", chunk_index, e);
                    }
                }

                chunk_start += step_samples;
                chunk_index += 1;
            }

            all_segments
        };

        // Assign speaker to all segments
        for segment in &mut segments {
            segment.speaker = speaker;
        }

        Ok(segments)
    }
}

/// Convert transcribe-rs result to our segment format
fn convert_result(result: TrResult) -> Vec<TranscriptSegment> {
    match result.segments {
        Some(segments) => segments
            .into_iter()
            .filter(|s| !s.text.trim().is_empty())
            .map(|s| TranscriptSegment {
                start_ms: (s.start * 1000.0) as u64,
                end_ms: (s.end * 1000.0) as u64,
                text: s.text.trim().to_string(),
                speaker: Speaker::Unknown,
                confidence: None,
            })
            .collect(),
        None => {
            // No segments from engine - split text into sentences for better display
            if result.text.trim().is_empty() {
                Vec::new()
            } else {
                split_text_into_segments(&result.text)
            }
        }
    }
}

/// Split text into sentence-based segments when no timestamp info available
/// Uses estimated timestamps based on word count (~150 WPM speaking rate)
fn split_text_into_segments(text: &str) -> Vec<TranscriptSegment> {
    // Split by sentence-ending punctuation
    let mut segments = Vec::new();
    let mut current_pos = 0u64;

    // Use regex-like splitting for sentences
    let sentence_enders = ['.', '?', '!'];
    let mut current_sentence = String::new();
    let mut last_end = 0;

    for (i, c) in text.char_indices() {
        current_sentence.push(c);

        if sentence_enders.contains(&c) {
            let trimmed = current_sentence.trim();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                // Estimate duration based on word count (~150 WPM = 400ms per word)
                let word_count = trimmed.split_whitespace().count();
                let duration_ms = (word_count as u64 * 400).max(500); // Min 500ms per segment

                segments.push(TranscriptSegment {
                    start_ms: current_pos,
                    end_ms: current_pos + duration_ms,
                    text: trimmed.to_string(),
                    speaker: Speaker::Unknown,
                    confidence: None,
                });

                current_pos += duration_ms;
            }
            current_sentence.clear();
            last_end = i + c.len_utf8();
        }
    }

    // Handle remaining text (no sentence-ending punctuation)
    let remaining = text[last_end..].trim();
    if !remaining.is_empty() {
        let word_count = remaining.split_whitespace().count();
        let duration_ms = (word_count as u64 * 400).max(500);

        segments.push(TranscriptSegment {
            start_ms: current_pos,
            end_ms: current_pos + duration_ms,
            text: remaining.to_string(),
            speaker: Speaker::Unknown,
            confidence: None,
        });
    }

    // If no segments were created (text has no sentence structure), return single segment
    if segments.is_empty() && !text.trim().is_empty() {
        let word_count = text.split_whitespace().count();
        let duration_ms = (word_count as u64 * 400).max(1000);

        segments.push(TranscriptSegment {
            start_ms: 0,
            end_ms: duration_ms,
            text: text.trim().to_string(),
            speaker: Speaker::Unknown,
            confidence: None,
        });
    }

    segments
}

/// Format a timestamp in MM:SS or HH:MM:SS format
pub fn format_timestamp(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

/// Format duration in a human-readable format
pub fn format_duration(ms: u64) -> String {
    let total_seconds = ms / 1000;

    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        if seconds > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}m", minutes)
        }
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "00:00");
        assert_eq!(format_timestamp(30_000), "00:30");
        assert_eq!(format_timestamp(90_000), "01:30");
        assert_eq!(format_timestamp(3600_000), "01:00:00");
        assert_eq!(format_timestamp(3661_000), "01:01:01");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30_000), "30s");
        assert_eq!(format_duration(60_000), "1m");
        assert_eq!(format_duration(90_000), "1m 30s");
        assert_eq!(format_duration(3600_000), "1h 0m");
        assert_eq!(format_duration(3900_000), "1h 5m");
    }

    #[test]
    fn test_speaker_display() {
        assert_eq!(format!("{}", Speaker::You), "You");
        assert_eq!(format!("{}", Speaker::Others), "Others");
        assert_eq!(format!("{}", Speaker::Unknown), "Speaker");
    }

    #[test]
    fn test_transcript_segment() {
        let segment = TranscriptSegment {
            start_ms: 1000,
            end_ms: 5000,
            text: "Hello world".to_string(),
            speaker: Speaker::You,
            confidence: Some(0.95),
        };

        assert_eq!(segment.duration_ms(), 4000);
        assert!(segment.format_line().contains("You"));
        assert!(segment.format_line().contains("Hello world"));
    }

    #[test]
    fn test_transcription_service_creation() {
        let service = TranscriptionService::new();
        assert!(!service.is_ready());
        assert_eq!(service.backend(), TranscriptionBackend::Parakeet);
    }
}
