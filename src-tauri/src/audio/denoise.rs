//! Audio denoising using nnnoiseless (RNNoise)

use anyhow::{Context, Result};
use nnnoiseless::DenoiseState;
use tracing::{debug, info};

use super::capture::BufferedResampler;
use super::{DENOISE_SAMPLE_RATE, WHISPER_SAMPLE_RATE};

/// Frame size for nnnoiseless (fixed at 480 samples for 48kHz = 10ms)
const DENOISE_FRAME_SIZE: usize = 480;

/// Audio denoiser with integrated resampling
///
/// Pipeline: 16kHz input -> upsample to 48kHz -> denoise -> downsample to 16kHz
pub struct AudioDenoiser {
    /// RNNoise denoiser state
    state: Box<DenoiseState<'static>>,
    /// Resampler 16kHz -> 48kHz
    upsampler: Option<BufferedResampler>,
    /// Resampler 48kHz -> 16kHz
    downsampler: Option<BufferedResampler>,
    /// Buffer for accumulated 48kHz samples before denoising
    denoise_buffer: Vec<f32>,
    /// Whether denoising is enabled
    enabled: bool,
}

impl AudioDenoiser {
    /// Create a new denoiser
    pub fn new() -> Result<Self> {
        // Create resamplers
        let upsampler = BufferedResampler::new(WHISPER_SAMPLE_RATE, DENOISE_SAMPLE_RATE)
            .context("Failed to create upsampler")?;

        let downsampler = BufferedResampler::new(DENOISE_SAMPLE_RATE, WHISPER_SAMPLE_RATE)
            .context("Failed to create downsampler")?;

        info!("Audio denoiser initialized (16kHz -> 48kHz -> 16kHz)");

        Ok(Self {
            state: DenoiseState::new(),
            upsampler: Some(upsampler),
            downsampler: Some(downsampler),
            denoise_buffer: Vec::with_capacity(DENOISE_FRAME_SIZE * 4),
            enabled: true,
        })
    }

    /// Enable or disable denoising
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        info!("Denoising {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Check if denoising is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process audio samples (16kHz input -> 16kHz output)
    ///
    /// For streaming use - may return fewer samples than input due to buffering
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        if !self.enabled || samples.is_empty() {
            return samples.to_vec();
        }

        // Step 1: Upsample 16kHz -> 48kHz
        let upsampled = if let Some(ref mut upsampler) = self.upsampler {
            match upsampler.process(samples) {
                Ok(s) => s,
                Err(e) => {
                    debug!("Upsampling error: {}, passing through", e);
                    return samples.to_vec();
                }
            }
        } else {
            return samples.to_vec();
        };

        if upsampled.is_empty() {
            return Vec::new();
        }

        // Accumulate upsampled samples
        self.denoise_buffer.extend(upsampled);

        // Step 2: Denoise in 480-sample frames
        let mut denoised_48k = Vec::new();
        while self.denoise_buffer.len() >= DENOISE_FRAME_SIZE {
            let frame: Vec<f32> = self.denoise_buffer.drain(..DENOISE_FRAME_SIZE).collect();
            let mut output = vec![0.0f32; DENOISE_FRAME_SIZE];
            self.state.process_frame(&mut output, &frame);
            denoised_48k.extend(output);
        }

        if denoised_48k.is_empty() {
            return Vec::new();
        }

        // Step 3: Downsample 48kHz -> 16kHz
        if let Some(ref mut downsampler) = self.downsampler {
            match downsampler.process(&denoised_48k) {
                Ok(s) => s,
                Err(e) => {
                    debug!("Downsampling error: {}, returning denoised 48kHz", e);
                    // Fallback: simple decimation
                    denoised_48k.iter().step_by(3).copied().collect()
                }
            }
        } else {
            denoised_48k.iter().step_by(3).copied().collect()
        }
    }

