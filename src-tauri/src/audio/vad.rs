//! Voice Activity Detection using Silero VAD V5

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use voice_activity_detector::{LabeledAudio, VoiceActivityDetector};

use super::WHISPER_SAMPLE_RATE;

/// VAD configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Speech detection threshold (0.0-1.0)
    /// Higher = stricter (fewer false positives, might miss quiet speech)
    pub threshold: f32,

    /// Minimum speech duration in milliseconds
    /// Filters out very short sounds (clicks, etc.)
    pub min_speech_duration_ms: u32,

    /// Minimum silence duration before ending a speech segment
    /// Higher = more context kept together, fewer segments
    pub min_silence_duration_ms: u32,

    /// Padding added before/after speech segments (milliseconds)
    /// Prevents cutting off word beginnings/endings
    pub speech_pad_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            min_speech_duration_ms: 250,
            min_silence_duration_ms: 300,
            speech_pad_ms: 30,
        }
    }
}

impl VadConfig {
    /// Configuration optimized for meeting transcription
    pub fn for_meetings() -> Self {
        Self {
            threshold: 0.5,
            min_speech_duration_ms: 250,
            min_silence_duration_ms: 500, // Keep sentences together
            speech_pad_ms: 50,
        }
    }

    /// Configuration for noisy environments
    pub fn for_noisy() -> Self {
        Self {
            threshold: 0.65, // Higher threshold
            min_speech_duration_ms: 300,
            min_silence_duration_ms: 400,
            speech_pad_ms: 30,
        }
    }
}

/// A detected speech segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechSegment {
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// Average speech probability for this segment
    pub avg_probability: f32,
}

impl SpeechSegment {
    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.end_ms - self.start_ms
    }

    /// Get start sample index
    pub fn start_sample(&self, sample_rate: u32) -> usize {
        ((self.start_ms as u64 * sample_rate as u64) / 1000) as usize
    }

    /// Get end sample index
    pub fn end_sample(&self, sample_rate: u32) -> usize {
        ((self.end_ms as u64 * sample_rate as u64) / 1000) as usize
    }
}

/// Voice Activity Detector wrapper
pub struct Vad {
    detector: VoiceActivityDetector,
    config: VadConfig,
    sample_rate: u32,
}

impl Vad {
    /// Create a new VAD instance
    pub fn new(config: VadConfig) -> Result<Self> {
        let detector = VoiceActivityDetector::builder()
            .sample_rate(WHISPER_SAMPLE_RATE)
            .chunk_size(512usize) // Required for 16kHz
            .build()
            .context("Failed to create VAD")?;

        info!("VAD initialized with threshold {}", config.threshold);

        Ok(Self {
            detector,
            config,
            sample_rate: WHISPER_SAMPLE_RATE,
        })
    }

    /// Process audio and detect speech segments
    pub fn detect_speech(&mut self, samples: &[f32]) -> Vec<SpeechSegment> {
        // Convert f32 to i16 (VAD expects i16)
        let samples_i16: Vec<i16> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();

        self.detect_speech_i16(&samples_i16)
    }

    /// Process i16 audio and detect speech segments
    pub fn detect_speech_i16(&mut self, samples: &[i16]) -> Vec<SpeechSegment> {
        use voice_activity_detector::IteratorExt;

        let padding_chunks = self.ms_to_chunks(self.config.speech_pad_ms);
        let threshold = self.config.threshold;
        let min_speech_duration_ms = self.config.min_speech_duration_ms;

        // Collect labels first to release the mutable borrow on detector
        let labels: Vec<LabeledAudio<i16>> = samples
            .iter()
            .copied()
            .label(&mut self.detector, threshold, padding_chunks)
            .collect();

        let mut segments = Vec::new();
        let mut current_start: Option<usize> = None;
        let mut current_probs: Vec<f32> = Vec::new();
        let mut sample_idx = 0;

        for label in labels {
            match label {
                LabeledAudio::Speech(chunk) => {
                    if current_start.is_none() {
                        current_start = Some(sample_idx);
                    }
                    // Note: chunk probability not directly available in this API
                    current_probs.push(threshold);
                    sample_idx += chunk.len();
                }
                LabeledAudio::NonSpeech(chunk) => {
                    if let Some(start) = current_start.take() {
                        let segment = self.create_segment(start, sample_idx, &current_probs);
                        if segment.duration_ms() >= min_speech_duration_ms as u64 {
                            segments.push(segment);
                        }
                        current_probs.clear();
                    }
                    sample_idx += chunk.len();
                }
            }
        }

        // Handle trailing speech
        if let Some(start) = current_start {
            let segment = self.create_segment(start, sample_idx, &current_probs);
            if segment.duration_ms() >= min_speech_duration_ms as u64 {
                segments.push(segment);
            }
        }

        // Merge segments that are close together
        let merged = self.merge_close_segments(segments);

        debug!("Detected {} speech segments", merged.len());
        merged
    }

