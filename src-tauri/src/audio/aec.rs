//! Acoustic Echo Cancellation using SpeexDSP
//!
//! Removes echo from microphone input using system audio as reference signal.

use aec_rs::{Aec, AecConfig};
use tracing::{debug, info};

/// Frame size for AEC processing (10ms at 16kHz = 160 samples)
const AEC_FRAME_SIZE: usize = 160;

/// Filter length in samples (100ms tail at 16kHz = 1600 samples)
/// This accommodates typical acoustic delays (speaker → mic: 50-200ms)
const AEC_FILTER_LENGTH: i32 = 1600;

/// Sample rate for AEC processing
const AEC_SAMPLE_RATE: u32 = 16000;

/// Acoustic Echo Canceller wrapper
pub struct EchoCanceller {
    aec: Aec,
    enabled: bool,
    /// Pre-allocated buffer for i16 mic frame
    mic_frame_i16: Vec<i16>,
    /// Pre-allocated buffer for i16 reference frame
    ref_frame_i16: Vec<i16>,
    /// Pre-allocated buffer for i16 output frame
    out_frame_i16: Vec<i16>,
}

impl EchoCanceller {
    /// Create a new echo canceller
    pub fn new() -> Self {
        let config = AecConfig {
            frame_size: AEC_FRAME_SIZE,
            filter_length: AEC_FILTER_LENGTH,
            sample_rate: AEC_SAMPLE_RATE,
            enable_preprocess: true, // Also run speex denoising/AGC
        };
        let aec = Aec::new(&config);

        info!(
            "Echo canceller initialized (frame={}ms, filter={}ms)",
            AEC_FRAME_SIZE * 1000 / AEC_SAMPLE_RATE as usize,
            AEC_FILTER_LENGTH as usize * 1000 / AEC_SAMPLE_RATE as usize
        );

        Self {
            aec,
            enabled: true,
            mic_frame_i16: vec![0i16; AEC_FRAME_SIZE],
            ref_frame_i16: vec![0i16; AEC_FRAME_SIZE],
            out_frame_i16: vec![0i16; AEC_FRAME_SIZE],
        }
    }

    /// Enable or disable echo cancellation
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        info!(
            "Echo cancellation {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Check if echo cancellation is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process entire audio files in batch mode (optimized for large files)
    ///
    /// This is much faster than frame-by-frame streaming for offline processing.
    /// AEC is only applied where both mic and reference audio exist.
    /// Remaining mic samples (after reference ends) are passed through unchanged.
    ///
    /// # Arguments
    /// * `mic` - Microphone input samples (may contain echo from speakers)
    /// * `reference` - System audio samples (what was played through speakers)
    ///
    /// # Returns
    /// Echo-cancelled mic audio (same length as input mic)
    pub fn process_batch(&mut self, mic: &[f32], reference: &[f32]) -> Vec<f32> {
        // If disabled or no reference, pass through unchanged
        if !self.enabled || reference.is_empty() {
            info!("AEC skipped: disabled={}, reference_len={}", !self.enabled, reference.len());
            return mic.to_vec();
        }

        let mic_len = mic.len();
        let ref_len = reference.len();

        // Determine how many samples we can actually process with AEC
        // (limited by the shorter of mic or reference)
        let processable_samples = mic_len.min(ref_len);
        let num_frames = processable_samples / AEC_FRAME_SIZE;

        info!(
            "AEC batch: {} mic samples, {} ref samples -> {} frames to process ({:.1}s)",
            mic_len,
            ref_len,
            num_frames,
            num_frames as f32 * AEC_FRAME_SIZE as f32 / AEC_SAMPLE_RATE as f32
        );

        // Pre-allocate output buffer
        let mut output = Vec::with_capacity(mic_len);

        // Process complete frames
        let mut processed_samples = 0;
        for frame_idx in 0..num_frames {
            let start = frame_idx * AEC_FRAME_SIZE;
            let end = start + AEC_FRAME_SIZE;

            // Convert mic frame to i16 in-place
            for (i, &sample) in mic[start..end].iter().enumerate() {
                self.mic_frame_i16[i] = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            }

            // Convert reference frame to i16 in-place
            for (i, &sample) in reference[start..end].iter().enumerate() {
                self.ref_frame_i16[i] = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            }

            // Process
            self.aec
                .cancel_echo(&self.mic_frame_i16, &self.ref_frame_i16, &mut self.out_frame_i16);

            // Convert output to f32 and append
            for &sample in &self.out_frame_i16 {
                output.push(sample as f32 / 32768.0);
            }

            processed_samples = end;

            // Log progress every ~30 seconds of audio
            if frame_idx > 0 && frame_idx % 3000 == 0 {
                let progress_secs = frame_idx as f32 * AEC_FRAME_SIZE as f32 / AEC_SAMPLE_RATE as f32;
                info!("AEC progress: {:.1}s processed ({} frames)", progress_secs, frame_idx);
            }
        }

        // Handle remaining mic samples after reference runs out
        // (these pass through unchanged since we have no reference)
        if processed_samples < mic_len {
            let remaining = mic_len - processed_samples;
            info!(
                "AEC: passing through {} remaining mic samples ({:.1}s) without echo cancellation",
                remaining,
                remaining as f32 / AEC_SAMPLE_RATE as f32
            );
            output.extend_from_slice(&mic[processed_samples..]);
        }

        info!(
            "AEC batch complete: {} input -> {} output samples",
            mic_len,
            output.len()
        );

        output
    }

    /// Reset echo canceller state (for new recording)
    pub fn reset(&mut self) {
        // Recreate AEC to reset internal adaptive filter state
        let config = AecConfig {
            frame_size: AEC_FRAME_SIZE,
            filter_length: AEC_FILTER_LENGTH,
            sample_rate: AEC_SAMPLE_RATE,
            enable_preprocess: true,
        };
        self.aec = Aec::new(&config);
        debug!("Echo canceller reset");
    }
}

impl Default for EchoCanceller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_canceller_creation() {
        let ec = EchoCanceller::new();
        assert!(ec.is_enabled());
    }

    #[test]
    fn test_passthrough_when_disabled() {
        let mut ec = EchoCanceller::new();
        ec.set_enabled(false);

        let mic = vec![0.5; 320];
        let reference = vec![0.3; 320];

        let output = ec.process_batch(&mic, &reference);
        assert_eq!(output, mic);
    }

    #[test]
    fn test_passthrough_when_no_reference() {
        let mut ec = EchoCanceller::new();

        let mic = vec![0.5; 320];
        let reference: Vec<f32> = vec![];

        let output = ec.process_batch(&mic, &reference);
        assert_eq!(output, mic);
    }

    #[test]
    fn test_batch_process_with_reference() {
        let mut ec = EchoCanceller::new();

        // Create test signals
        let mic = vec![0.5; 320]; // 20ms of audio
        let reference = vec![0.3; 320];

        let output = ec.process_batch(&mic, &reference);

        // Should output same length as input
        assert_eq!(output.len(), 320);
    }

    #[test]
    fn test_batch_process_mic_longer_than_ref() {
        let mut ec = EchoCanceller::new();

        // Mic is longer than reference
        let mic = vec![0.5; 640]; // 40ms
        let reference = vec![0.3; 320]; // 20ms

        let output = ec.process_batch(&mic, &reference);

        // Output should be same length as mic
        assert_eq!(output.len(), 640);

        // Last 320 samples should be unchanged (passthrough)
        assert_eq!(&output[320..], &mic[320..]);
    }
}
