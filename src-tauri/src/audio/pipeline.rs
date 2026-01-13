//! Complete audio preprocessing pipeline

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::denoise::AudioDenoiser;
use super::vad::{SpeechSegment, Vad, VadConfig};

/// Preprocessing pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// VAD configuration
    pub vad: VadConfig,
    /// Enable denoising
    pub denoise_enabled: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            vad: VadConfig::for_meetings(),
            denoise_enabled: true,
        }
    }
}

impl PipelineConfig {
    /// Configuration with denoising disabled (faster, for clean audio)
    pub fn no_denoise() -> Self {
        Self {
            vad: VadConfig::for_meetings(),
            denoise_enabled: false,
        }
    }

    /// Configuration for noisy environments
    pub fn for_noisy() -> Self {
        Self {
            vad: VadConfig::for_noisy(),
            denoise_enabled: true,
        }
    }
}

/// Result of preprocessing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingResult {
    /// Preprocessed audio samples (denoised if enabled)
    #[serde(skip)]
    pub audio: Vec<f32>,
    /// Detected speech segments
    pub segments: Vec<SpeechSegment>,
    /// Total audio duration in milliseconds
    pub duration_ms: u64,
    /// Total speech duration in milliseconds
    pub speech_duration_ms: u64,
    /// Whether denoising was applied
    pub denoised: bool,
}

impl PreprocessingResult {
    /// Calculate speech ratio (0.0 to 1.0)
    pub fn speech_ratio(&self) -> f32 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        self.speech_duration_ms as f32 / self.duration_ms as f32
    }

    /// Check if any speech was detected
    pub fn has_speech(&self) -> bool {
        !self.segments.is_empty()
    }

    /// Get number of speech segments
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
}

/// Audio preprocessing pipeline
///
/// Combines denoising and VAD into a single processing step
pub struct AudioPipeline {
    denoiser: AudioDenoiser,
    vad: Vad,
    config: PipelineConfig,
}

impl AudioPipeline {
    /// Create a new preprocessing pipeline
    pub fn new(config: PipelineConfig) -> Result<Self> {
        let mut denoiser = AudioDenoiser::new()?;
        denoiser.set_enabled(config.denoise_enabled);

        let vad = Vad::new(config.vad.clone())?;

        info!(
            "Audio pipeline initialized (denoise={})",
            config.denoise_enabled
        );

        Ok(Self {
            denoiser,
            vad,
            config,
        })
    }

    /// Process audio samples through the pipeline
    ///
    /// Pipeline: Denoise (if enabled) -> VAD -> Return segments
    pub fn process(&mut self, samples: &[f32], sample_rate: u32) -> PreprocessingResult {
        // Step 1: Denoise (if enabled)
        let processed = if self.config.denoise_enabled {
            self.denoiser.process_buffer(samples)
        } else {
            samples.to_vec()
        };

        // Step 2: VAD
        let segments = self.vad.detect_speech(&processed);

        // Calculate durations
        let duration_ms = (samples.len() as u64 * 1000) / sample_rate as u64;
        let speech_duration_ms: u64 = segments.iter().map(|s| s.duration_ms()).sum();

        info!(
            "Preprocessed {} ms audio: {} speech segments ({} ms speech, {:.1}% ratio)",
            duration_ms,
            segments.len(),
            speech_duration_ms,
            if duration_ms > 0 {
                (speech_duration_ms as f32 / duration_ms as f32) * 100.0
            } else {
                0.0
            }
        );

        PreprocessingResult {
            audio: processed,
            segments,
            duration_ms,
            speech_duration_ms,
            denoised: self.config.denoise_enabled,
        }
    }