    /// Get speech probability for a single chunk (real-time use)
    pub fn get_speech_probability(&mut self, samples: &[f32]) -> f32 {
        // Convert to i16
        let samples_i16: Vec<i16> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();

        // Process chunk - predict returns f32 directly
        if samples_i16.len() >= 512 {
            self.detector.predict(samples_i16.iter().copied())
        } else {
            0.0
        }
    }

    fn create_segment(
        &self,
        start_sample: usize,
        end_sample: usize,
        probs: &[f32],
    ) -> SpeechSegment {
        let avg_prob = if probs.is_empty() {
            0.0
        } else {
            probs.iter().sum::<f32>() / probs.len() as f32
        };

        SpeechSegment {
            start_ms: self.samples_to_ms(start_sample),
            end_ms: self.samples_to_ms(end_sample),
            avg_probability: avg_prob,
        }
    }

    fn merge_close_segments(&self, segments: Vec<SpeechSegment>) -> Vec<SpeechSegment> {
        if segments.is_empty() {
            return segments;
        }

        let mut merged = Vec::new();
        let mut current = segments[0].clone();

        for segment in segments.into_iter().skip(1) {
            let gap = segment.start_ms.saturating_sub(current.end_ms);

            if gap <= self.config.min_silence_duration_ms as u64 {
                // Merge segments
                current.end_ms = segment.end_ms;
                current.avg_probability =
                    (current.avg_probability + segment.avg_probability) / 2.0;
            } else {
                merged.push(current);
                current = segment;
            }
        }

        merged.push(current);
        merged
    }

    fn samples_to_ms(&self, samples: usize) -> u64 {
        (samples as u64 * 1000) / self.sample_rate as u64
    }

    fn ms_to_chunks(&self, ms: u32) -> usize {
        let samples = (ms as usize * self.sample_rate as usize) / 1000;
        samples / 512 // 512 samples per chunk at 16kHz
    }
}

/// Extract speech audio from samples using VAD segments
pub fn extract_speech_audio(
    samples: &[f32],
    segments: &[SpeechSegment],
    sample_rate: u32,
) -> Vec<f32> {
    let mut speech_audio = Vec::new();

    for segment in segments {
        let start = segment.start_sample(sample_rate);
        let end = segment.end_sample(sample_rate).min(samples.len());

        if start < end && end <= samples.len() {
            speech_audio.extend_from_slice(&samples[start..end]);
        }
    }

    speech_audio
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_config_defaults() {
        let config = VadConfig::default();
        assert_eq!(config.threshold, 0.5);
        assert_eq!(config.min_speech_duration_ms, 250);
    }

    #[test]
    fn test_speech_segment() {
        let segment = SpeechSegment {
            start_ms: 1000,
            end_ms: 2000,
            avg_probability: 0.8,
        };

        assert_eq!(segment.duration_ms(), 1000);
        assert_eq!(segment.start_sample(16000), 16000);
        assert_eq!(segment.end_sample(16000), 32000);
    }

    #[test]
    fn test_vad_creation() {
        let vad = Vad::new(VadConfig::default());
        assert!(vad.is_ok());
    }

    #[test]
    fn test_vad_with_silence() {
        let mut vad = Vad::new(VadConfig::default()).unwrap();

        // Create 1 second of silence
        let silence: Vec<f32> = vec![0.0; 16000];
        let segments = vad.detect_speech(&silence);

        assert!(segments.is_empty(), "Should detect no speech in silence");
    }

    #[test]
    fn test_extract_speech_audio() {
        let samples: Vec<f32> = (0..32000).map(|i| i as f32 / 32000.0).collect();
        let segments = vec![SpeechSegment {
            start_ms: 500,
            end_ms: 1000,
            avg_probability: 0.8,
        }];

        let extracted = extract_speech_audio(&samples, &segments, 16000);

        // Should extract 500ms = 8000 samples
        assert_eq!(extracted.len(), 8000);
    }
}
