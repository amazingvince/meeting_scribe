//! Waveform data for visualization

use serde::{Deserialize, Serialize};

/// Number of waveform points to send to frontend
pub const WAVEFORM_POINTS: usize = 64;

/// Waveform metrics for a single channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMetrics {
    /// Root mean square (0.0 - 1.0)
    pub rms: f32,
    /// Peak amplitude (0.0 - 1.0)
    pub peak: f32,
    /// Downsampled waveform points for rendering
    pub samples: Vec<f32>,
    /// VAD speech probability (if available)
    pub speech_probability: Option<f32>,
}

impl ChannelMetrics {
    /// Calculate metrics from raw samples
    pub fn from_samples(samples: &[f32], downsample_to: usize) -> Self {
        if samples.is_empty() {
            return Self::empty(downsample_to);
        }

        // Calculate RMS
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_squares / samples.len() as f32).sqrt();

        // Calculate peak
        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

        // Downsample for visualization
        let step = (samples.len() / downsample_to).max(1);
        let downsampled: Vec<f32> = samples
            .chunks(step)
            .take(downsample_to)
            .map(|chunk| {
                // Use max absolute value in each chunk
                chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
            })
            .collect();

        Self {
            rms: rms.min(1.0),
            peak: peak.min(1.0),
            samples: downsampled,
            speech_probability: None,
        }
    }

    /// Create empty metrics
    pub fn empty(downsample_to: usize) -> Self {
        Self {
            rms: 0.0,
            peak: 0.0,
            samples: vec![0.0; downsample_to],
            speech_probability: None,
        }
    }
}

/// Combined waveform update for both channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformUpdate {
    /// Timestamp in milliseconds since recording start
    pub timestamp_ms: u64,
    /// Mic channel metrics
    pub mic: ChannelMetrics,
    /// System audio channel metrics
    pub system: ChannelMetrics,
    /// Recording duration in milliseconds
    pub duration_ms: u64,
}

/// Calculate waveform update from buffers
pub fn calculate_waveform(
    mic_samples: &[f32],
    system_samples: &[f32],
    timestamp_ms: u64,
    duration_ms: u64,
) -> WaveformUpdate {
    WaveformUpdate {
        timestamp_ms,
        mic: ChannelMetrics::from_samples(mic_samples, WAVEFORM_POINTS),
        system: ChannelMetrics::from_samples(system_samples, WAVEFORM_POINTS),
        duration_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_metrics() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let metrics = ChannelMetrics::from_samples(&samples, 64);

        assert!(metrics.rms > 0.0);
        assert!(metrics.peak <= 1.0);
        assert_eq!(metrics.samples.len(), 64);
    }

    #[test]
    fn test_empty_samples() {
        let metrics = ChannelMetrics::from_samples(&[], 64);

        assert_eq!(metrics.rms, 0.0);
        assert_eq!(metrics.peak, 0.0);
        assert_eq!(metrics.samples.len(), 64);
    }

    #[test]
    fn test_calculate_waveform() {
        let mic = vec![0.1, 0.2, 0.3];
        let system = vec![0.4, 0.5, 0.6];

        let update = calculate_waveform(&mic, &system, 1000, 2000);

        assert_eq!(update.timestamp_ms, 1000);
        assert_eq!(update.duration_ms, 2000);
        assert!(update.mic.rms > 0.0);
        assert!(update.system.rms > 0.0);
    }
}
