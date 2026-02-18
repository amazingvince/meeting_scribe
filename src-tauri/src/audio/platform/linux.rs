//! Linux system-audio capture via PipeWire/Pulse monitor inputs.
//!
//! This backend captures from monitor-style input devices exposed by the
//! active Linux audio stack (typically PipeWire or PulseAudio).

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig};
use parking_lot::Mutex;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::audio::buffer::AudioBuffer;
use crate::audio::capture::BufferedResampler;
use crate::audio::{AudioChannel, WHISPER_SAMPLE_RATE};

/// Optional env override for selecting a specific monitor input device.
const SYSTEM_AUDIO_DEVICE_ENV: &str = "MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE";
/// Gain multiplier for system audio (monitor feeds are often quieter than mic).
const SYSTEM_AUDIO_GAIN: f32 = 3.0;
/// Preserve headroom so gain staging does not clip the AEC reference.
const SYSTEM_AUDIO_TARGET_PEAK: f32 = 0.95;

struct MonitorDeviceConfig {
    device: Device,
    device_name: String,
    config: StreamConfig,
    sample_format: SampleFormat,
    source_channels: u16,
    source_rate: u32,
}

pub struct SystemAudioCapture {
    stream: Option<Stream>,
    monitor_config: Option<MonitorDeviceConfig>,
    monitor_unavailable_reason: Option<String>,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
    /// Shared resampler for flushing on stop.
    resampler: Arc<Mutex<Option<BufferedResampler>>>,
}

