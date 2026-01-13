//! Transcription service using transcribe-rs
//!
//! Provides speech-to-text transcription using Parakeet (default), Whisper, or Moonshine.

use anyhow::Result;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info};

use transcribe_rs::engines::parakeet::{
    ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity,
};
use transcribe_rs::{TranscriptionEngine, TranscriptionResult as TrResult};

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

        match backend {
            TranscriptionBackend::Parakeet => {
                let mut engine = ParakeetEngine::new();

                // Use int8 quantization for better performance
                let params = ParakeetModelParams::int8();
                engine
                    .load_model_with_params(&model_path, params)
                    .map_err(|e| anyhow::anyhow!("Failed to load Parakeet model: {}", e))?;

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

    /// Transcribe a WAV file
    ///
    /// The WAV file should be 16kHz, mono, 16-bit PCM.
    pub fn transcribe_file(&self, wav_path: &Path) -> Result<Vec<TranscriptSegment>> {
        if !wav_path.exists() {
            anyhow::bail!("Audio file not found: {:?}", wav_path);
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

                debug!("Transcribing file: {:?}", wav_path);

                let result = engine
                    .transcribe_file(wav_path, Some(params))
                    .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

                info!(
                    "Transcribed {} segments from {:?}",
                    result.segments.as_ref().map(|s| s.len()).unwrap_or(0),
                    wav_path
                );

                Ok(convert_result(result))
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
            // No segments, create a single segment from the full text
            if result.text.trim().is_empty() {
                Vec::new()
            } else {
                vec![TranscriptSegment {
                    start_ms: 0,
                    end_ms: 0,
                    text: result.text.trim().to_string(),
                    speaker: Speaker::Unknown,
                    confidence: None,
                }]
            }
        }
    }
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
