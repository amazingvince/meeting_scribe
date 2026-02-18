//! macOS system audio capture backends.
//!
//! Preferred path: CoreAudio Process Tap (native system-output capture).
//! Fallback path: CoreAudio loopback input devices (BlackHole/Loopback/etc.).

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig};
use parking_lot::Mutex;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::audio::buffer::AudioBuffer;
use crate::audio::capture::BufferedResampler;
use crate::audio::{AudioChannel, WHISPER_SAMPLE_RATE};

/// Optional env override for selecting a specific loopback input device.
const SYSTEM_AUDIO_DEVICE_ENV: &str = "MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE";
/// Optional backend preference for macOS system-audio capture.
/// Supported values: `auto`, `process_tap`, `loopback`.
const SYSTEM_AUDIO_BACKEND_ENV: &str = "MEETING_SCRIBE_MACOS_SYSTEM_AUDIO_BACKEND";
const PROCESS_TAP_HELPER_SOURCE: &str = "src/audio/platform/macos_process_tap_helper.swift";
const PROCESS_TAP_HELPER_BINARY_NAME: &str = "meeting-scribe-process-tap-helper";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacBackendPreference {
    Auto,
    ProcessTap,
    Loopback,
}

struct LoopbackDeviceConfig {
    device: Device,
    device_name: String,
    config: StreamConfig,
    sample_format: SampleFormat,
    source_channels: u16,
    source_rate: u32,
}

struct ProcessTapRuntime {
    child: Child,
    reader_thread: Option<JoinHandle<()>>,
    stderr_thread: Option<JoinHandle<()>>,
}

pub struct SystemAudioCapture {
    stream: Option<Stream>,
    process_tap_runtime: Option<ProcessTapRuntime>,
    loopback_config: Option<LoopbackDeviceConfig>,
    loopback_unavailable_reason: Option<String>,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
    /// Shared resampler used only by loopback-input fallback path.
    resampler: Arc<Mutex<Option<BufferedResampler>>>,
}

impl SystemAudioCapture {
    pub fn new() -> Result<Self> {
        let (loopback_config, loopback_unavailable_reason) = match prepare_loopback_device() {
            Ok(config) => (Some(config), None),
            Err(e) => (None, Some(e.to_string())),
        };

        if let Some(reason) = loopback_unavailable_reason.as_deref() {
            info!(
                "macOS loopback-input fallback unavailable at init: {}",
                reason
            );
        }

        Ok(Self {
            stream: None,
            process_tap_runtime: None,
            loopback_config,
            loopback_unavailable_reason,
            buffer: AudioBuffer::new(AudioChannel::System),
            is_running: Arc::new(AtomicBool::new(false)),
            resampler: Arc::new(Mutex::new(None)),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            warn!("macOS system-audio capture already running");
            return Ok(());
        }

        let preference = backend_preference_from_env();
        let mut process_tap_error: Option<anyhow::Error> = None;

        if !matches!(preference, MacBackendPreference::Loopback) {
            match self.start_process_tap() {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("CoreAudio Process Tap backend unavailable: {}", e);
                    process_tap_error = Some(e);
                }
            }
        }

        if matches!(preference, MacBackendPreference::ProcessTap) {
            return Err(process_tap_error.unwrap_or_else(|| {
                anyhow!("CoreAudio Process Tap backend required by preference but unavailable")
            }));
        }

        self.start_loopback().with_context(|| {
            let loopback_hint = self
                .loopback_unavailable_reason
                .as_deref()
                .unwrap_or("loopback configuration unavailable");
            if let Some(err) = process_tap_error {
                format!(
                    "CoreAudio Process Tap failed ({err}); loopback fallback failed ({loopback_hint})"
                )
            } else {
                format!("Loopback fallback failed ({loopback_hint})")
            }
        })
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);

        if let Some(mut runtime) = self.process_tap_runtime.take() {
            terminate_child(&mut runtime.child);
            if let Some(handle) = runtime.reader_thread.take() {
                let _ = handle.join();
            }
            if let Some(handle) = runtime.stderr_thread.take() {
                let _ = handle.join();
            }
            info!("macOS system-audio capture stopped (CoreAudio Process Tap)");
            return;
        }

        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        if let Some(ref mut resampler) = *self.resampler.lock() {
            match resampler.flush() {
                Ok(flushed) => {
                    if !flushed.is_empty() {
                        debug!(
                            "Flushed {} samples from macOS loopback resampler",
                            flushed.len()
                        );
                        self.buffer.push_samples(&flushed);
                    }
                }
                Err(e) => warn!("Failed to flush macOS loopback resampler: {}", e),
            }
        }

