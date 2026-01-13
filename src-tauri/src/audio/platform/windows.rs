//! Windows system audio capture via WASAPI loopback

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream, StreamConfig};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::audio::buffer::AudioBuffer;
use crate::audio::capture::BufferedResampler;
use crate::audio::{AudioChannel, WHISPER_SAMPLE_RATE};

/// System audio capture using WASAPI loopback
pub struct SystemAudioCapture {
    stream: Option<Stream>,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
    /// Shared resampler for flushing on stop
    resampler: Arc<Mutex<Option<BufferedResampler>>>,
}

impl SystemAudioCapture {
    /// Create capture for default output device (loopback)
    pub fn new() -> Result<Self> {
        Ok(Self {
            stream: None,
            buffer: AudioBuffer::new(AudioChannel::System),
            is_running: Arc::new(AtomicBool::new(false)),
            resampler: Arc::new(Mutex::new(None)),
        })
    }

    /// Start capturing system audio
    pub fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("System capture already running");
            return Ok(());
        }

        // Use WASAPI host for loopback support
        let host = cpal::host_from_id(cpal::HostId::Wasapi)
            .context("WASAPI host not available")?;

        // Get default output device for loopback
        let device = host
            .default_output_device()
            .context("No output device available for loopback")?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using loopback device: {}", device_name);

        let supported = device
            .default_output_config()
            .context("Failed to get default output config")?;

        let sample_format = supported.sample_format();
        let source_rate = supported.sample_rate().0;
        let source_channels = supported.channels();

        debug!(
            "Loopback device: {} Hz, {} channels, {:?}",
            source_rate, source_channels, sample_format
        );

        // Build config for loopback capture
        let config = StreamConfig {
            channels: source_channels,
            sample_rate: SampleRate(source_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer = self.buffer.clone();
        let is_running = Arc::clone(&self.is_running);

        // Create buffered resampler if needed
        let needs_resampling = source_rate != WHISPER_SAMPLE_RATE;
        {
            let mut resampler_guard = self.resampler.lock();
            *resampler_guard = if needs_resampling {
                Some(BufferedResampler::new(source_rate, WHISPER_SAMPLE_RATE)?)
            } else {
                None
            };
        }

        let resampler = Arc::clone(&self.resampler);
        let source_channels_count = source_channels as usize;

        let err_fn = |err| error!("System audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, config, buffer, resampler, source_channels_count, err_fn)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, config, buffer, resampler, source_channels_count, err_fn)?
            }
            _ => anyhow::bail!("Unsupported sample format: {:?}", sample_format),
        };

        stream.play().context("Failed to start loopback stream")?;

        self.stream = Some(stream);
        is_running.store(true, Ordering::SeqCst);

        info!("System audio capture started");
        Ok(())
    }

    /// Stop capturing
    pub fn stop(&mut self) {
        // First drop the stream to stop new samples from arriving
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // Flush any remaining samples from the resampler
        if let Some(ref mut resampler) = *self.resampler.lock() {
            match resampler.flush() {
                Ok(flushed) => {
                    if !flushed.is_empty() {
                        debug!("Flushed {} samples from system resampler", flushed.len());
                        self.buffer.push_samples(&flushed);
                    }
                }
                Err(e) => {
                    warn!("Failed to flush system resampler: {}", e);
                }
            }
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

fn build_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    buffer: AudioBuffer,
    resampler: Arc<Mutex<Option<BufferedResampler>>>,
    source_channels: usize,
    err_fn: impl Fn(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    // Note: For WASAPI loopback, we use build_input_stream on an output device
    let stream = device.build_input_stream(
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
                match resampler.process(&mono_samples) {
                    Ok(resampled) => resampled,
                    Err(e) => {
                        error!("Resampling error: {}", e);
                        mono_samples
                    }
                }
            } else {
                mono_samples
            };

            if !final_samples.is_empty() {
                buffer.push_samples(&final_samples);
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}
