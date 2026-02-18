//! Audio capture and processing module

pub mod aec;
pub mod buffer;
pub mod capture;
pub mod denoise;
pub mod pipeline;
pub mod platform;
pub mod vad;
pub mod waveform;

// Re-export key types for convenience
pub use aec::{EchoCancellationBackend, EchoCanceller, EchoProcessingInfo};
pub use denoise::AudioDenoiser;
pub use pipeline::{AudioPipeline, PipelineConfig, PreprocessingResult};
pub use vad::{SpeechSegment, Vad, VadConfig};

use serde::{Deserialize, Serialize};

/// Standard sample rate for Whisper/Parakeet models
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Sample rate required by nnnoiseless denoiser
pub const DENOISE_SAMPLE_RATE: u32 = 48_000;

/// Mono audio (single channel)
pub const CHANNELS: u16 = 1;

/// 16-bit samples
pub const BITS_PER_SAMPLE: u16 = 16;

/// Waveform update interval in milliseconds
pub const WAVEFORM_UPDATE_MS: u64 = 50;

/// Ring buffer capacity (30 seconds at 16kHz)
pub const BUFFER_CAPACITY_SAMPLES: usize = WHISPER_SAMPLE_RATE as usize * 30;

/// Recording state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingState {
    Idle,
    Recording,
    Paused,
    Processing,
}

/// Audio channel identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioChannel {
    /// Microphone input (labeled "you")
    Mic,
    /// System audio loopback (labeled "others")
    System,
}

impl AudioChannel {
    pub fn label(&self) -> &'static str {
        match self {
            AudioChannel::Mic => "you",
            AudioChannel::System => "others",
        }
    }

    pub fn filename(&self) -> &'static str {
        match self {
            AudioChannel::Mic => "you.wav",
            AudioChannel::System => "others.wav",
        }
    }
}
