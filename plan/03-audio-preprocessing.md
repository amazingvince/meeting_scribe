# 03 - Audio Preprocessing

> **Goal:** Implement Voice Activity Detection (VAD) and audio denoising  
> **Prerequisites:** [02-audio-capture.md](./02-audio-capture.md) complete  
> **Estimated Time:** 4-5 days  
> **Outcome:** Clean, speech-segmented audio ready for transcription

---

## Table of Contents
1. [Overview](#overview)
2. [Voice Activity Detection (Silero VAD)](#voice-activity-detection-silero-vad)
3. [Audio Denoising (nnnoiseless)](#audio-denoising-nnnoiseless)
4. [Complete Preprocessing Pipeline](#complete-preprocessing-pipeline)
5. [Integration with Recording](#integration-with-recording)
6. [Verification Checklist](#verification-checklist)

---

## Overview

### Why Preprocessing?

| Step | Purpose | Benefit |
|------|---------|---------|
| **VAD** | Detect speech vs silence | Skip transcribing silence, faster processing |
| **Denoising** | Remove background noise | Better transcription accuracy |
| **Resampling** | Match model requirements | Silero needs 16kHz, nnnoiseless needs 48kHz |

### Processing Pipeline

```
Raw Audio (16kHz)
       │
       ▼
┌──────────────────┐
│   Resample to    │ ──► nnnoiseless requires 48kHz
│     48 kHz       │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│    Denoise       │ ──► RNNoise removes background noise
│  (nnnoiseless)   │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   Resample to    │ ──► VAD and Whisper need 16kHz
│     16 kHz       │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│       VAD        │ ──► Detect speech segments
│   (Silero V5)    │
└────────┬─────────┘
         │
         ▼
  Speech Segments
  with timestamps
```

---

## Voice Activity Detection (Silero VAD)

### Overview

Silero VAD V5 is a state-of-the-art voice activity detector that runs efficiently on CPU via ONNX Runtime.

**Key specs:**
- Accuracy: 99%+ on standard benchmarks
- Latency: <1ms per chunk
- Sample rates: 8kHz or 16kHz
- Chunk sizes: 256 samples (8kHz) or 512 samples (16kHz)

**Reference:** [Silero VAD GitHub](https://github.com/snakers4/silero-vad)

### Implementation

Update `src-tauri/src/audio/vad.rs`:

```rust
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
            .map(|&s| (s * i16::MAX as f32) as i16)
            .collect();

        self.detect_speech_i16(&samples_i16)
    }

    /// Process i16 audio and detect speech segments
    pub fn detect_speech_i16(&mut self, samples: &[i16]) -> Vec<SpeechSegment> {
        use voice_activity_detector::IteratorExt;

        let padding_chunks = self.ms_to_chunks(self.config.speech_pad_ms);

        let labels = samples
            .iter()
            .copied()
            .label(&mut self.detector, self.config.threshold, padding_chunks);

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
                    current_probs.push(self.config.threshold);
                    sample_idx += chunk.len();
                }
                LabeledAudio::NonSpeech(chunk) => {
                    if let Some(start) = current_start.take() {
                        let segment = self.create_segment(start, sample_idx, &current_probs);
                        if self.is_valid_segment(&segment) {
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
            if self.is_valid_segment(&segment) {
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
            .map(|&s| (s * i16::MAX as f32) as i16)
            .collect();

        // Process chunk
        if samples_i16.len() >= 512 {
            self.detector
                .predict(samples_i16.iter().copied())
                .unwrap_or(0.0)
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

    fn is_valid_segment(&self, segment: &SpeechSegment) -> bool {
        segment.duration_ms() >= self.config.min_speech_duration_ms as u64
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
pub fn extract_speech_audio(samples: &[f32], segments: &[SpeechSegment], sample_rate: u32) -> Vec<f32> {
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
}
```

**Reference:** [voice_activity_detector crate](https://crates.io/crates/voice_activity_detector)

---

## Audio Denoising (nnnoiseless)

### Overview

`nnnoiseless` is a pure Rust port of RNNoise, a neural network-based noise suppression library.

**Key specs:**
- Works at 48kHz sample rate (requires resampling)
- Processes 480 samples at a time (10ms frames)
- Very low latency, CPU efficient

**Reference:** [nnnoiseless crate](https://docs.rs/nnnoiseless/latest/nnnoiseless/)

### Implementation

Update `src-tauri/src/audio/denoise.rs`:

```rust
//! Audio denoising using nnnoiseless (RNNoise)

use anyhow::{Context, Result};
use nnnoiseless::DenoiseState;
use rubato::{FftFixedIn, Resampler};
use tracing::{debug, info};

use super::{DENOISE_SAMPLE_RATE, WHISPER_SAMPLE_RATE};

/// Frame size for nnnoiseless (fixed at 480 samples for 48kHz)
const DENOISE_FRAME_SIZE: usize = 480;

/// Audio denoiser with integrated resampling
pub struct AudioDenoiser {
    /// RNNoise denoiser state
    state: Box<DenoiseState<'static>>,
    /// Resampler 16kHz -> 48kHz
    upsampler: FftFixedIn<f32>,
    /// Resampler 48kHz -> 16kHz
    downsampler: FftFixedIn<f32>,
    /// Buffer for accumulated input samples
    input_buffer: Vec<f32>,
    /// Buffer for accumulated output samples
    output_buffer: Vec<f32>,
    /// Whether denoising is enabled
    enabled: bool,
}

impl AudioDenoiser {
    /// Create a new denoiser
    pub fn new() -> Result<Self> {
        // Create resamplers
        let upsampler = FftFixedIn::new(
            WHISPER_SAMPLE_RATE as usize,
            DENOISE_SAMPLE_RATE as usize,
            DENOISE_FRAME_SIZE, // Output chunk size
            1,                   // Sub-chunks
            1,                   // Channels
        )
        .context("Failed to create upsampler")?;

        let downsampler = FftFixedIn::new(
            DENOISE_SAMPLE_RATE as usize,
            WHISPER_SAMPLE_RATE as usize,
            160, // Output chunk size (480 * 16000/48000)
            1,
            1,
        )
        .context("Failed to create downsampler")?;

        info!("Audio denoiser initialized");

        Ok(Self {
            state: Box::new(DenoiseState::new()),
            upsampler,
            downsampler,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
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
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        if !self.enabled {
            return samples.to_vec();
        }

        // Accumulate input samples
        self.input_buffer.extend_from_slice(samples);

        // Calculate how many input samples we need for one frame
        // 480 samples at 48kHz = 160 samples at 16kHz
        let input_frame_size = 160;

        while self.input_buffer.len() >= input_frame_size {
            // Take one frame worth of input
            let input_chunk: Vec<f32> = self.input_buffer.drain(..input_frame_size).collect();

            // Upsample to 48kHz
            let upsampled = self.upsample(&input_chunk);

            // Denoise
            let denoised = self.denoise_frame(&upsampled);

            // Downsample back to 16kHz
            let output = self.downsample(&denoised);

            self.output_buffer.extend(output);
        }

        // Return accumulated output
        std::mem::take(&mut self.output_buffer)
    }

    /// Process a complete audio buffer (for file processing)
    pub fn process_buffer(&mut self, samples: &[f32]) -> Vec<f32> {
        if !self.enabled {
            return samples.to_vec();
        }

        let mut output = Vec::with_capacity(samples.len());

        // Process in chunks
        for chunk in samples.chunks(1600) {
            // ~100ms chunks
            output.extend(self.process(chunk));
        }

        // Flush any remaining samples
        output.extend(self.flush());

        output
    }

    /// Flush any remaining samples in the buffer
    pub fn flush(&mut self) -> Vec<f32> {
        if self.input_buffer.is_empty() {
            return Vec::new();
        }

        // Pad to frame size
        let input_frame_size = 160;
        while self.input_buffer.len() < input_frame_size {
            self.input_buffer.push(0.0);
        }

        self.process(&[])
    }

    /// Reset the denoiser state
    pub fn reset(&mut self) {
        self.state = Box::new(DenoiseState::new());
        self.input_buffer.clear();
        self.output_buffer.clear();
        debug!("Denoiser state reset");
    }

    fn upsample(&mut self, samples: &[f32]) -> Vec<f32> {
        let input = vec![samples.to_vec()];
        match self.upsampler.process(&input, None) {
            Ok(output) => output.into_iter().next().unwrap_or_default(),
            Err(e) => {
                debug!("Upsampling error: {}", e);
                // Fallback: simple linear interpolation
                samples
                    .iter()
                    .flat_map(|&s| vec![s, s, s])
                    .collect()
            }
        }
    }

    fn downsample(&mut self, samples: &[f32]) -> Vec<f32> {
        let input = vec![samples.to_vec()];
        match self.downsampler.process(&input, None) {
            Ok(output) => output.into_iter().next().unwrap_or_default(),
            Err(e) => {
                debug!("Downsampling error: {}", e);
                // Fallback: simple decimation
                samples.iter().step_by(3).copied().collect()
            }
        }
    }

    fn denoise_frame(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut output = vec![0.0f32; DENOISE_FRAME_SIZE];

        // nnnoiseless expects exactly 480 samples
        if samples.len() >= DENOISE_FRAME_SIZE {
            self.state.process_frame(&mut output, &samples[..DENOISE_FRAME_SIZE]);
        } else {
            // Pad if needed
            let mut padded = samples.to_vec();
            padded.resize(DENOISE_FRAME_SIZE, 0.0);
            self.state.process_frame(&mut output, &padded);
        }

        output
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
    fn test_denoiser_output_size() {
        let mut denoiser = AudioDenoiser::new().unwrap();

        // Process 1 second of audio
        let input: Vec<f32> = vec![0.0; 16000];
        let output = denoiser.process_buffer(&input);

        // Output should be approximately the same size
        // (might differ slightly due to resampling)
        let diff = (output.len() as i64 - input.len() as i64).abs();
        assert!(diff < 1000, "Output size differs too much: {} vs {}", output.len(), input.len());
    }
}
```

---

## Complete Preprocessing Pipeline

### Pipeline Implementation

Create `src-tauri/src/audio/pipeline.rs`:

```rust
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

/// Result of preprocessing
#[derive(Debug, Clone)]
pub struct PreprocessingResult {
    /// Preprocessed audio samples (denoised)
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
}

/// Audio preprocessing pipeline
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

        info!("Audio pipeline initialized (denoise={})", config.denoise_enabled);

        Ok(Self {
            denoiser,
            vad,
            config,
        })
    }

    /// Process audio samples through the pipeline
    pub fn process(&mut self, samples: &[f32], sample_rate: u32) -> PreprocessingResult {
        // Step 1: Denoise
        let denoised = if self.config.denoise_enabled {
            self.denoiser.process_buffer(samples)
        } else {
            samples.to_vec()
        };

        // Step 2: VAD
        let segments = self.vad.detect_speech(&denoised);

        // Calculate durations
        let duration_ms = (samples.len() as u64 * 1000) / sample_rate as u64;
        let speech_duration_ms: u64 = segments.iter().map(|s| s.duration_ms()).sum();

        info!(
            "Preprocessed {} ms audio: {} speech segments ({} ms speech, {:.1}% ratio)",
            duration_ms,
            segments.len(),
            speech_duration_ms,
            (speech_duration_ms as f32 / duration_ms as f32) * 100.0
        );

        PreprocessingResult {
            audio: denoised,
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

/// Combine audio segments with silence markers
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
    fn test_pipeline_with_silence() {
        let mut pipeline = AudioPipeline::new(PipelineConfig::default()).unwrap();

        let silence: Vec<f32> = vec![0.0; 16000];
        let result = pipeline.process(&silence, 16000);

        assert_eq!(result.duration_ms, 1000);
        assert!(result.segments.is_empty());
        assert_eq!(result.speech_ratio(), 0.0);
    }
}
```

Add to `src-tauri/src/audio/mod.rs`:

```rust
pub mod pipeline;
```

---

## Integration with Recording

### Update Recording Commands

Update `src-tauri/src/commands/recording.rs` to include preprocessing:

```rust
// Add to imports
use crate::audio::pipeline::{AudioPipeline, PipelineConfig, PreprocessingResult};
use crate::audio::vad::SpeechSegment;

/// Preprocess a recorded meeting
#[tauri::command]
pub async fn preprocess_meeting(
    meeting_id: String,
    config: tauri::State<'_, AppConfig>,
) -> Result<PreprocessingInfo, String> {
    let meeting_dir = config.audio_dir.join(&meeting_id);
    
    // Load audio files
    let mic_path = meeting_dir.join("you.wav");
    let system_path = meeting_dir.join("others.wav");
    
    let mut results = PreprocessingInfo {
        meeting_id,
        mic_segments: Vec::new(),
        system_segments: Vec::new(),
        mic_speech_ratio: 0.0,
        system_speech_ratio: 0.0,
    };
    
    // Create pipeline
    let mut pipeline = AudioPipeline::new(PipelineConfig::default())
        .map_err(|e| e.to_string())?;
    
    // Process mic audio
    if mic_path.exists() {
        let samples = load_wav(&mic_path).map_err(|e| e.to_string())?;
        let result = pipeline.process(&samples, WHISPER_SAMPLE_RATE);
        results.mic_segments = result.segments;
        results.mic_speech_ratio = result.speech_ratio();
        pipeline.reset();
    }
    
    // Process system audio
    if system_path.exists() {
        let samples = load_wav(&system_path).map_err(|e| e.to_string())?;
        let result = pipeline.process(&samples, WHISPER_SAMPLE_RATE);
        results.system_segments = result.segments;
        results.system_speech_ratio = result.speech_ratio();
    }
    
    Ok(results)
}

/// Load WAV file samples
fn load_wav(path: &std::path::Path) -> Result<Vec<f32>, anyhow::Error> {
    use hound::WavReader;
    
    let reader = WavReader::open(path)?;
    let spec = reader.spec();
    
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            reader
                .into_samples::<i16>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / i16::MAX as f32)
                .collect()
        }
        hound::SampleFormat::Float => {
            reader
                .into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };
    
    Ok(samples)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingInfo {
    pub meeting_id: String,
    pub mic_segments: Vec<SpeechSegment>,
    pub system_segments: Vec<SpeechSegment>,
    pub mic_speech_ratio: f32,
    pub system_speech_ratio: f32,
}
```

Don't forget to register the new command in `main.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    commands::preprocess_meeting,
])
```

---

## Verification Checklist

### ✅ Acceptance Criteria

- [ ] **VAD detects speech in audio**
  ```bash
  cd src-tauri && cargo test vad -- --nocapture
  ```

- [ ] **VAD returns empty segments for silence**

- [ ] **Denoiser processes audio without errors**
  ```bash
  cd src-tauri && cargo test denoise -- --nocapture
  ```

- [ ] **Denoiser output is same approximate length as input**

- [ ] **Pipeline combines VAD + denoising**

- [ ] **Pipeline reports speech ratio**

- [ ] **Preprocessing command returns segment data**

### 🧪 Test with Real Audio

```rust
// Add to tests
#[test]
#[ignore] // Requires test audio file
fn test_pipeline_with_real_audio() {
    let samples = load_wav(Path::new("test_audio.wav")).unwrap();
    let mut pipeline = AudioPipeline::new(PipelineConfig::default()).unwrap();
    
    let result = pipeline.process(&samples, 16000);
    
    println!("Duration: {} ms", result.duration_ms);
    println!("Speech segments: {}", result.segments.len());
    println!("Speech ratio: {:.1}%", result.speech_ratio() * 100.0);
    
    for (i, seg) in result.segments.iter().enumerate() {
        println!(
            "  Segment {}: {} - {} ms ({} ms)",
            i + 1,
            seg.start_ms,
            seg.end_ms,
            seg.duration_ms()
        );
    }
}
```

### 📝 Commit Checkpoint

```bash
git add .
git commit -m "Add audio preprocessing with VAD and denoising"
```

---

## Troubleshooting

### Common Issues

#### "Failed to create VAD"
- Ensure `voice_activity_detector` crate is properly installed
- Check ONNX runtime is available

#### Denoiser produces distorted audio
- Check sample rates match (must resample to 48kHz for nnnoiseless)
- Verify input is normalized (-1.0 to 1.0)

#### VAD detects too much/too little speech
- Adjust `threshold` in VadConfig
- Try `VadConfig::for_noisy()` in noisy environments

#### Memory usage too high
- Process audio in chunks rather than entire buffer
- Clear buffers after processing

---

## Performance Notes

| Component | Latency | Memory |
|-----------|---------|--------|
| VAD (512 samples) | <1ms | ~50MB (ONNX model) |
| Denoiser (480 samples) | <1ms | ~5MB |
| Resampling | <1ms | ~1MB |

For a 1-hour meeting (57.6M samples at 16kHz):
- VAD: ~2 seconds
- Denoising: ~3 seconds
- Total preprocessing: ~5 seconds

---

## Next Steps

With preprocessing complete, proceed to:

→ **[04-transcription-engine.md](./04-transcription-engine.md)** - Integrate transcribe-rs for speech-to-text

---

## References

- [voice_activity_detector crate](https://crates.io/crates/voice_activity_detector)
- [Silero VAD GitHub](https://github.com/snakers4/silero-vad)
- [nnnoiseless crate](https://docs.rs/nnnoiseless/latest/nnnoiseless/)
- [RNNoise](https://jmvalin.ca/demo/rnnoise/)
- [rubato resampling](https://docs.rs/rubato/latest/rubato/)