impl SystemAudioCapture {
    pub fn new() -> Result<Self> {
        let (monitor_config, monitor_unavailable_reason) = match prepare_monitor_device() {
            Ok(config) => (Some(config), None),
            Err(e) => (None, Some(e.to_string())),
        };

        if let Some(reason) = monitor_unavailable_reason.as_deref() {
            info!(
                "Linux monitor-input capture unavailable at init: {}",
                reason
            );
        }

        Ok(Self {
            stream: None,
            monitor_config,
            monitor_unavailable_reason,
            buffer: AudioBuffer::new(AudioChannel::System),
            is_running: Arc::new(AtomicBool::new(false)),
            resampler: Arc::new(Mutex::new(None)),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("Linux system-audio capture already running");
            return Ok(());
        }

        let config = self.monitor_config.as_ref().with_context(|| {
            self.monitor_unavailable_reason.clone().unwrap_or_else(|| {
                "Linux monitor input device configuration unavailable".to_string()
            })
        })?;

        let needs_resampling = config.source_rate != WHISPER_SAMPLE_RATE;
        {
            let mut resampler_guard = self.resampler.lock();
            *resampler_guard = if needs_resampling {
                Some(BufferedResampler::new(
                    config.source_rate,
                    WHISPER_SAMPLE_RATE,
                )?)
            } else {
                None
            };
        }
        let resampler = Arc::clone(&self.resampler);
        let source_channels = config.source_channels as usize;
        let err_fn = |err| error!("Linux system audio stream error: {}", err);

        let stream = match config.sample_format {
            SampleFormat::F32 => build_stream::<f32>(
                &config.device,
                config.config.clone(),
                self.buffer.clone(),
                resampler,
                source_channels,
                err_fn,
            )?,
            SampleFormat::I16 => build_stream::<i16>(
                &config.device,
                config.config.clone(),
                self.buffer.clone(),
                resampler,
                source_channels,
                err_fn,
            )?,
            SampleFormat::U16 => build_stream::<u16>(
                &config.device,
                config.config.clone(),
                self.buffer.clone(),
                resampler,
                source_channels,
                err_fn,
            )?,
            _ => anyhow::bail!("Unsupported sample format: {:?}", config.sample_format),
        };

        stream.play().with_context(|| {
            format!(
                "Failed to start Linux monitor input stream on '{}'",
                config.device_name
            )
        })?;

        self.stream = Some(stream);
        self.is_running.store(true, Ordering::SeqCst);
        info!(
            "Linux system-audio capture started (monitor input '{}')",
            config.device_name
        );
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);

        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        if let Some(ref mut resampler) = *self.resampler.lock() {
            match resampler.flush() {
                Ok(flushed) => {
                    if !flushed.is_empty() {
                        debug!(
                            "Flushed {} samples from Linux monitor resampler",
                            flushed.len()
                        );
                        self.buffer.push_samples(&flushed);
                    }
                }
                Err(e) => warn!("Failed to flush Linux monitor resampler: {}", e),
            }
        }

        info!("Linux system-audio capture stopped");
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn prepare_monitor_device() -> Result<MonitorDeviceConfig> {
    let host = cpal::default_host();
    let preferred = env::var(SYSTEM_AUDIO_DEVICE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut devices = host
        .input_devices()
        .context("Failed to enumerate Linux input devices")?
        .map(|device| {
            let name = device
                .name()
                .unwrap_or_else(|_| "Unknown input device".to_string());
            (device, name)
        })
        .collect::<Vec<_>>();

    if devices.is_empty() {
        anyhow::bail!("No Linux input devices found for system-audio capture");
    }

    let device_names = devices
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>();
    let selection = select_monitor_device_index(&device_names, preferred.as_deref())?;
    let (device, device_name) = devices.swap_remove(selection);

    let supported = device
        .default_input_config()
        .with_context(|| format!("Failed to read input config for device '{}'", device_name))?;

    let source_rate = supported.sample_rate().0;
    let source_channels = supported.channels();
    let sample_format = supported.sample_format();

    let config = StreamConfig {
        channels: source_channels,
        sample_rate: SampleRate(source_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    info!(
        "Prepared Linux monitor input device: '{}' ({} Hz, {} channel(s), {:?})",
        device_name, source_rate, source_channels, sample_format
    );

    Ok(MonitorDeviceConfig {
        device,
        device_name,
        config,
        sample_format,
        source_channels,
        source_rate,
    })
}

fn build_stream<T>(
    device: &Device,
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
    let stream = device.build_input_stream(
        &config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let samples: Vec<f32> = data.iter().map(|&s| cpal::Sample::from_sample(s)).collect();

            let mono_samples = if source_channels > 1 {
                samples
                    .chunks(source_channels)
                    .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
                    .collect::<Vec<f32>>()
            } else {
                samples
            };

            let amplified = apply_safe_gain(&mono_samples);

            let final_samples = if let Some(ref mut resampler) = *resampler.lock() {
                match resampler.process(&amplified) {
                    Ok(resampled) => resampled,
                    Err(e) => {
                        error!("Linux monitor resampling error: {}", e);
                        amplified
                    }
                }
            } else {
                amplified
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

fn select_monitor_device_index(
    device_names: &[&str],
    preferred_device: Option<&str>,
) -> Result<usize> {
    if let Some(preferred) = preferred_device {
        let preferred_lower = preferred.to_ascii_lowercase();

        if let Some((idx, _)) = device_names
            .iter()
            .enumerate()
            .find(|(_, name)| name.to_ascii_lowercase() == preferred_lower)
        {
            return Ok(idx);
        }

        if let Some((idx, _)) = device_names
            .iter()
            .enumerate()
            .find(|(_, name)| name.to_ascii_lowercase().contains(&preferred_lower))
        {
            return Ok(idx);
        }

        anyhow::bail!(
            "Configured '{}'='{}', but no input device matched. Available input devices: {}",
            SYSTEM_AUDIO_DEVICE_ENV,
            preferred,
            format_device_list(device_names)
        );
    }

    let mut best_match: Option<(usize, usize)> = None;
    for (idx, name) in device_names.iter().enumerate() {
        let score = monitor_device_score(name);
        if score == 0 {
            continue;
        }
        if best_match
            .map(|(_, best_score)| score > best_score)
            .unwrap_or(true)
        {
            best_match = Some((idx, score));
        }
    }

    if let Some((idx, _)) = best_match {
        return Ok(idx);
    }

    anyhow::bail!(
        "No monitor input device detected for Linux system-audio capture. \
Ensure PipeWire/PulseAudio exposes a monitor source (for example, 'Monitor of ...') \
or route output into a virtual loopback input, then retry. \
You can also set '{}' to a specific input device name. Available input devices: {}",
        SYSTEM_AUDIO_DEVICE_ENV,
        format_device_list(device_names)
    )
}

fn monitor_device_score(name: &str) -> usize {
    let name = name.to_ascii_lowercase();
    let mut score = 0usize;

    if name.contains("monitor of") {
        score = score.max(130);
    }
    if name.contains(".monitor") {
        score = score.max(120);
    }
    if name.contains("monitor") {
        score = score.max(110);
    }
    if name.contains("loopback") {
        score = score.max(95);
    }
    if name.contains("stereo mix") {
        score = score.max(90);
    }
    if name.contains("what u hear") {
        score = score.max(85);
    }
    if name.contains("virtual sink") || name.contains("virtual output") {
        score = score.max(80);
    }

    if name.contains("microphone") || name.contains("mic") {
        score = score.saturating_sub(70);
    }

    score
}

fn format_device_list(device_names: &[&str]) -> String {
    if device_names.is_empty() {
        return "none".to_string();
    }
    device_names.join(", ")
}

fn apply_safe_gain(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let peak = samples
        .iter()
        .fold(0.0f32, |max_abs, s| max_abs.max(s.abs()));
    let safe_gain = if peak > 0.0 {
        SYSTEM_AUDIO_GAIN.min(SYSTEM_AUDIO_TARGET_PEAK / peak)
    } else {
        SYSTEM_AUDIO_GAIN
    };

    samples
        .iter()
        .map(|&s| (s * safe_gain).clamp(-1.0, 1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{monitor_device_score, select_monitor_device_index};

    #[test]
    fn monitor_score_prefers_monitor_inputs() {
        assert!(
            monitor_device_score("Monitor of Built-in Audio Analog Stereo")
                > monitor_device_score("Built-in Microphone")
        );
        assert!(monitor_device_score("alsa_output.pci-0000_00_1f.3.analog-stereo.monitor") > 0);
        assert_eq!(monitor_device_score("Built-in Microphone"), 0);
    }

    #[test]
    fn selection_uses_best_keyword_match() {
        let devices = vec![
            "Built-in Microphone",
            "Monitor of Starship-Matisse HD Audio Controller Analog Stereo",
            "USB Mic",
        ];

        let selected =
            select_monitor_device_index(&devices, None).expect("should select monitor input");
        assert_eq!(selected, 1);
    }

    #[test]
    fn selection_uses_env_preference_exact_or_substring() {
        let devices = vec![
            "Built-in Microphone",
            "Monitor of HDMI Output",
            "Monitor of USB DAC",
        ];

        let selected_exact = select_monitor_device_index(&devices, Some("Monitor of USB DAC"))
            .expect("should select exact preference");
        assert_eq!(selected_exact, 2);

        let selected_substring = select_monitor_device_index(&devices, Some("hdmi"))
            .expect("should select substring preference");
        assert_eq!(selected_substring, 1);
    }

    #[test]
    fn selection_errors_when_no_monitor_found() {
        let devices = vec!["Built-in Microphone", "USB Mic"];
        let err =
            select_monitor_device_index(&devices, None).expect_err("should fail without monitor");
        let msg = err.to_string();
        assert!(msg.contains("No monitor input device detected"));
    }
}