    /// Process a complete audio buffer (for file/batch processing)
    ///
    /// More efficient for processing entire recordings
    pub fn process_buffer(&mut self, samples: &[f32]) -> Vec<f32> {
        if !self.enabled || samples.is_empty() {
            return samples.to_vec();
        }

        let mut output = Vec::with_capacity(samples.len());

        // Process in chunks to avoid memory issues with large files
        for chunk in samples.chunks(16000) {
            // ~1 second chunks
            output.extend(self.process(chunk));
        }

        // Flush any remaining samples
        output.extend(self.flush());

        output
    }

    /// Flush any remaining samples in the buffers
    pub fn flush(&mut self) -> Vec<f32> {
        let mut output = Vec::new();

        // Process any remaining samples in denoise buffer
        if !self.denoise_buffer.is_empty() {
            // Pad to frame size
            while self.denoise_buffer.len() < DENOISE_FRAME_SIZE {
                self.denoise_buffer.push(0.0);
            }

            let frame: Vec<f32> = self.denoise_buffer.drain(..DENOISE_FRAME_SIZE).collect();
            let mut denoised = vec![0.0f32; DENOISE_FRAME_SIZE];
            self.state.process_frame(&mut denoised, &frame);

            // Downsample the final frame
            if let Some(ref mut downsampler) = self.downsampler {
                if let Ok(downsampled) = downsampler.process(&denoised) {
                    output.extend(downsampled);
                }
            }
        }

        output
    }

    /// Reset the denoiser state (for new recording)
    pub fn reset(&mut self) {
        self.state = DenoiseState::new();
        self.denoise_buffer.clear();

        // Recreate resamplers to reset their state
        if let Ok(up) = BufferedResampler::new(WHISPER_SAMPLE_RATE, DENOISE_SAMPLE_RATE) {
            self.upsampler = Some(up);
        }
        if let Ok(down) = BufferedResampler::new(DENOISE_SAMPLE_RATE, WHISPER_SAMPLE_RATE) {
            self.downsampler = Some(down);
        }

        debug!("Denoiser state reset");
    }
}

impl Default for AudioDenoiser {
    fn default() -> Self {
        Self::new().expect("Failed to create default denoiser")
    }
}

/// Simple denoising function for one-shot processing
pub fn denoise_audio(samples: &[f32]) -> Result<Vec<f32>> {
    let mut denoiser = AudioDenoiser::new()?;
    Ok(denoiser.process_buffer(samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_denoiser_creation() {
        let denoiser = AudioDenoiser::new();
        assert!(denoiser.is_ok());
    }

    #[test]
    fn test_denoiser_passthrough_when_disabled() {
        let mut denoiser = AudioDenoiser::new().unwrap();
        denoiser.set_enabled(false);

        let input: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let output = denoiser.process(&input);

        assert_eq!(input, output);
    }

    #[test]
    fn test_denoiser_output_approximate_size() {
        let mut denoiser = AudioDenoiser::new().unwrap();

        // Process 1 second of audio (16000 samples at 16kHz)
        let input: Vec<f32> = vec![0.1; 16000];
        let output = denoiser.process_buffer(&input);

        // Output should be approximately the same size
        // Allow some variance due to resampling and buffering
        let diff = (output.len() as i64 - input.len() as i64).abs();
        assert!(
            diff < 2000,
            "Output size differs too much: {} vs {} (diff: {})",
            output.len(),
            input.len(),
            diff
        );
    }

    #[test]
    fn test_denoiser_reset() {
        let mut denoiser = AudioDenoiser::new().unwrap();

        // Process some audio
        let input: Vec<f32> = vec![0.1; 1600];
        let _ = denoiser.process(&input);

        // Reset
        denoiser.reset();

        // Should work after reset
        let output = denoiser.process(&input);
        assert!(!output.is_empty() || input.len() < 160); // May be empty due to buffering
    }

    #[test]
    fn test_denoise_audio_function() {
        let input: Vec<f32> = vec![0.1; 16000];
        let result = denoise_audio(&input);
        assert!(result.is_ok());
    }
}