        info!("macOS system-audio capture stopped (loopback fallback)");
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }

    fn start_process_tap(&mut self) -> Result<()> {
        let helper_binary = ensure_process_tap_helper_binary()?;

        let mut child = Command::new(&helper_binary)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to launch CoreAudio Process Tap helper at '{}'",
                    helper_binary.display()
                )
            })?;

        let stdout = child
            .stdout
            .take()
            .context("Failed to capture Process Tap helper stdout")?;
        let stderr = child
            .stderr
            .take()
            .context("Failed to capture Process Tap helper stderr")?;

        let buffer = self.buffer.clone();
        let is_running = Arc::clone(&self.is_running);
        let reader_thread =
            thread::spawn(move || read_process_tap_pcm_stream(stdout, buffer, is_running));
        let helper_errors = Arc::new(Mutex::new(Vec::<String>::new()));
        let helper_errors_for_thread = Arc::clone(&helper_errors);
        let stderr_thread =
            thread::spawn(move || log_process_tap_stderr(stderr, helper_errors_for_thread));

        // Give the helper a moment to report immediate setup failures.
        thread::sleep(Duration::from_millis(400));
        if let Some(status) = child
            .try_wait()
            .context("Failed to poll Process Tap helper")?
        {
            let _ = reader_thread.join();
            let _ = stderr_thread.join();
            let detail = helper_errors.lock().last().cloned().unwrap_or_default();
            anyhow::bail!(
                "CoreAudio Process Tap helper exited early with status {}{}",
                status,
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(" ({detail})")
                }
            );
        }

        self.process_tap_runtime = Some(ProcessTapRuntime {
            child,
            reader_thread: Some(reader_thread),
            stderr_thread: Some(stderr_thread),
        });
        self.is_running.store(true, Ordering::SeqCst);
        info!("macOS system-audio capture started (CoreAudio Process Tap)");
        Ok(())
    }

    fn start_loopback(&mut self) -> Result<()> {
        let config = self.loopback_config.as_ref().with_context(|| {
            self.loopback_unavailable_reason
                .clone()
                .unwrap_or_else(|| "loopback device configuration missing".to_string())
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
        let err_fn = |err| error!("macOS loopback stream error: {}", err);

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
                "Failed to start loopback input stream on '{}'",
                config.device_name
            )
        })?;

        self.stream = Some(stream);
        self.is_running.store(true, Ordering::SeqCst);
        info!(
            "macOS system-audio capture started (loopback input '{}')",
            config.device_name
        );
        Ok(())
    }
}

impl Drop for SystemAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn read_process_tap_pcm_stream(
    mut stdout: impl Read,
    buffer: AudioBuffer,
    is_running: Arc<AtomicBool>,
) {
    let mut pending = Vec::<u8>::new();
    let mut chunk = [0u8; 16 * 1024];

    loop {
        match stdout.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                pending.extend_from_slice(&chunk[..n]);
                let usable = pending.len() - (pending.len() % 4);
                if usable == 0 {
                    continue;
                }

                let mut samples = Vec::<f32>::with_capacity(usable / 4);
                for bytes in pending[..usable].chunks_exact(4) {
                    samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                }

                if !samples.is_empty() {
                    buffer.push_samples(&samples);
                }

                pending.drain(..usable);
            }
            Err(e) => {
                error!("CoreAudio Process Tap stdout read error: {}", e);
                break;
            }
        }
    }

    if is_running.swap(false, Ordering::SeqCst) {
        warn!("CoreAudio Process Tap helper stream ended");
    } else {
        debug!("CoreAudio Process Tap helper stream closed");
    }
}

