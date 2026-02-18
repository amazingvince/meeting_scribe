//! Thread-safe ring buffer management for audio capture

use parking_lot::RwLock;
use ringbuf::{traits::*, HeapRb};
use std::sync::Arc;

use super::{AudioChannel, BUFFER_CAPACITY_SAMPLES};

/// Thread-safe audio buffer for a single channel
pub struct AudioBuffer {
    /// The underlying ring buffer
    buffer: Arc<RwLock<HeapRb<f32>>>,
    /// Channel identifier
    channel: AudioChannel,
}

impl AudioBuffer {
    /// Create a new audio buffer with default capacity (30 seconds)
    pub fn new(channel: AudioChannel) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(HeapRb::new(BUFFER_CAPACITY_SAMPLES))),
            channel,
        }
    }

    /// Push samples into the buffer
    pub fn push_samples(&self, samples: &[f32]) {
        let mut buffer = self.buffer.write();
        for &sample in samples {
            // If buffer is full, oldest samples are overwritten
            if buffer.try_push(sample).is_err() {
                // Buffer full, pop oldest and push new
                let _ = buffer.try_pop();
                let _ = buffer.try_push(sample);
            }
        }
    }

    /// Read samples without consuming them (for waveform visualization)
    pub fn peek_samples(&self, max_samples: usize) -> Vec<f32> {
        let buffer = self.buffer.read();
        let available = buffer.occupied_len().min(max_samples);
        buffer.iter().take(available).copied().collect()
    }

    /// Read the most recent samples without consuming them.
    pub fn peek_latest_samples(&self, max_samples: usize) -> Vec<f32> {
        let buffer = self.buffer.read();
        let occupied = buffer.occupied_len();
        if occupied == 0 {
            return Vec::new();
        }
        let to_take = occupied.min(max_samples);
        let to_skip = occupied.saturating_sub(to_take);
        buffer.iter().skip(to_skip).copied().collect()
    }

    /// Consume and return all samples
    pub fn drain_samples(&self) -> Vec<f32> {
        let mut buffer = self.buffer.write();
        let mut samples = Vec::with_capacity(buffer.occupied_len());
        while let Some(sample) = buffer.try_pop() {
            samples.push(sample);
        }
        samples
    }

    /// Get current buffer occupancy
    pub fn len(&self) -> usize {
        self.buffer.read().occupied_len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the buffer
    pub fn clear(&self) {
        self.buffer.write().clear();
    }

    /// Get channel identifier
    pub fn channel(&self) -> AudioChannel {
        self.channel
    }
}

impl Clone for AudioBuffer {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            channel: self.channel,
        }
    }
}

/// Manager for both audio channels
pub struct AudioBufferManager {
    pub mic: AudioBuffer,
    pub mic_clean: AudioBuffer,
    pub system: AudioBuffer,
    /// Rolling preview buffer (cleaned mic, not drained by recorder writes).
    pub mic_preview: AudioBuffer,
    /// Rolling preview buffer (system audio, not drained by recorder writes).
    pub system_preview: AudioBuffer,
}

impl AudioBufferManager {
    pub fn new() -> Self {
        Self {
            mic: AudioBuffer::new(AudioChannel::Mic),
            // Dedicated cleaned-mic stream for playback while preserving raw mic.
            mic_clean: AudioBuffer::new(AudioChannel::Mic),
            system: AudioBuffer::new(AudioChannel::System),
            mic_preview: AudioBuffer::new(AudioChannel::Mic),
            system_preview: AudioBuffer::new(AudioChannel::System),
        }
    }

    /// Get buffer for specific channel
    pub fn get(&self, channel: AudioChannel) -> &AudioBuffer {
        match channel {
            AudioChannel::Mic => &self.mic,
            AudioChannel::System => &self.system,
        }
    }

    /// Clear all buffers
    pub fn clear_all(&self) {
        self.mic.clear();
        self.mic_clean.clear();
        self.system.clear();
        self.mic_preview.clear();
        self.system_preview.clear();
    }
}

impl Default for AudioBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_push_and_drain() {
        let buffer = AudioBuffer::new(AudioChannel::Mic);

        // Push some samples
        buffer.push_samples(&[0.1, 0.2, 0.3, 0.4, 0.5]);
        assert_eq!(buffer.len(), 5);

        // Drain samples
        let samples = buffer.drain_samples();
        assert_eq!(samples, vec![0.1, 0.2, 0.3, 0.4, 0.5]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_buffer_peek() {
        let buffer = AudioBuffer::new(AudioChannel::Mic);

        buffer.push_samples(&[0.1, 0.2, 0.3, 0.4, 0.5]);

        // Peek should not consume
        let peeked = buffer.peek_samples(3);
        assert_eq!(peeked, vec![0.1, 0.2, 0.3]);
        assert_eq!(buffer.len(), 5); // Still has all samples
    }

    #[test]
    fn test_buffer_peek_latest() {
        let buffer = AudioBuffer::new(AudioChannel::Mic);
        buffer.push_samples(&[0.1, 0.2, 0.3, 0.4, 0.5]);

        let latest = buffer.peek_latest_samples(3);
        assert_eq!(latest, vec![0.3, 0.4, 0.5]);
    }

    #[test]
    fn test_buffer_overflow() {
        let buffer = AudioBuffer::new(AudioChannel::Mic);

        // Buffer should handle overflow gracefully
        let large_data: Vec<f32> = (0..BUFFER_CAPACITY_SAMPLES + 100)
            .map(|i| i as f32)
            .collect();

        buffer.push_samples(&large_data);

        // Should only contain capacity samples
        assert!(buffer.len() <= BUFFER_CAPACITY_SAMPLES);
    }

    #[test]
    fn test_buffer_manager() {
        let manager = AudioBufferManager::new();

        manager.mic.push_samples(&[0.1, 0.2]);
        manager.mic_clean.push_samples(&[0.15, 0.25]);
        manager.system.push_samples(&[0.3, 0.4]);
        manager.mic_preview.push_samples(&[0.11, 0.21]);
        manager.system_preview.push_samples(&[0.31, 0.41]);

        assert_eq!(manager.get(AudioChannel::Mic).len(), 2);
        assert_eq!(manager.get(AudioChannel::System).len(), 2);
        assert_eq!(manager.mic_clean.len(), 2);
        assert_eq!(manager.mic_preview.len(), 2);
        assert_eq!(manager.system_preview.len(), 2);

        manager.clear_all();
        assert!(manager.mic.is_empty());
        assert!(manager.mic_clean.is_empty());
        assert!(manager.system.is_empty());
        assert!(manager.mic_preview.is_empty());
        assert!(manager.system_preview.is_empty());
    }
}