    /// Update configuration
    pub fn set_config(&mut self, config: PipelineConfig) -> Result<()> {
        self.denoiser.set_enabled(config.denoise_enabled);
        self.vad = Vad::new(config.vad.clone())?;
        self.config = config;
        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Reset pipeline state (for new recording)
    pub fn reset(&mut self) {
        self.denoiser.reset();
    }
}

/// Extract audio for specific segments
pub fn extract_segments(
    audio: &[f32],
    segments: &[SpeechSegment],
    sample_rate: u32,
) -> Vec<(SpeechSegment, Vec<f32>)> {
    segments
        .iter()
        .filter_map(|segment| {
            let start = segment.start_sample(sample_rate);
            let end = segment.end_sample(sample_rate).min(audio.len());

            if start < end && end <= audio.len() {
                Some((segment.clone(), audio[start..end].to_vec()))
            } else {
                None
            }
        })
        .collect()
}

/// Combine audio segments with adjusted timestamps
///
/// Returns: (combined_audio, adjusted_segments)
/// Adjusted segments have timestamps relative to the combined audio
pub fn combine_with_timestamps(
    segments: Vec<(SpeechSegment, Vec<f32>)>,
) -> (Vec<f32>, Vec<SpeechSegment>) {
    let mut combined_audio = Vec::new();
    let mut adjusted_segments = Vec::new();
    let mut current_offset = 0u64;

    for (segment, audio) in segments {
        let duration_ms = segment.duration_ms();

        adjusted_segments.push(SpeechSegment {
            start_ms: current_offset,
            end_ms: current_offset + duration_ms,
            avg_probability: segment.avg_probability,
        });

        combined_audio.extend(audio);
        current_offset += duration_ms;
    }

    (combined_audio, adjusted_segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = AudioPipeline::new(PipelineConfig::default());
        assert!(pipeline.is_ok());
    }

    #[test]
    fn test_pipeline_config_variants() {
        let default = PipelineConfig::default();
        assert!(default.denoise_enabled);

        let no_denoise = PipelineConfig::no_denoise();
        assert!(!no_denoise.denoise_enabled);

        let noisy = PipelineConfig::for_noisy();
        assert!(noisy.denoise_enabled);
        assert_eq!(noisy.vad.threshold, 0.65);
    }

    #[test]
    fn test_pipeline_with_silence() {
        let mut pipeline = AudioPipeline::new(PipelineConfig::default()).unwrap();

        let silence: Vec<f32> = vec![0.0; 16000];
        let result = pipeline.process(&silence, 16000);

        assert_eq!(result.duration_ms, 1000);
        assert!(result.segments.is_empty());
        assert_eq!(result.speech_ratio(), 0.0);
        assert!(!result.has_speech());
    }

    #[test]
    fn test_preprocessing_result() {
        let result = PreprocessingResult {
            audio: vec![],
            segments: vec![SpeechSegment {
                start_ms: 0,
                end_ms: 500,
                avg_probability: 0.8,
            }],
            duration_ms: 1000,
            speech_duration_ms: 500,
            denoised: true,
        };

        assert_eq!(result.speech_ratio(), 0.5);
        assert!(result.has_speech());
        assert_eq!(result.segment_count(), 1);
    }

    #[test]
    fn test_extract_segments() {
        let audio: Vec<f32> = (0..32000).map(|i| i as f32 / 32000.0).collect();
        let segments = vec![
            SpeechSegment {
                start_ms: 0,
                end_ms: 500,
                avg_probability: 0.8,
            },
            SpeechSegment {
                start_ms: 1000,
                end_ms: 1500,
                avg_probability: 0.9,
            },
        ];

        let extracted = extract_segments(&audio, &segments, 16000);

        assert_eq!(extracted.len(), 2);
        assert_eq!(extracted[0].1.len(), 8000); // 500ms at 16kHz
        assert_eq!(extracted[1].1.len(), 8000);
    }

    #[test]
    fn test_combine_with_timestamps() {
        let segments = vec![
            (
                SpeechSegment {
                    start_ms: 1000,
                    end_ms: 1500,
                    avg_probability: 0.8,
                },
                vec![0.1; 8000],
            ),
            (
                SpeechSegment {
                    start_ms: 3000,
                    end_ms: 3500,
                    avg_probability: 0.9,
                },
                vec![0.2; 8000],
            ),
        ];

        let (combined, adjusted) = combine_with_timestamps(segments);

        assert_eq!(combined.len(), 16000);
        assert_eq!(adjusted.len(), 2);
        assert_eq!(adjusted[0].start_ms, 0);
        assert_eq!(adjusted[0].end_ms, 500);
        assert_eq!(adjusted[1].start_ms, 500);
        assert_eq!(adjusted[1].end_ms, 1000);
    }
}