fn log_process_tap_stderr(stderr: impl Read, helper_errors: Arc<Mutex<Vec<String>>>) {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
        match line {
            Ok(message) => {
                let trimmed = message.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("ERROR:") {
                    let mut errors = helper_errors.lock();
                    errors.push(trimmed.to_string());
                    if errors.len() > 8 {
                        errors.remove(0);
                    }
                }
                if trimmed.starts_with("ERROR:") {
                    error!("CoreAudio Process Tap helper: {}", trimmed);
                } else if trimmed.starts_with("READY") {
                    info!("CoreAudio Process Tap helper ready");
                } else if trimmed.starts_with("SOURCE:") {
                    info!("CoreAudio Process Tap helper {}", trimmed);
                } else {
                    debug!("CoreAudio Process Tap helper: {}", trimmed);
                }
            }
            Err(e) => {
                debug!("CoreAudio Process Tap helper stderr closed: {}", e);
                break;
            }
        }
    }
}

fn terminate_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {}
        Err(e) => {
            warn!("Failed to poll Process Tap helper status: {}", e);
        }
    }

    if let Err(e) = child.kill() {
        warn!("Failed to terminate Process Tap helper: {}", e);
    }
    if let Err(e) = child.wait() {
        warn!("Failed to wait for Process Tap helper exit: {}", e);
    }
}

fn ensure_process_tap_helper_binary() -> Result<PathBuf> {
    let source = process_tap_helper_source_path();
    if !source.exists() {
        anyhow::bail!(
            "Process Tap helper source not found at '{}'",
            source.display()
        );
    }

    let binary = process_tap_helper_binary_path()?;
    let needs_rebuild = needs_process_tap_helper_rebuild(&source, &binary)
        .context("Failed to check Process Tap helper build status")?;

    if needs_rebuild {
        compile_process_tap_helper(&source, &binary)?;
    }

    Ok(binary)
}

fn process_tap_helper_source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PROCESS_TAP_HELPER_SOURCE)
}

fn process_tap_helper_binary_path() -> Result<PathBuf> {
    let mut dir = std::env::temp_dir();
    dir.push("meeting-scribe");
    dir.push("helpers");
    fs::create_dir_all(&dir).with_context(|| {
        format!(
            "Failed to create Process Tap helper directory '{}'",
            dir.display()
        )
    })?;
    dir.push(PROCESS_TAP_HELPER_BINARY_NAME);
    Ok(dir)
}

fn needs_process_tap_helper_rebuild(source: &Path, binary: &Path) -> Result<bool> {
    if !binary.exists() {
        return Ok(true);
    }

    let source_meta = fs::metadata(source).with_context(|| {
        format!(
            "Failed to read metadata for helper source '{}'",
            source.display()
        )
    })?;
    let binary_meta = fs::metadata(binary).with_context(|| {
        format!(
            "Failed to read metadata for helper binary '{}'",
            binary.display()
        )
    })?;

    let source_modified = source_meta
        .modified()
        .context("Failed to read helper source modification time")?;
    let binary_modified = binary_meta
        .modified()
        .context("Failed to read helper binary modification time")?;

    Ok(source_modified > binary_modified)
}

fn compile_process_tap_helper(source: &Path, binary: &Path) -> Result<()> {
    info!(
        "Compiling CoreAudio Process Tap helper: '{}' -> '{}'",
        source.display(),
        binary.display()
    );

    let output = Command::new("xcrun")
        .args(["swiftc", "-O", "-parse-as-library"])
        .arg(source)
        .arg("-o")
        .arg(binary)
        .output()
        .context(
            "Failed to run 'xcrun swiftc'. Install Xcode Command Line Tools to enable macOS Process Tap capture.",
        )?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "Failed to compile Process Tap helper (status={}): {} {}",
            output.status,
            stderr.trim(),
            stdout.trim()
        );
    }

    Ok(())
}

