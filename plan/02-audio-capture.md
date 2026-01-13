# 02 - Audio Capture

> **Goal:** Implement microphone and system audio capture with real-time waveform visualization  
> **Prerequisites:** [01-project-setup.md](./01-project-setup.md) complete  
> **Estimated Time:** 5-6 days  
> **Outcome:** Working audio capture with dual channels (mic + system) on Windows

---

## Table of Contents
1. [Overview](#overview)
2. [Audio Format Standards](#audio-format-standards)
3. [Ring Buffer Architecture](#ring-buffer-architecture)
4. [Microphone Capture (cpal)](#microphone-capture-cpal)
5. [System Audio Loopback (Windows)](#system-audio-loopback-windows)
6. [WAV File Recording](#wav-file-recording)
7. [Waveform Visualization](#waveform-visualization)
8. [Tauri Commands Integration](#tauri-commands-integration)
9. [Frontend Components](#frontend-components)
10. [Verification Checklist](#verification-checklist)

---

## Overview

Meeting Scribe captures two audio streams simultaneously:

| Stream | Source | Label | Purpose |
|--------|--------|-------|---------|
| **Mic Input** | Default microphone | `"you"` | Your voice |
| **System Audio** | System loopback | `"others"` | Meeting participants, shared audio |

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     AUDIO CAPTURE LAYER                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────┐     ┌──────────────────┐             │
│  │   Mic Input      │     │  System Audio    │             │
│  │   (cpal)         │     │  (WASAPI Loop)   │             │
│  │                  │     │                  │             │
│  │  16kHz mono      │     │  → resample to   │             │
│  │                  │     │    16kHz mono    │             │
│  └────────┬─────────┘     └────────┬─────────┘             │
│           │                        │                        │
│           ▼                        ▼                        │
│  ┌──────────────────────────────────────────┐              │
│  │            Ring Buffer Manager            │              │
│  │                                           │              │
│  │   mic_buffer ─────────┬────────► WAV     │              │
│  │                       │          file    │              │
│  │   system_buffer ──────┴────────► WAV     │              │
│  │                       │          file    │              │
│  │                       │                   │              │
│  │                       ▼                   │              │
│  │               Waveform Data               │              │
│  │               (every 50ms)                │              │
│  └──────────────────────────────────────────┘              │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Audio Format Standards

### Constants Definition

Create `src-tauri/src/audio/mod.rs`:

```rust
//! Audio capture and processing module

pub mod buffer;
pub mod capture;
pub mod denoise;
pub mod vad;

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
```

**Key References:**
- [Whisper.cpp Audio Requirements](https://github.com/ggerganov/whisper.cpp#quick-start)
- [cpal Sample Formats](https://docs.rs/cpal/latest/cpal/enum.SampleFormat.html)

---

## Ring Buffer Architecture

### Buffer Implementation

Create `src-tauri/src/audio/buffer.rs`:

```rust
//! Thread-safe ring buffer management for audio capture

use parking_lot::RwLock;
use ringbuf::{traits::*, HeapRb};
use std::sync::Arc;

use super::{AudioChannel, BUFFER_CAPACITY_SAMPLES, WHISPER_SAMPLE_RATE};

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
            let _ = buffer.try_push(sample);
        }
    }

    /// Read all available samples without consuming them
    pub fn peek_samples(&self, max_samples: usize) -> Vec<f32> {
        let buffer = self.buffer.read();
        let available = buffer.occupied_len().min(max_samples);
        buffer.iter().take(available).copied().collect()
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
    pub system: AudioBuffer,
}

impl AudioBufferManager {
    pub fn new() -> Self {
        Self {
            mic: AudioBuffer::new(AudioChannel::Mic),
            system: AudioBuffer::new(AudioChannel::System),
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
        self.system.clear();
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
    fn test_buffer_overflow() {
        // Create tiny buffer for testing
        let buffer = AudioBuffer::new(AudioChannel::Mic);
        
        // Buffer should handle overflow gracefully
        let large_data: Vec<f32> = (0..BUFFER_CAPACITY_SAMPLES + 100)
            .map(|i| i as f32)
            .collect();
        
        buffer.push_samples(&large_data);
        
        // Should only contain capacity samples
        assert!(buffer.len() <= BUFFER_CAPACITY_SAMPLES);
    }
}
```

**Reference:** [ringbuf crate documentation](https://docs.rs/ringbuf/latest/ringbuf/)

---

## Microphone Capture (cpal)

### Capture Implementation

Create `src-tauri/src/audio/capture.rs`:

```rust
//! Audio capture using cpal

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::buffer::AudioBuffer;
use super::{AudioChannel, CHANNELS, WHISPER_SAMPLE_RATE};

/// Audio capture manager for a single input device
pub struct AudioCapture {
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
}

impl AudioCapture {
    /// Create capture for default input device (microphone)
    pub fn new_microphone() -> Result<Self> {
        let host = cpal::default_host();
        
        let device = host
            .default_input_device()
            .context("No input device available")?;
        
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using input device: {}", device_name);
        
        // Get supported config
        let supported_config = device
            .default_input_config()
            .context("Failed to get default input config")?;
        
        debug!("Device config: {:?}", supported_config);
        
        // Build our desired config
        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: SampleRate(WHISPER_SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };
        
        Ok(Self {
            device,
            config,
            stream: None,
            buffer: AudioBuffer::new(AudioChannel::Mic),
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start capturing audio
    pub fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("Capture already running");
            return Ok(());
        }

        let buffer = self.buffer.clone();
        let is_running = Arc::clone(&self.is_running);

        // Get the sample format from device
        let supported = self.device.default_input_config()?;
        let sample_format = supported.sample_format();

        let err_fn = |err| error!("Audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => self.build_stream::<f32>(buffer, err_fn)?,
            SampleFormat::I16 => self.build_stream::<i16>(buffer, err_fn)?,
            SampleFormat::U16 => self.build_stream::<u16>(buffer, err_fn)?,
            _ => anyhow::bail!("Unsupported sample format: {:?}", sample_format),
        };

        stream.play().context("Failed to start audio stream")?;
        
        self.stream = Some(stream);
        is_running.store(true, Ordering::SeqCst);
        
        info!("Audio capture started");
        Ok(())
    }

    /// Build audio stream for specific sample type
    fn build_stream<T>(
        &self,
        buffer: AudioBuffer,
        err_fn: impl Fn(cpal::StreamError) + Send + 'static,
    ) -> Result<Stream>
    where
        T: cpal::Sample + cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let config = self.config.clone();
        
        let stream = self.device.build_input_stream(
            &config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Convert to f32 and push to buffer
                let samples: Vec<f32> = data
                    .iter()
                    .map(|&s| cpal::Sample::from_sample(s))
                    .collect();
                buffer.push_samples(&samples);
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    /// Stop capturing
    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.is_running.store(false, Ordering::SeqCst);
        info!("Audio capture stopped");
    }

    /// Check if currently capturing
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Get reference to buffer
    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }

    /// Get device name
    pub fn device_name(&self) -> String {
        self.device.name().unwrap_or_else(|_| "Unknown".to_string())
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// List available input devices
pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices: Vec<String> = host
        .input_devices()?
        .filter_map(|d| d.name().ok())
        .collect();
    Ok(devices)
}

/// List available output devices (for loopback)
pub fn list_output_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices: Vec<String> = host
        .output_devices()?
        .filter_map(|d| d.name().ok())
        .collect();
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    #[ignore] // Requires audio hardware
    fn test_microphone_capture() {
        let mut capture = AudioCapture::new_microphone().unwrap();
        capture.start().unwrap();
        
        // Record for 1 second
        thread::sleep(Duration::from_secs(1));
        
        capture.stop();
        
        let samples = capture.buffer().drain_samples();
        println!("Captured {} samples", samples.len());
        
        // Should have approximately 16000 samples for 1 second at 16kHz
        assert!(samples.len() > 10000);
    }
}
```

**Key References:**
- [cpal documentation](https://docs.rs/cpal/latest/cpal/)
- [cpal examples](https://github.com/RustAudio/cpal/tree/master/examples)
- [Sample format conversion](https://docs.rs/cpal/latest/cpal/trait.FromSample.html)

---

## System Audio Loopback (Windows)

### WASAPI Loopback Implementation

Create `src-tauri/src/audio/platform/mod.rs`:

```rust
//! Platform-specific audio capture

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

// Re-export platform-specific loopback capture
#[cfg(target_os = "windows")]
pub use windows::SystemAudioCapture;

#[cfg(target_os = "macos")]
pub use macos::SystemAudioCapture;

#[cfg(target_os = "linux")]
pub use linux::SystemAudioCapture;
```

Create `src-tauri/src/audio/platform/windows.rs`:

```rust
//! Windows system audio capture via WASAPI loopback

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig};
use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::audio::buffer::AudioBuffer;
use crate::audio::{AudioChannel, CHANNELS, WHISPER_SAMPLE_RATE};

/// System audio capture using WASAPI loopback
pub struct SystemAudioCapture {
    device: Device,
    stream: Option<Stream>,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
    source_sample_rate: u32,
}

impl SystemAudioCapture {
    /// Create capture for default output device (loopback)
    pub fn new() -> Result<Self> {
        // Use WASAPI host for loopback support
        let host = cpal::host_from_id(cpal::HostId::Wasapi)
            .context("WASAPI host not available")?;
        
        // Get default output device for loopback
        let device = host
            .default_output_device()
            .context("No output device available for loopback")?;
        
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using loopback device: {}", device_name);
        
        // Get device's native sample rate
        let supported_config = device
            .default_output_config()
            .context("Failed to get default output config")?;
        
        let source_sample_rate = supported_config.sample_rate().0;
        debug!(
            "Loopback device sample rate: {} Hz",
            source_sample_rate
        );
        
        Ok(Self {
            device,
            stream: None,
            buffer: AudioBuffer::new(AudioChannel::System),
            is_running: Arc::new(AtomicBool::new(false)),
            source_sample_rate,
        })
    }

    /// Start capturing system audio
    pub fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("System capture already running");
            return Ok(());
        }

        let supported = self.device.default_output_config()?;
        let sample_format = supported.sample_format();
        let source_rate = supported.sample_rate().0;
        let source_channels = supported.channels();

        // Build config for loopback capture
        let config = StreamConfig {
            channels: source_channels,
            sample_rate: SampleRate(source_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer = self.buffer.clone();
        let is_running = Arc::clone(&self.is_running);

        // Create resampler if needed
        let needs_resampling = source_rate != WHISPER_SAMPLE_RATE;
        let resampler = if needs_resampling {
            Some(create_resampler(source_rate, WHISPER_SAMPLE_RATE)?)
        } else {
            None
        };
        
        let resampler = Arc::new(parking_lot::Mutex::new(resampler));
        let source_channels_count = source_channels as usize;

        let err_fn = |err| error!("System audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => {
                self.build_stream::<f32>(config, buffer, resampler, source_channels_count, err_fn)?
            }
            SampleFormat::I16 => {
                self.build_stream::<i16>(config, buffer, resampler, source_channels_count, err_fn)?
            }
            _ => anyhow::bail!("Unsupported sample format: {:?}", sample_format),
        };

        stream.play().context("Failed to start loopback stream")?;
        
        self.stream = Some(stream);
        is_running.store(true, Ordering::SeqCst);
        
        info!("System audio capture started");
        Ok(())
    }

    fn build_stream<T>(
        &self,
        config: StreamConfig,
        buffer: AudioBuffer,
        resampler: Arc<parking_lot::Mutex<Option<SincFixedIn<f32>>>>,
        source_channels: usize,
        err_fn: impl Fn(cpal::StreamError) + Send + 'static,
    ) -> Result<Stream>
    where
        T: cpal::Sample + cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        // Note: For WASAPI loopback, we use build_input_stream on an output device
        let stream = self.device.build_input_stream(
            &config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Convert to f32
                let samples: Vec<f32> = data
                    .iter()
                    .map(|&s| cpal::Sample::from_sample(s))
                    .collect();

                // Convert to mono if stereo
                let mono_samples = if source_channels > 1 {
                    samples
                        .chunks(source_channels)
                        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
                        .collect::<Vec<f32>>()
                } else {
                    samples
                };

                // Resample if needed
                let final_samples = if let Some(ref mut resampler) = *resampler.lock() {
                    match resample(resampler, &mono_samples) {
                        Ok(resampled) => resampled,
                        Err(e) => {
                            error!("Resampling error: {}", e);
                            mono_samples
                        }
                    }
                } else {
                    mono_samples
                };

                buffer.push_samples(&final_samples);
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    /// Stop capturing
    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.is_running.store(false, Ordering::SeqCst);
        info!("System audio capture stopped");
    }

    /// Check if currently capturing
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Get reference to buffer
    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Create a high-quality resampler
fn create_resampler(from_rate: u32, to_rate: u32) -> Result<SincFixedIn<f32>> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    SincFixedIn::new(
        to_rate as f64 / from_rate as f64,
        2.0,
        params,
        1024,
        1, // mono
    )
    .context("Failed to create resampler")
}

/// Resample audio data
fn resample(resampler: &mut SincFixedIn<f32>, input: &[f32]) -> Result<Vec<f32>> {
    use rubato::Resampler;
    
    // rubato expects Vec<Vec<f32>> for multi-channel
    let input_frames = vec![input.to_vec()];
    
    let output = resampler
        .process(&input_frames, None)
        .context("Resampling failed")?;
    
    Ok(output.into_iter().next().unwrap_or_default())
}
```

**Key References:**
- [WASAPI Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- [rubato resampling](https://docs.rs/rubato/latest/rubato/)
- [cpal WASAPI backend](https://docs.rs/cpal/latest/cpal/platform/windows/index.html)

### Placeholder for macOS and Linux

Create `src-tauri/src/audio/platform/macos.rs`:

```rust
//! macOS system audio capture via ScreenCaptureKit
//! 
//! Implementation planned for step 10-cross-platform.md

use anyhow::Result;
use crate::audio::buffer::AudioBuffer;
use crate::audio::AudioChannel;

pub struct SystemAudioCapture {
    buffer: AudioBuffer,
}

impl SystemAudioCapture {
    pub fn new() -> Result<Self> {
        // TODO: Implement ScreenCaptureKit capture
        // See: https://developer.apple.com/documentation/screencapturekit
        Ok(Self {
            buffer: AudioBuffer::new(AudioChannel::System),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        anyhow::bail!("macOS system audio capture not yet implemented. See 10-cross-platform.md")
    }

    pub fn stop(&mut self) {}

    pub fn is_running(&self) -> bool {
        false
    }

    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }
}
```

Create `src-tauri/src/audio/platform/linux.rs`:

```rust
//! Linux system audio capture via PipeWire
//! 
//! Implementation planned for step 10-cross-platform.md

use anyhow::Result;
use crate::audio::buffer::AudioBuffer;
use crate::audio::AudioChannel;

pub struct SystemAudioCapture {
    buffer: AudioBuffer,
}

impl SystemAudioCapture {
    pub fn new() -> Result<Self> {
        // TODO: Implement PipeWire/PulseAudio capture
        Ok(Self {
            buffer: AudioBuffer::new(AudioChannel::System),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        anyhow::bail!("Linux system audio capture not yet implemented. See 10-cross-platform.md")
    }

    pub fn stop(&mut self) {}

    pub fn is_running(&self) -> bool {
        false
    }

    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }
}
```

Update `src-tauri/src/audio/mod.rs` to include platform module:

```rust
//! Audio capture and processing module

pub mod buffer;
pub mod capture;
pub mod denoise;
pub mod platform;
pub mod vad;

// ... rest of the file remains the same
```

---

## WAV File Recording

### WAV Writer Implementation

Add to `src-tauri/src/audio/capture.rs`:

```rust
use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

/// WAV file writer with buffered I/O
pub struct AudioRecorder {
    writer: Option<WavWriter<BufWriter<File>>>,
    path: PathBuf,
    samples_written: u64,
}

impl AudioRecorder {
    /// Create a new recorder for the given path
    pub fn new(path: PathBuf) -> Result<Self> {
        let spec = WavSpec {
            channels: CHANNELS,
            sample_rate: WHISPER_SAMPLE_RATE,
            bits_per_sample: BITS_PER_SAMPLE,
            sample_format: hound::SampleFormat::Int,
        };

        let file = File::create(&path).context("Failed to create WAV file")?;
        let buf_writer = BufWriter::new(file);
        let writer = WavWriter::new(buf_writer, spec).context("Failed to create WAV writer")?;

        info!("Created WAV file: {:?}", path);

        Ok(Self {
            writer: Some(writer),
            path,
            samples_written: 0,
        })
    }

    /// Write samples to file (f32 -> i16 conversion)
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            for &sample in samples {
                // Convert f32 [-1.0, 1.0] to i16
                let sample_i16 = (sample * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16)?;
                self.samples_written += 1;
            }
        }
        Ok(())
    }

    /// Finalize and close the file
    pub fn finalize(mut self) -> Result<PathBuf> {
        if let Some(writer) = self.writer.take() {
            writer.finalize().context("Failed to finalize WAV file")?;
        }
        
        let duration_secs = self.samples_written as f64 / WHISPER_SAMPLE_RATE as f64;
        info!(
            "Finalized WAV file: {:?} ({:.1}s, {} samples)",
            self.path, duration_secs, self.samples_written
        );
        
        Ok(self.path)
    }

    /// Get number of samples written
    pub fn samples_written(&self) -> u64 {
        self.samples_written
    }

    /// Get duration in seconds
    pub fn duration_secs(&self) -> f64 {
        self.samples_written as f64 / WHISPER_SAMPLE_RATE as f64
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            if let Err(e) = writer.finalize() {
                error!("Failed to finalize WAV on drop: {}", e);
            }
        }
    }
}
```

**Reference:** [hound WAV library](https://docs.rs/hound/latest/hound/)

---

## Waveform Visualization

### Waveform Data Structure

Create `src-tauri/src/audio/waveform.rs`:

```rust
//! Waveform data for visualization

use serde::{Deserialize, Serialize};

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
            return Self {
                rms: 0.0,
                peak: 0.0,
                samples: vec![0.0; downsample_to],
                speech_probability: None,
            };
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

/// Number of waveform points to send to frontend
pub const WAVEFORM_POINTS: usize = 64;

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
    }
}
```

Add to `src-tauri/src/audio/mod.rs`:

```rust
pub mod waveform;
```

---

## Tauri Commands Integration

### Recording Manager

Create `src-tauri/src/commands/recording.rs`:

```rust
//! Recording-related Tauri commands

use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::interval;
use tracing::{error, info};
use uuid::Uuid;

use crate::audio::buffer::AudioBufferManager;
use crate::audio::capture::{AudioCapture, AudioRecorder};
use crate::audio::platform::SystemAudioCapture;
use crate::audio::waveform::{calculate_waveform, WaveformUpdate, WAVEFORM_POINTS};
use crate::audio::{AudioChannel, RecordingState, WAVEFORM_UPDATE_MS, WHISPER_SAMPLE_RATE};
use crate::AppConfig;

/// Recording session state
pub struct RecordingSession {
    pub id: String,
    pub state: RecordingState,
    pub start_time: Option<Instant>,
    pub mic_capture: Option<AudioCapture>,
    pub system_capture: Option<SystemAudioCapture>,
    pub mic_recorder: Option<AudioRecorder>,
    pub system_recorder: Option<AudioRecorder>,
    pub buffers: AudioBufferManager,
}

impl RecordingSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            state: RecordingState::Idle,
            start_time: None,
            mic_capture: None,
            system_capture: None,
            mic_recorder: None,
            system_recorder: None,
            buffers: AudioBufferManager::new(),
        }
    }
}

/// Shared recording state type
pub type SharedRecordingSession = Arc<Mutex<RecordingSession>>;

/// Start recording
#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    session: tauri::State<'_, SharedRecordingSession>,
    config: tauri::State<'_, AppConfig>,
) -> Result<String, String> {
    let mut session = session.lock();
    
    if session.state == RecordingState::Recording {
        return Err("Already recording".to_string());
    }

    // Create new session
    let meeting_id = Uuid::new_v4().to_string();
    let meeting_dir = config.audio_dir.join(&meeting_id);
    std::fs::create_dir_all(&meeting_dir).map_err(|e| e.to_string())?;

    // Initialize captures
    let mut mic_capture = AudioCapture::new_microphone().map_err(|e| e.to_string())?;
    
    #[cfg(target_os = "windows")]
    let mut system_capture = SystemAudioCapture::new().map_err(|e| e.to_string())?;
    
    #[cfg(not(target_os = "windows"))]
    let mut system_capture = SystemAudioCapture::new().map_err(|e| e.to_string())?;

    // Initialize recorders
    let mic_recorder = AudioRecorder::new(meeting_dir.join("you.wav"))
        .map_err(|e| e.to_string())?;
    let system_recorder = AudioRecorder::new(meeting_dir.join("others.wav"))
        .map_err(|e| e.to_string())?;

    // Start captures
    mic_capture.start().map_err(|e| e.to_string())?;
    
    // System capture may fail on non-Windows, that's okay for now
    if let Err(e) = system_capture.start() {
        info!("System audio capture not available: {}", e);
    }

    session.id = meeting_id.clone();
    session.state = RecordingState::Recording;
    session.start_time = Some(Instant::now());
    session.mic_capture = Some(mic_capture);
    session.system_capture = Some(system_capture);
    session.mic_recorder = Some(mic_recorder);
    session.system_recorder = Some(system_recorder);

    // Start waveform emission task
    let session_clone = app.state::<SharedRecordingSession>().inner().clone();
    let app_clone = app.clone();
    
    tokio::spawn(async move {
        emit_waveform_loop(app_clone, session_clone).await;
    });

    info!("Recording started: {}", meeting_id);
    Ok(meeting_id)
}

/// Stop recording
#[tauri::command]
pub async fn stop_recording(
    session: tauri::State<'_, SharedRecordingSession>,
) -> Result<RecordingResult, String> {
    let mut session = session.lock();
    
    if session.state != RecordingState::Recording {
        return Err("Not currently recording".to_string());
    }

    // Stop captures
    if let Some(ref mut mic) = session.mic_capture {
        mic.stop();
    }
    if let Some(ref mut system) = session.system_capture {
        system.stop();
    }

    // Drain buffers to recorders
    drain_buffers_to_recorders(&mut session)?;

    // Finalize WAV files
    let mic_path = session
        .mic_recorder
        .take()
        .map(|r| r.finalize())
        .transpose()
        .map_err(|e| e.to_string())?;
    
    let system_path = session
        .system_recorder
        .take()
        .map(|r| r.finalize())
        .transpose()
        .map_err(|e| e.to_string())?;

    let duration_ms = session
        .start_time
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);

    let result = RecordingResult {
        meeting_id: session.id.clone(),
        duration_ms,
        mic_path: mic_path.map(|p| p.display().to_string()),
        system_path: system_path.map(|p| p.display().to_string()),
    };

    // Reset session
    session.state = RecordingState::Idle;
    session.start_time = None;
    session.mic_capture = None;
    session.system_capture = None;
    session.buffers.clear_all();

    info!("Recording stopped: {} ({}ms)", result.meeting_id, duration_ms);
    Ok(result)
}

/// Get current recording state
#[tauri::command]
pub fn get_recording_state(
    session: tauri::State<'_, SharedRecordingSession>,
) -> RecordingStateResponse {
    let session = session.lock();
    
    let duration_ms = session
        .start_time
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);
    
    RecordingStateResponse {
        state: session.state,
        meeting_id: if session.state != RecordingState::Idle {
            Some(session.id.clone())
        } else {
            None
        },
        duration_ms,
    }
}

/// List available audio devices
#[tauri::command]
pub fn list_audio_devices() -> Result<AudioDevices, String> {
    let input_devices = crate::audio::capture::list_input_devices()
        .map_err(|e| e.to_string())?;
    let output_devices = crate::audio::capture::list_output_devices()
        .map_err(|e| e.to_string())?;
    
    Ok(AudioDevices {
        input_devices,
        output_devices,
    })
}

// Helper to drain buffers to WAV files
fn drain_buffers_to_recorders(session: &mut RecordingSession) -> Result<(), String> {
    if let Some(ref mic_capture) = session.mic_capture {
        let samples = mic_capture.buffer().drain_samples();
        if let Some(ref mut recorder) = session.mic_recorder {
            recorder.write_samples(&samples).map_err(|e| e.to_string())?;
        }
    }
    
    if let Some(ref system_capture) = session.system_capture {
        let samples = system_capture.buffer().drain_samples();
        if let Some(ref mut recorder) = session.system_recorder {
            recorder.write_samples(&samples).map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}

// Waveform emission loop
async fn emit_waveform_loop(app: AppHandle, session: SharedRecordingSession) {
    let mut interval = interval(Duration::from_millis(WAVEFORM_UPDATE_MS));
    
    loop {
        interval.tick().await;
        
        let waveform = {
            let mut session = session.lock();
            
            if session.state != RecordingState::Recording {
                break;
            }
            
            let duration_ms = session
                .start_time
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(0);
            
            // Get samples from captures
            let mic_samples = session
                .mic_capture
                .as_ref()
                .map(|c| c.buffer().peek_samples(WHISPER_SAMPLE_RATE as usize / 20))
                .unwrap_or_default();
            
            let system_samples = session
                .system_capture
                .as_ref()
                .map(|c| c.buffer().peek_samples(WHISPER_SAMPLE_RATE as usize / 20))
                .unwrap_or_default();
            
            // Write to recorders periodically
            if let Err(e) = drain_buffers_to_recorders(&mut session) {
                error!("Failed to write to recorders: {}", e);
            }
            
            calculate_waveform(&mic_samples, &system_samples, duration_ms, duration_ms)
        };
        
        // Emit to frontend
        if let Err(e) = app.emit("waveform-update", &waveform) {
            error!("Failed to emit waveform: {}", e);
        }
    }
}

// Response types
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub meeting_id: String,
    pub duration_ms: u64,
    pub mic_path: Option<String>,
    pub system_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStateResponse {
    pub state: RecordingState,
    pub meeting_id: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevices {
    pub input_devices: Vec<String>,
    pub output_devices: Vec<String>,
}
```

Update `src-tauri/src/commands/mod.rs`:

```rust
//! Tauri commands - These functions are callable from the frontend via IPC

pub mod recording;

use serde::{Deserialize, Serialize};

// Re-export recording commands
pub use recording::*;

/// Basic greeting command for testing IPC
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Meeting Scribe.", name)
}

/// Application info response
#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub platform: String,
}

/// Get application information
#[tauri::command]
pub fn get_app_info(config: tauri::State<crate::AppConfig>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: config.data_dir.display().to_string(),
        platform: std::env::consts::OS.to_string(),
    }
}
```

Update `src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use meeting_scribe_lib::{commands, AppConfig};
use meeting_scribe_lib::commands::recording::{RecordingSession, SharedRecordingSession};
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meeting_scribe=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Meeting Scribe...");

    // Initialize app config
    let config = AppConfig::new().expect("Failed to create app config");
    config.ensure_dirs().expect("Failed to create directories");

    // Initialize recording session
    let recording_session: SharedRecordingSession = Arc::new(Mutex::new(RecordingSession::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(config)
        .manage(recording_session)
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_app_info,
            commands::start_recording,
            commands::stop_recording,
            commands::get_recording_state,
            commands::list_audio_devices,
        ])
        .setup(|app| {
            info!("Application setup complete");
            
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## Frontend Components

### Updated Recording View

Update `src/components/Recording/RecordingView.tsx`:

```tsx
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { Mic, Square, Pause, Play } from 'lucide-react';
import { Waveform } from './Waveform';
import { formatDuration } from '../../utils/format';

interface WaveformUpdate {
  timestamp_ms: number;
  duration_ms: number;
  mic: ChannelMetrics;
  system: ChannelMetrics;
}

interface ChannelMetrics {
  rms: number;
  peak: number;
  samples: number[];
  speech_probability: number | null;
}

interface RecordingState {
  state: 'Idle' | 'Recording' | 'Paused' | 'Processing';
  meeting_id: string | null;
  duration_ms: number;
}

export function RecordingView() {
  const [recordingState, setRecordingState] = useState<RecordingState>({
    state: 'Idle',
    meeting_id: null,
    duration_ms: 0,
  });
  const [waveform, setWaveform] = useState<WaveformUpdate | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Subscribe to waveform updates
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setup = async () => {
      unlisten = await listen<WaveformUpdate>('waveform-update', (event) => {
        setWaveform(event.payload);
        setRecordingState((prev) => ({
          ...prev,
          duration_ms: event.payload.duration_ms,
        }));
      });
    };

    setup();

    return () => {
      unlisten?.();
    };
  }, []);

  // Poll recording state on mount
  useEffect(() => {
    invoke<RecordingState>('get_recording_state').then(setRecordingState);
  }, []);

  const handleStartRecording = useCallback(async () => {
    try {
      setError(null);
      const meetingId = await invoke<string>('start_recording');
      setRecordingState({
        state: 'Recording',
        meeting_id: meetingId,
        duration_ms: 0,
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const handleStopRecording = useCallback(async () => {
    try {
      setError(null);
      const result = await invoke<{
        meeting_id: string;
        duration_ms: number;
      }>('stop_recording');
      
      setRecordingState({
        state: 'Idle',
        meeting_id: null,
        duration_ms: 0,
      });
      setWaveform(null);
      
      // TODO: Navigate to meeting detail or show success
      console.log('Recording saved:', result);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const isRecording = recordingState.state === 'Recording';

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Recording</h1>

      {error && (
        <div className="bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      <div className="card p-8">
        {/* Timer */}
        <div className="text-center mb-8">
          <div className="text-5xl font-mono font-bold">
            {formatDuration(recordingState.duration_ms)}
          </div>
          <div className="text-sm text-gray-500 mt-2">
            {isRecording ? 'Recording...' : 'Ready to record'}
          </div>
        </div>

        {/* Waveforms */}
        <div className="space-y-4 mb-8">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <div className="w-2 h-2 rounded-full bg-blue-500" />
              <span className="text-sm font-medium">You (Microphone)</span>
              {waveform?.mic.speech_probability !== null && (
                <span className="text-xs text-gray-500">
                  Speech: {Math.round((waveform?.mic.speech_probability || 0) * 100)}%
                </span>
              )}
            </div>
            <Waveform
              samples={waveform?.mic.samples || []}
              rms={waveform?.mic.rms || 0}
              color="rgb(59, 130, 246)"
              height={60}
            />
          </div>

          <div>
            <div className="flex items-center gap-2 mb-2">
              <div className="w-2 h-2 rounded-full bg-green-500" />
              <span className="text-sm font-medium">Others (System Audio)</span>
            </div>
            <Waveform
              samples={waveform?.system.samples || []}
              rms={waveform?.system.rms || 0}
              color="rgb(34, 197, 94)"
              height={60}
            />
          </div>
        </div>

        {/* Controls */}
        <div className="flex justify-center gap-4">
          {!isRecording ? (
            <button
              onClick={handleStartRecording}
              className="btn btn-primary flex items-center gap-2 text-lg px-8 py-3"
            >
              <Mic size={24} />
              Start Recording
            </button>
          ) : (
            <>
              <button
                onClick={handleStopRecording}
                className="btn bg-red-600 text-white hover:bg-red-700 flex items-center gap-2 px-6 py-3"
              >
                <Square size={20} />
                Stop
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
```

### Waveform Component

Create `src/components/Recording/Waveform.tsx`:

```tsx
import { useRef, useEffect } from 'react';

interface WaveformProps {
  samples: number[];
  rms: number;
  color: string;
  height: number;
}

export function Waveform({ samples, rms, color, height }: WaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Get actual dimensions
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    
    // Set canvas size with DPR
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const width = rect.width;
    const centerY = height / 2;

    // Clear
    ctx.clearRect(0, 0, width, height);

    // Draw background
    ctx.fillStyle = 'rgba(0, 0, 0, 0.05)';
    ctx.fillRect(0, 0, width, height);

    // Draw center line
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, centerY);
    ctx.lineTo(width, centerY);
    ctx.stroke();

    // Draw waveform bars
    if (samples.length > 0) {
      const barWidth = width / samples.length;
      const maxHeight = height * 0.8;

      ctx.fillStyle = color;

      samples.forEach((sample, i) => {
        const barHeight = sample * maxHeight;
        const x = i * barWidth;
        const y = centerY - barHeight / 2;

        ctx.fillRect(x, y, barWidth - 1, barHeight);
      });
    }

    // Draw RMS level indicator
    if (rms > 0) {
      const rmsHeight = rms * height * 0.9;
      ctx.fillStyle = color.replace('rgb', 'rgba').replace(')', ', 0.3)');
      ctx.fillRect(0, centerY - rmsHeight / 2, 4, rmsHeight);
    }
  }, [samples, rms, color, height]);

  return (
    <canvas
      ref={canvasRef}
      className="w-full rounded-lg"
      style={{ height }}
    />
  );
}
```

### Utility Functions

Create `src/utils/format.ts`:

```typescript
/**
 * Format milliseconds as HH:MM:SS
 */
export function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours.toString().padStart(2, '0')}:${minutes
      .toString()
      .padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
  }

  return `${minutes.toString().padStart(2, '0')}:${seconds
    .toString()
    .padStart(2, '0')}`;
}

/**
 * Format bytes as human-readable size
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';

  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

/**
 * Format date for display
 */
export function formatDate(date: Date | string | number): string {
  const d = new Date(date);
  return d.toLocaleDateString(undefined, {
    weekday: 'short',
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/**
 * Format time for display
 */
export function formatTime(date: Date | string | number): string {
  const d = new Date(date);
  return d.toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  });
}
```

---

## Verification Checklist

### ✅ Acceptance Criteria

- [ ] **Microphone capture works**
  ```bash
  # Should see "Audio capture started" in logs
  pnpm tauri dev
  # Click "Start Recording" and speak
  ```

- [ ] **System audio capture works** (Windows only)
  - Play audio from any app while recording
  - Should see waveform activity on "Others" channel

- [ ] **Waveform visualization updates**
  - Both channels show real-time waveform during recording
  - RMS level indicator visible

- [ ] **WAV files are created**
  - After stopping, check `~/.meeting-scribe/audio/{meeting_id}/`
  - Should contain `you.wav` and `others.wav`

- [ ] **WAV files are playable**
  ```bash
  # Play with any audio player
  ffplay ~/.meeting-scribe/audio/{meeting_id}/you.wav
  ```

- [ ] **Recording timer works**
  - Timer updates every second during recording

- [ ] **Start/stop controls work**
  - Can start and stop multiple recordings

### 🧪 Test Commands

```bash
# Run audio capture tests (requires audio hardware)
cd src-tauri && cargo test audio -- --ignored

# Check for audio devices
cd src-tauri && cargo run --example list_devices
```

### 📝 Commit Checkpoint

```bash
git add .
git commit -m "Implement audio capture with microphone and WASAPI loopback"
```

---

## Troubleshooting

### Common Issues

#### "No input device available"
- Check Windows Sound settings
- Ensure microphone is plugged in and enabled
- Grant microphone permission to the app

#### "WASAPI host not available"
- Only happens on non-Windows platforms
- System audio capture is Windows-only for now

#### Waveform not updating
- Check browser console for errors
- Verify `waveform-update` event is being emitted
- Check that recording state is `Recording`

#### WAV files are silent
- Check microphone volume in system settings
- Verify the correct input device is being used

---

## Next Steps

With audio capture working, proceed to:

→ **[03-audio-preprocessing.md](./03-audio-preprocessing.md)** - Add VAD and denoising

---

## References

- [cpal Documentation](https://docs.rs/cpal/latest/cpal/)
- [cpal Examples Repository](https://github.com/RustAudio/cpal/tree/master/examples)
- [WASAPI Loopback Recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- [hound WAV Library](https://docs.rs/hound/latest/hound/)
- [rubato Resampling](https://docs.rs/rubato/latest/rubato/)
- [ringbuf Lock-free Ring Buffer](https://docs.rs/ringbuf/latest/ringbuf/)
- [Tauri Events](https://tauri.app/v2/guide/event/)
