//! Audio capture using cpal

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig};
use hound::{WavSpec, WavWriter};
use parking_lot::Mutex;
use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::buffer::AudioBuffer;
use super::{AudioChannel, BITS_PER_SAMPLE, CHANNELS, WHISPER_SAMPLE_RATE};

/// Optional input-device override for microphone capture.
const MICROPHONE_DEVICE_ENV: &str = "MEETING_SCRIBE_MIC_DEVICE";
/// Backward-compatible alias for microphone device override.
const MICROPHONE_DEVICE_ENV_LEGACY: &str = "MEETING_SCRIBE_MICROPHONE_DEVICE";

/// Audio capture manager for a single input device
pub struct AudioCapture {
    device: Device,
    config: StreamConfig,
    source_channels: u16,
    source_rate: u32,
    stream: Option<Stream>,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
    /// Shared resampler for flushing on stop
    resampler: Arc<Mutex<Option<BufferedResampler>>>,
}

impl AudioCapture {
    /// Create capture for microphone input.
    pub fn new_microphone() -> Result<Self> {
        let host = cpal::default_host();
        let (device, device_name, selection_reason) = resolve_microphone_device(&host)?;
        info!(
            "Using input device for microphone capture: {} ({})",
            device_name, selection_reason
        );

        // Get supported config from the device
        let supported_config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let source_rate = supported_config.sample_rate().0;
        let source_channels = supported_config.channels();

        debug!(
            "Device config: {} Hz, {} channels, {:?}",
            source_rate,
            source_channels,
            supported_config.sample_format()
        );

        // Use the device's native config for capture
        let config = StreamConfig {
            channels: source_channels,
            sample_rate: SampleRate(source_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        info!(
            "Microphone capture configured: {} Hz {} channel(s) -> {} Hz mono",
            source_rate, source_channels, WHISPER_SAMPLE_RATE
        );

        Ok(Self {
            device,
            config,
            source_channels,
            source_rate,
            stream: None,
            buffer: AudioBuffer::new(AudioChannel::Mic),
            is_running: Arc::new(AtomicBool::new(false)),
            resampler: Arc::new(Mutex::new(None)),
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

        // Create buffered resampler if device rate differs from target
        let needs_resampling = self.source_rate != WHISPER_SAMPLE_RATE;
        {
            let mut resampler_guard = self.resampler.lock();
            *resampler_guard = if needs_resampling {
                Some(BufferedResampler::new(
                    self.source_rate,
                    WHISPER_SAMPLE_RATE,
                )?)
            } else {
                None
            };
        }
        let resampler = Arc::clone(&self.resampler);

        let source_channels = self.source_channels as usize;

        let err_fn = |err| error!("Audio stream error: {}", err);

        let stream = match sample_format {
            SampleFormat::F32 => {
                self.build_stream::<f32>(buffer, resampler, source_channels, err_fn)?
            }
            SampleFormat::I16 => {
                self.build_stream::<i16>(buffer, resampler, source_channels, err_fn)?
            }
            SampleFormat::U16 => {
                self.build_stream::<u16>(buffer, resampler, source_channels, err_fn)?
            }
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
        resampler: Arc<Mutex<Option<BufferedResampler>>>,
        source_channels: usize,
        err_fn: impl Fn(cpal::StreamError) + Send + 'static,
    ) -> Result<Stream>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: cpal::FromSample<T>,
    {
        let config = self.config.clone();

        let stream = self.device.build_input_stream(
            &config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Convert to f32
                let samples: Vec<f32> =
                    data.iter().map(|&s| cpal::Sample::from_sample(s)).collect();

                // Convert to mono if stereo/multi-channel
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
                        debug!("Flushed {} samples from resampler", flushed.len());
                        self.buffer.push_samples(&flushed);
                    }
                }
                Err(e) => {
                    warn!("Failed to flush resampler: {}", e);
                }
            }
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

fn resolve_microphone_device(host: &cpal::Host) -> Result<(Device, String, &'static str)> {
    let preferred = resolve_preferred_microphone_name();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());

    let mut devices = host
        .input_devices()
        .context("Failed to enumerate input devices")?
        .map(|device| {
            let name = device
                .name()
                .unwrap_or_else(|_| "Unknown input device".to_string());
            (device, name)
        })
        .collect::<Vec<_>>();

    if devices.is_empty() {
        anyhow::bail!("No input device available");
    }

    let names = devices
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>();
    if let Some(default_name) = default_name.as_deref() {
        info!("Default input device reported by OS: {}", default_name);
    } else {
        warn!("OS did not report a default input device; selecting from available inputs");
    }

    let device_debug = names
        .iter()
        .map(|name| {
            format!(
                "{} [score={}, loopback={}, output_like={}]",
                name,
                microphone_device_score(name),
                is_probable_loopback_device(name),
                is_probable_output_like_device(name)
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    info!("Available input device candidates: {}", device_debug);

    let (selected_index, reason) =
        select_microphone_device_index(&names, default_name.as_deref(), preferred.as_deref())?;

    let (device, device_name) = devices.swap_remove(selected_index);
    Ok((device, device_name, reason))
}

fn resolve_preferred_microphone_name() -> Option<String> {
    std::env::var(MICROPHONE_DEVICE_ENV)
        .ok()
        .or_else(|| std::env::var(MICROPHONE_DEVICE_ENV_LEGACY).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn select_microphone_device_index(
    device_names: &[&str],
    default_device_name: Option<&str>,
    preferred_device_name: Option<&str>,
) -> Result<(usize, &'static str)> {
    if let Some(preferred) = preferred_device_name {
        let preferred_lower = preferred.to_ascii_lowercase();

        if let Some((idx, _)) = device_names
            .iter()
            .enumerate()
            .find(|(_, name)| name.to_ascii_lowercase() == preferred_lower)
        {
            let candidate = device_names[idx];
            if !is_probable_loopback_device(candidate) && !is_probable_output_like_device(candidate)
            {
                return Ok((idx, "env override"));
            }
            warn!(
                "Ignoring '{}'='{}' because '{}' looks like a loopback/output source",
                MICROPHONE_DEVICE_ENV, preferred, candidate
            );
        }

        if let Some((idx, _)) = device_names
            .iter()
            .enumerate()
            .find(|(_, name)| name.to_ascii_lowercase().contains(&preferred_lower))
        {
            let candidate = device_names[idx];
            if !is_probable_loopback_device(candidate) && !is_probable_output_like_device(candidate)
            {
                return Ok((idx, "env override"));
            }
            warn!(
                "Ignoring '{}'='{}' because '{}' looks like a loopback/output source",
                MICROPHONE_DEVICE_ENV, preferred, candidate
            );
        }

        info!(
            "Configured '{}'='{}' did not resolve to a usable microphone; falling back to automatic microphone selection",
            MICROPHONE_DEVICE_ENV, preferred
        );
    }

    let default_index = default_device_name.and_then(|default| {
        let default_lower = default.to_ascii_lowercase();
        device_names
            .iter()
            .enumerate()
            .find(|(_, name)| name.to_ascii_lowercase() == default_lower)
            .map(|(idx, _)| idx)
    });

    if let Some(default_index) = default_index {
        let default_name = device_names[default_index];
        if !is_probable_loopback_device(default_name)
            && !is_probable_output_like_device(default_name)
        {
            return Ok((default_index, "default input"));
        }
    }

    let best_non_loopback = device_names
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            !is_probable_loopback_device(name) && !is_probable_output_like_device(name)
        })
        .map(|(idx, name)| (idx, microphone_device_score(name)))
        .max_by_key(|(_, score)| *score);

    if let Some((idx, _)) = best_non_loopback {
        return if default_index.is_some() {
            Ok((
                idx,
                "default input not suitable for microphone; selected microphone candidate",
            ))
        } else {
            Ok((idx, "microphone candidate match"))
        };
    }

    if let Some(default_index) = default_index {
        return Ok((
            default_index,
            "default input (no better microphone candidate)",
        ));
    }

    let first_non_loopback = device_names
        .iter()
        .enumerate()
        .find(|(_, name)| {
            !is_probable_loopback_device(name) && !is_probable_output_like_device(name)
        })
        .map(|(idx, _)| idx);

    if let Some(idx) = first_non_loopback {
        return Ok((idx, "first non-loopback input device"));
    }

    Ok((
        0,
        "first available input device (no clear microphone candidate)",
    ))
}

fn is_probable_loopback_device(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("blackhole")
        || normalized.contains("loopback")
        || normalized.contains("soundflower")
        || normalized.contains("vb-cable")
        || normalized.contains("vb cable")
        || normalized.contains("background music")
        || normalized.contains("monitor")
        || normalized.contains("process tap")
        || normalized.contains("systemtap")
        || normalized.contains("system audio")
        || normalized.contains("virtual audio")
        || normalized.contains("aggregate")
}

fn is_probable_output_like_device(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    let has_output_words = normalized.contains("speaker")
        || normalized.contains("output")
        || normalized.contains("playback");
    let has_mic_words = normalized.contains("microphone")
        || normalized.contains("mic")
        || normalized.contains("headset")
        || normalized.contains("airpods");
    has_output_words && !has_mic_words
}

fn microphone_device_score(name: &str) -> i32 {
    let normalized = name.to_ascii_lowercase();
    if is_probable_loopback_device(&normalized) {
        return -100;
    }
    if is_probable_output_like_device(&normalized) {
        return -50;
    }

    let mut score = 0i32;
    if normalized.contains("microphone") || normalized.contains("mic") {
        score += 90;
    }
    if normalized.contains("built-in")
        || normalized.contains("builtin")
        || normalized.contains("internal")
    {
        score += 35;
    }
    if normalized.contains("headset")
        || normalized.contains("headphone")
        || normalized.contains("airpods")
    {
        score += 30;
    }
    if normalized.contains("usb") || normalized.contains("external") {
        score += 20;
    }

    score
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
                let clamped = sample.clamp(-1.0, 1.0);
                let sample_i16 = (clamped * i16::MAX as f32) as i16;
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

        Ok(self.path.clone())
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

/// Load samples from a WAV file
///
/// Supports both Int16 and Float32 WAV formats.
/// Returns samples normalized to f32 range [-1.0, 1.0]
pub fn load_wav(path: &std::path::Path) -> Result<Vec<f32>> {
    use hound::WavReader;

    let reader = WavReader::open(path).context("Failed to open WAV file")?;
    let spec = reader.spec();

    debug!(
        "Loading WAV: {:?} ({} Hz, {} channels, {:?})",
        path, spec.sample_rate, spec.channels, spec.sample_format
    );

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = match spec.bits_per_sample {
                8 => i8::MAX as f32,
                16 => i16::MAX as f32,
                24 => (1 << 23) as f32,
                32 => i32::MAX as f32,
                _ => i16::MAX as f32,
            };

            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    // Convert stereo to mono if needed
    let mono_samples = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    info!(
        "Loaded {} samples from {:?} ({:.1}s)",
        mono_samples.len(),
        path,
        mono_samples.len() as f64 / spec.sample_rate as f64
    );

    Ok(mono_samples)
}

/// Save mono f32 samples to a WAV file at whisper sample rate.
///
/// Samples are expected in range [-1.0, 1.0] and are clamped when written.
pub fn save_wav(path: &std::path::Path, samples: &[f32]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory for {}", path.display()))?;
    }

    let mut recorder = AudioRecorder::new(path.to_path_buf())?;
    recorder.write_samples(samples)?;
    recorder.finalize()?;
    Ok(())
}

/// Chunk size for the resampler - balance between latency and efficiency
const RESAMPLER_CHUNK_SIZE: usize = 1024;

/// Buffered resampler that accumulates samples before processing
/// This is needed because SincFixedIn requires fixed-size input chunks
pub struct BufferedResampler {
    resampler: SincFixedIn<f32>,
    input_buffer: Vec<f32>,
    chunk_size: usize,
}

impl BufferedResampler {
    /// Create a new buffered resampler
    pub fn new(from_rate: u32, to_rate: u32) -> Result<Self> {
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler = SincFixedIn::new(
            to_rate as f64 / from_rate as f64,
            2.0,
            params,
            RESAMPLER_CHUNK_SIZE,
            1, // mono
        )
        .context("Failed to create resampler")?;

        Ok(Self {
            resampler,
            input_buffer: Vec::with_capacity(RESAMPLER_CHUNK_SIZE * 2),
            chunk_size: RESAMPLER_CHUNK_SIZE,
        })
    }

    /// Process input samples, buffering as needed
    /// Returns resampled output (may be empty if not enough input accumulated)
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        use rubato::Resampler;

        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Add new samples to buffer
        self.input_buffer.extend_from_slice(input);

        let mut output = Vec::new();

        // Process all complete chunks
        while self.input_buffer.len() >= self.chunk_size {
            let chunk: Vec<f32> = self.input_buffer.drain(..self.chunk_size).collect();
            let input_frames = vec![chunk];

            let resampled = self
                .resampler
                .process(&input_frames, None)
                .context("Resampling failed")?;

            if let Some(channel) = resampled.into_iter().next() {
                output.extend(channel);
            }
        }

        Ok(output)
    }

    /// Flush any remaining samples in the buffer (call at end of stream)
    pub fn flush(&mut self) -> Result<Vec<f32>> {
        use rubato::Resampler;

        if self.input_buffer.is_empty() {
            return Ok(Vec::new());
        }

        // Pad remaining samples to chunk size
        let remaining = self.input_buffer.len();
        self.input_buffer.resize(self.chunk_size, 0.0);

        let input_frames = vec![std::mem::take(&mut self.input_buffer)];

        let resampled = self
            .resampler
            .process(&input_frames, None)
            .context("Resampling flush failed")?;

        // Calculate how many output samples correspond to the actual input
        let ratio = self.resampler.output_frames_max();
        let actual_output_len = (remaining * ratio) / self.chunk_size;

        if let Some(channel) = resampled.into_iter().next() {
            Ok(channel.into_iter().take(actual_output_len).collect())
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_list_devices() {
        // Should not panic even if no devices
        let input = list_input_devices();
        let output = list_output_devices();

        println!("Input devices: {:?}", input);
        println!("Output devices: {:?}", output);
    }

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

    #[test]
    fn test_save_and_load_wav_roundtrip() {
        let temp = TempDir::new().unwrap();
        let wav_path = temp.path().join("roundtrip.wav");

        let input = vec![0.0_f32, 0.25, -0.25, 0.8, -0.8];
        save_wav(&wav_path, &input).unwrap();

        let output = load_wav(&wav_path).unwrap();
        assert_eq!(output.len(), input.len());
        // Int16 quantization introduces tiny error; keep tolerance small.
        for (in_sample, out_sample) in input.iter().zip(output.iter()) {
            assert!((in_sample - out_sample).abs() < 1e-3);
        }
    }

    #[test]
    fn microphone_selection_prefers_real_mic_over_loopback_default() {
        let devices = vec!["BlackHole 2ch", "MacBook Pro Microphone", "Loopback Audio"];
        let (idx, reason) =
            select_microphone_device_index(&devices, Some("BlackHole 2ch"), None).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(
            reason,
            "default input not suitable for microphone; selected microphone candidate"
        );
    }

    #[test]
    fn microphone_selection_uses_env_override() {
        let devices = vec!["MacBook Pro Microphone", "USB PnP Audio Device"];
        let (idx, reason) = select_microphone_device_index(
            &devices,
            Some("MacBook Pro Microphone"),
            Some("usb pnp"),
        )
        .unwrap();
        assert_eq!(idx, 1);
        assert_eq!(reason, "env override");
    }

    #[test]
    fn microphone_selection_ignores_loopback_env_override() {
        let devices = vec!["Built-in Microphone", "BlackHole 2ch"];
        let (idx, reason) = select_microphone_device_index(
            &devices,
            Some("Built-in Microphone"),
            Some("blackhole"),
        )
        .unwrap();
        assert_eq!(idx, 0);
        assert_eq!(reason, "default input");
    }

    #[test]
    fn microphone_selection_prefers_mic_when_default_is_non_mic_device() {
        let devices = vec![
            "MacBook Pro Speakers",
            "Built-in Microphone",
            "BlackHole 2ch",
        ];
        let (idx, reason) =
            select_microphone_device_index(&devices, Some("MacBook Pro Speakers"), None).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(
            reason,
            "default input not suitable for microphone; selected microphone candidate"
        );
    }

    #[test]
    fn microphone_scoring_penalizes_virtual_loopback_devices() {
        assert!(microphone_device_score("Built-in Microphone") > 0);
        assert!(microphone_device_score("BlackHole 2ch") < 0);
        assert!(microphone_device_score("Loopback Audio") < 0);
        assert!(microphone_device_score("MacBook Pro Speakers") < 0);
    }

    #[test]
    fn microphone_selection_prefers_non_loopback_even_without_mic_keyword() {
        let devices = vec!["BlackHole 2ch", "Scarlett 2i2 USB", "Loopback Audio"];
        let (idx, reason) =
            select_microphone_device_index(&devices, Some("BlackHole 2ch"), None).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(
            reason,
            "default input not suitable for microphone; selected microphone candidate"
        );
    }
}