fn prepare_loopback_device() -> Result<LoopbackDeviceConfig> {
    let host = cpal::default_host();
    let preferred = env::var(SYSTEM_AUDIO_DEVICE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut devices = host
        .input_devices()
        .context("Failed to enumerate macOS input devices")?
        .map(|device| {
            let name = device
                .name()
                .unwrap_or_else(|_| "Unknown input device".to_string());
            (device, name)
        })
        .collect::<Vec<_>>();

    if devices.is_empty() {
        anyhow::bail!("No macOS input devices found for loopback fallback");
    }

    let device_names = devices
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>();
    let selection = select_loopback_device_index(&device_names, preferred.as_deref())?;
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
        "Prepared macOS loopback fallback device: '{}' ({} Hz, {} channel(s), {:?})",
        device_name, source_rate, source_channels, sample_format
    );

    Ok(LoopbackDeviceConfig {
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

            let final_samples = if let Some(ref mut resampler) = *resampler.lock() {
                match resampler.process(&mono_samples) {
                    Ok(resampled) => resampled,
                    Err(e) => {
                        error!("macOS loopback resampling error: {}", e);
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

fn backend_preference_from_env() -> MacBackendPreference {
    let raw = env::var(SYSTEM_AUDIO_BACKEND_ENV).ok();
    raw.as_deref()
        .and_then(parse_backend_preference)
        .unwrap_or(MacBackendPreference::Auto)
}

fn parse_backend_preference(value: &str) -> Option<MacBackendPreference> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Some(MacBackendPreference::Auto),
        "process_tap" | "process-tap" | "tap" => Some(MacBackendPreference::ProcessTap),
        "loopback" | "loopback_input" | "loopback-input" => Some(MacBackendPreference::Loopback),
        _ => None,
    }
}

fn select_loopback_device_index(
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
        let score = loopback_device_score(name);
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
        "No loopback input device detected for macOS fallback capture. \
Install BlackHole/Loopback/Soundflower/Background Music, route output into that input, \
or set '{}' to a specific input device name. Available input devices: {}",
        SYSTEM_AUDIO_DEVICE_ENV,
        format_device_list(device_names)
    )
}

fn loopback_device_score(name: &str) -> usize {
    let name = name.to_ascii_lowercase();
    let mut score = 0usize;

    if name.contains("blackhole") {
        score = score.max(100);
    }
    if name.contains("loopback") {
        score = score.max(95);
    }
    if name.contains("soundflower") {
        score = score.max(90);
    }
    if name.contains("background music") {
        score = score.max(85);
    }
    if name.contains("vb-cable") || name.contains("vb cable") {
        score = score.max(80);
    }
    if name.contains("system audio") {
        score = score.max(70);
    }
    if name.contains("microphone") || name.contains("mic") {
        score = score.saturating_sub(50);
    }

    score
}

fn format_device_list(device_names: &[&str]) -> String {
    if device_names.is_empty() {
        return "none".to_string();
    }
    device_names.join(", ")
}

#[cfg(test)]
mod tests {
    use super::{
        loopback_device_score, parse_backend_preference, select_loopback_device_index,
        MacBackendPreference,
    };

    #[test]
    fn loopback_score_prefers_known_drivers() {
        assert!(
            loopback_device_score("BlackHole 2ch")
                > loopback_device_score("MacBook Pro Microphone")
        );
        assert!(
            loopback_device_score("Loopback Audio") > loopback_device_score("Built-in Microphone")
        );
        assert_eq!(loopback_device_score("Built-in Microphone"), 0);
    }

    #[test]
    fn selection_uses_best_keyword_match() {
        let devices = vec![
            "MacBook Pro Microphone",
            "Background Music",
            "BlackHole 2ch",
        ];
        let selected =
            select_loopback_device_index(&devices, None).expect("should select loopback");
        assert_eq!(selected, 2);
    }

    #[test]
    fn selection_uses_env_preference_exact_or_substring() {
        let devices = vec!["MacBook Pro Microphone", "BlackHole 2ch", "Loopback Audio"];

        let selected_exact = select_loopback_device_index(&devices, Some("Loopback Audio"))
            .expect("should select exact preference");
        assert_eq!(selected_exact, 2);

        let selected_substring = select_loopback_device_index(&devices, Some("blackhole"))
            .expect("should select substring preference");
        assert_eq!(selected_substring, 1);
    }

    #[test]
    fn selection_errors_when_no_loopback_found() {
        let devices = vec!["MacBook Pro Microphone", "AirPods Microphone"];
        let err =
            select_loopback_device_index(&devices, None).expect_err("should fail without loopback");
        let msg = err.to_string();
        assert!(msg.contains("No loopback input device"));
    }

    #[test]
    fn parse_backend_preference_variants() {
        assert_eq!(
            parse_backend_preference("process_tap"),
            Some(MacBackendPreference::ProcessTap)
        );
        assert_eq!(
            parse_backend_preference("process-tap"),
            Some(MacBackendPreference::ProcessTap)
        );
        assert_eq!(
            parse_backend_preference("loopback"),
            Some(MacBackendPreference::Loopback)
        );
        assert_eq!(
            parse_backend_preference("auto"),
            Some(MacBackendPreference::Auto)
        );
        assert_eq!(parse_backend_preference("invalid"), None);
    }
}
