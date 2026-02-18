//! Acoustic Echo Cancellation using SpeexDSP
//!
//! Removes echo from microphone input using system audio as reference signal.

use aec3::voip::VoipAec3;
use aec_rs::{Aec, AecConfig};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, info, warn};

/// Frame size for AEC processing (10ms at 16kHz = 160 samples)
const AEC_FRAME_SIZE: usize = 160;

/// Filter length in samples (100ms tail at 16kHz = 1600 samples)
/// This accommodates typical acoustic delays (speaker → mic: 50-200ms)
const AEC_FILTER_LENGTH: i32 = 1600;

/// Sample rate for AEC processing
const AEC_SAMPLE_RATE: u32 = 16000;
/// AEC frame size in milliseconds for frame-based engines.
const AEC_FRAME_MS: usize = 10;

/// Alignment hop size (10ms at 16kHz)
const ALIGN_HOP_SAMPLES: usize = AEC_FRAME_SIZE;
/// Max lag to search during coarse alignment
const ALIGN_MAX_LAG_MS: u32 = 400;
/// Minimum overlap needed to score a lag candidate
const ALIGN_MIN_OVERLAP_MS: u32 = 250;
/// Analyze only the first N seconds to keep alignment fast for long recordings
const ALIGN_MAX_ANALYSIS_SECONDS: u32 = 90;
/// Below this correlation, keep shift at zero (alignment likely unreliable)
const ALIGN_MIN_CORRELATION: f32 = 0.05;
/// Minimum reference RMS needed before residual suppression is considered.
const RESIDUAL_MIN_REF_RMS: f32 = 0.008;
/// Minimum microphone RMS needed before residual suppression is considered.
const RESIDUAL_MIN_MIC_RMS: f32 = 0.01;
/// Correlation threshold above which residual suppression engages.
const RESIDUAL_CORRELATION_THRESHOLD: f32 = 0.55;
/// Maximum attenuation applied by residual suppression.
const RESIDUAL_MAX_ATTENUATION: f32 = 0.82;
/// Minimum gain floor to avoid over-muting mic audio.
const RESIDUAL_MIN_GAIN: f32 = 0.18;
/// Keep suppression active for a short duration after strong echo detection.
const RESIDUAL_HANGOVER_FRAMES: usize = 4;

/// Echo cancellation backend options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EchoCancellationBackend {
    #[default]
    #[serde(rename = "webrtc_aec3", alias = "webrtc", alias = "aec3")]
    WebRtcAec3,
    #[serde(rename = "speex")]
    Speex,
}

impl EchoCancellationBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WebRtcAec3 => "webrtc_aec3",
            Self::Speex => "speex",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "webrtc_aec3" | "webrtc" | "aec3" => Some(Self::WebRtcAec3),
            "speex" | "speexdsp" => Some(Self::Speex),
            _ => None,
        }
    }
}

/// Resolve requested backend, applying environment override when explicit request is absent.
pub fn resolve_echo_backend(requested: Option<EchoCancellationBackend>) -> EchoCancellationBackend {
    if let Some(backend) = requested {
        return backend;
    }

    let from_env = env::var("MEETING_SCRIBE_ECHO_BACKEND")
        .ok()
        .and_then(|value| EchoCancellationBackend::parse(&value));

    from_env.unwrap_or_default()
}

/// Result metadata for a single echo-cancellation batch.
#[derive(Debug, Clone, Copy)]
pub struct EchoProcessingInfo {
    pub requested_backend: EchoCancellationBackend,
    pub backend_used: EchoCancellationBackend,
    pub fallback_used: bool,
}

/// Alignment metadata for AEC reference preparation.
#[derive(Debug, Clone, Copy)]
pub struct ReferenceAlignment {
    pub shift_samples: isize,
    pub correlation: f32,
}

impl ReferenceAlignment {
    pub fn shift_ms(&self, sample_rate: u32) -> f32 {
        self.shift_samples as f32 * 1000.0 / sample_rate as f32
    }
}

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
            info!(
                "AEC skipped: disabled={}, reference_len={}",
                !self.enabled,
                reference.len()
            );
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
            self.aec.cancel_echo(
                &self.mic_frame_i16,
                &self.ref_frame_i16,
                &mut self.out_frame_i16,
            );

            // Convert output to f32 and append
            for &sample in &self.out_frame_i16 {
                output.push(sample as f32 / 32768.0);
            }

            processed_samples = end;

            // Log progress every ~30 seconds of audio
            if frame_idx > 0 && frame_idx % 3000 == 0 {
                let progress_secs =
                    frame_idx as f32 * AEC_FRAME_SIZE as f32 / AEC_SAMPLE_RATE as f32;
                info!(
                    "AEC progress: {:.1}s processed ({} frames)",
                    progress_secs, frame_idx
                );
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

/// Apply echo cancellation using a selected backend.
///
/// WebRTC AEC3 is the default backend. If it fails to initialize/process, the
/// function falls back to Speex where compatible.
pub fn process_echo_cancellation(
    mic: &[f32],
    reference: &[f32],
    sample_rate: u32,
    requested_backend: EchoCancellationBackend,
) -> (Vec<f32>, EchoProcessingInfo) {
    if reference.is_empty() || mic.is_empty() {
        return (
            mic.to_vec(),
            EchoProcessingInfo {
                requested_backend,
                backend_used: requested_backend,
                fallback_used: false,
            },
        );
    }

    let mut fallback_used = false;
    let output = match requested_backend {
        EchoCancellationBackend::WebRtcAec3 => {
            match process_webrtc_batch(mic, reference, sample_rate) {
                Ok(processed) => processed,
                Err(err) => {
                    fallback_used = true;
                    warn!("WebRTC AEC3 failed, falling back to Speex: {}", err);
                    match process_speex_batch(mic, reference, sample_rate) {
                        Ok(processed) => processed,
                        Err(speex_err) => {
                            warn!(
                                "Speex fallback also failed ({}); returning mic passthrough",
                                speex_err
                            );
                            mic.to_vec()
                        }
                    }
                }
            }
        }
        EchoCancellationBackend::Speex => match process_speex_batch(mic, reference, sample_rate) {
            Ok(processed) => processed,
            Err(err) => {
                fallback_used = true;
                warn!("Speex AEC failed, falling back to WebRTC AEC3: {}", err);
                match process_webrtc_batch(mic, reference, sample_rate) {
                    Ok(processed) => processed,
                    Err(webrtc_err) => {
                        warn!(
                            "WebRTC fallback also failed ({}); returning mic passthrough",
                            webrtc_err
                        );
                        mic.to_vec()
                    }
                }
            }
        },
    };

    let backend_used = if !fallback_used {
        requested_backend
    } else {
        match requested_backend {
            EchoCancellationBackend::WebRtcAec3 => EchoCancellationBackend::Speex,
            EchoCancellationBackend::Speex => EchoCancellationBackend::WebRtcAec3,
        }
    };

    (
        output,
        EchoProcessingInfo {
            requested_backend,
            backend_used,
            fallback_used,
        },
    )
}

enum RealtimeProcessor {
    WebRtc(Box<WebRtcEchoCanceller>),
    Speex(EchoCanceller),
}

/// Stateful real-time echo canceller for chunked microphone/system streams.
///
/// This keeps backend adaptive state across chunks, unlike repeatedly calling
/// one-shot batch helpers for each small frame.
pub struct RealtimeEchoCanceller {
    backend_used: EchoCancellationBackend,
    fallback_used: bool,
    processor: RealtimeProcessor,
    frame_samples: usize,
    mic_pending: Vec<f32>,
    ref_pending: Vec<f32>,
}

impl RealtimeEchoCanceller {
    pub fn new(
        sample_rate: u32,
        requested_backend: EchoCancellationBackend,
    ) -> Result<Self, String> {
        match requested_backend {
            EchoCancellationBackend::WebRtcAec3 => {
                match WebRtcEchoCanceller::new(sample_rate, 1, 1) {
                    Ok(webrtc) => {
                        let frame_samples = webrtc.frame_samples;
                        Ok(Self {
                            backend_used: EchoCancellationBackend::WebRtcAec3,
                            fallback_used: false,
                            processor: RealtimeProcessor::WebRtc(Box::new(webrtc)),
                            frame_samples,
                            mic_pending: Vec::new(),
                            ref_pending: Vec::new(),
                        })
                    }
                    Err(webrtc_err) => {
                        if sample_rate != AEC_SAMPLE_RATE {
                            return Err(format!(
                                "WebRTC AEC3 init failed ({}) and Speex fallback requires {} Hz (got {})",
                                webrtc_err, AEC_SAMPLE_RATE, sample_rate
                            ));
                        }
                        warn!(
                            "WebRTC AEC3 real-time init failed, falling back to Speex: {}",
                            webrtc_err
                        );
                        let speex = EchoCanceller::new();
                        Ok(Self {
                            backend_used: EchoCancellationBackend::Speex,
                            fallback_used: true,
                            processor: RealtimeProcessor::Speex(speex),
                            frame_samples: AEC_FRAME_SIZE,
                            mic_pending: Vec::new(),
                            ref_pending: Vec::new(),
                        })
                    }
                }
            }
            EchoCancellationBackend::Speex => {
                if sample_rate == AEC_SAMPLE_RATE {
                    let speex = EchoCanceller::new();
                    Ok(Self {
                        backend_used: EchoCancellationBackend::Speex,
                        fallback_used: false,
                        processor: RealtimeProcessor::Speex(speex),
                        frame_samples: AEC_FRAME_SIZE,
                        mic_pending: Vec::new(),
                        ref_pending: Vec::new(),
                    })
                } else {
                    warn!(
                        "Speex real-time requested at {} Hz; falling back to WebRTC AEC3",
                        sample_rate
                    );
                    let webrtc = WebRtcEchoCanceller::new(sample_rate, 1, 1)?;
                    let frame_samples = webrtc.frame_samples;
                    Ok(Self {
                        backend_used: EchoCancellationBackend::WebRtcAec3,
                        fallback_used: true,
                        processor: RealtimeProcessor::WebRtc(Box::new(webrtc)),
                        frame_samples,
                        mic_pending: Vec::new(),
                        ref_pending: Vec::new(),
                    })
                }
            }
        }
    }

    pub fn backend_used(&self) -> EchoCancellationBackend {
        self.backend_used
    }

    pub fn fallback_used(&self) -> bool {
        self.fallback_used
    }

    /// Process incremental chunks and return available cleaned mic samples.
    ///
    /// If no reference is available for a chunk, microphone audio is passed
    /// through unchanged to avoid build-up/latency.
    pub fn process_chunk(&mut self, mic: &[f32], reference: &[f32]) -> Vec<f32> {
        if !mic.is_empty() {
            self.mic_pending.extend_from_slice(mic);
        }
        if !reference.is_empty() {
            self.ref_pending.extend_from_slice(reference);
        }

        let mut output = self.process_ready_frames();

        // No reference this interval: avoid extra latency by passthrough.
        if reference.is_empty() && self.ref_pending.is_empty() && !self.mic_pending.is_empty() {
            output.append(&mut self.mic_pending);
            return output;
        }

        // Bound latency if mic temporarily outruns reference.
        let max_pending = self.frame_samples * 8; // ~80ms at 16kHz/10ms frames
        if self.mic_pending.len() > max_pending && self.ref_pending.len() < self.frame_samples {
            let passthrough = self.mic_pending.len() - max_pending;
            output.extend(self.mic_pending.drain(..passthrough));
        }

        output
    }

    /// Flush any buffered samples at end of stream.
    pub fn flush(&mut self) -> Vec<f32> {
        let mut output = self.process_ready_frames();
        if !self.mic_pending.is_empty() {
            output.append(&mut self.mic_pending);
        }
        self.ref_pending.clear();
        output
    }

    fn process_ready_frames(&mut self) -> Vec<f32> {
        let frame_count = self.mic_pending.len().min(self.ref_pending.len()) / self.frame_samples;
        if frame_count == 0 {
            return Vec::new();
        }

        let process_len = frame_count * self.frame_samples;
        let mic_block: Vec<f32> = self.mic_pending.drain(..process_len).collect();
        let ref_block: Vec<f32> = self.ref_pending.drain(..process_len).collect();

        match &mut self.processor {
            RealtimeProcessor::WebRtc(webrtc) => {
                match webrtc.process_batch(&mic_block, &ref_block) {
                    Ok(processed) => processed,
                    Err(e) => {
                        warn!(
                            "WebRTC AEC3 real-time processing failed, passthrough chunk: {}",
                            e
                        );
                        mic_block
                    }
                }
            }
            RealtimeProcessor::Speex(speex) => speex.process_batch(&mic_block, &ref_block),
        }
    }
}

fn process_speex_batch(
    mic: &[f32],
    reference: &[f32],
    sample_rate: u32,
) -> Result<Vec<f32>, String> {
    if sample_rate != AEC_SAMPLE_RATE {
        return Err(format!(
            "Speex backend requires {} Hz input, got {} Hz",
            AEC_SAMPLE_RATE, sample_rate
        ));
    }

    let mut speex = EchoCanceller::new();
    Ok(speex.process_batch(mic, reference))
}

fn process_webrtc_batch(
    mic: &[f32],
    reference: &[f32],
    sample_rate: u32,
) -> Result<Vec<f32>, String> {
    let mut webrtc = WebRtcEchoCanceller::new(sample_rate, 1, 1)?;
    webrtc.process_batch(mic, reference)
}

struct WebRtcEchoCanceller {
    processor: VoipAec3,
    frame_samples: usize,
}

impl WebRtcEchoCanceller {
    fn new(
        sample_rate: u32,
        render_channels: usize,
        capture_channels: usize,
    ) -> Result<Self, String> {
        let processor = VoipAec3::builder(sample_rate as usize, render_channels, capture_channels)
            .enable_high_pass(true)
            .enable_noise_suppression(false)
            .build()
            .map_err(|e| format!("Failed to initialize WebRTC AEC3: {}", e))?;

        let frame_samples = processor.capture_frame_samples() * capture_channels;
        if frame_samples == 0 {
            return Err("WebRTC AEC3 returned zero frame size".to_string());
        }
        let expected_frame_samples =
            ((sample_rate as usize * AEC_FRAME_MS) / 1000) * capture_channels;
        if frame_samples != expected_frame_samples {
            warn!(
                "WebRTC AEC3 frame size mismatch: got {}, expected {}",
                frame_samples, expected_frame_samples
            );
        }

        info!(
            "WebRTC AEC3 initialized: {} Hz, frame={} samples",
            sample_rate, frame_samples
        );

        Ok(Self {
            processor,
            frame_samples,
        })
    }

    fn process_batch(&mut self, mic: &[f32], reference: &[f32]) -> Result<Vec<f32>, String> {
        if mic.is_empty() || reference.is_empty() {
            return Ok(mic.to_vec());
        }

        let processable_samples = mic.len().min(reference.len());
        let num_frames = processable_samples / self.frame_samples;
        if num_frames == 0 {
            return Ok(mic.to_vec());
        }

        let mut output = Vec::with_capacity(mic.len());
        let mut frame_output = vec![0.0f32; self.frame_samples];

        for frame_idx in 0..num_frames {
            let start = frame_idx * self.frame_samples;
            let end = start + self.frame_samples;
            let capture_frame = &mic[start..end];
            let render_frame = &reference[start..end];

            self.processor
                .process(capture_frame, Some(render_frame), false, &mut frame_output)
                .map_err(|e| format!("WebRTC AEC3 frame processing failed: {}", e))?;

            output.extend_from_slice(&frame_output);

            if frame_idx > 0 && frame_idx % 3000 == 0 {
                info!("WebRTC AEC3 progress: {} frames", frame_idx);
            }
        }

        let processed_samples = num_frames * self.frame_samples;
        if processed_samples < mic.len() {
            output.extend_from_slice(&mic[processed_samples..]);
        }

        Ok(output)
    }
}

/// Align system audio reference to microphone timing and pad/trim to mic length.
///
/// Positive `shift_samples` means the reference is delayed to match mic timing.
/// Negative `shift_samples` means the reference is advanced.
pub fn align_reference_for_aec(
    mic: &[f32],
    reference: &[f32],
    sample_rate: u32,
) -> (Vec<f32>, ReferenceAlignment) {
    if mic.is_empty() {
        return (
            Vec::new(),
            ReferenceAlignment {
                shift_samples: 0,
                correlation: 0.0,
            },
        );
    }

    if reference.is_empty() {
        return (
            vec![0.0; mic.len()],
            ReferenceAlignment {
                shift_samples: 0,
                correlation: 0.0,
            },
        );
    }

    let max_frames =
        ((ALIGN_MAX_ANALYSIS_SECONDS as usize * sample_rate as usize) / ALIGN_HOP_SAMPLES).max(1);
    let mic_env = envelope(mic, ALIGN_HOP_SAMPLES, max_frames);
    let ref_env = envelope(reference, ALIGN_HOP_SAMPLES, max_frames);

    if mic_env.len() < 2 || ref_env.len() < 2 {
        return (
            shift_and_pad(reference, 0, mic.len()),
            ReferenceAlignment {
                shift_samples: 0,
                correlation: 0.0,
            },
        );
    }

    let max_lag_frames =
        ((ALIGN_MAX_LAG_MS as usize * sample_rate as usize / 1000) / ALIGN_HOP_SAMPLES).max(1);
    let min_overlap_frames =
        ((ALIGN_MIN_OVERLAP_MS as usize * sample_rate as usize / 1000) / ALIGN_HOP_SAMPLES).max(1);

    let (best_shift_frames, best_corr) = best_shift_by_normalized_correlation(
        &mic_env,
        &ref_env,
        max_lag_frames,
        min_overlap_frames,
    );

    let shift_samples = if best_corr >= ALIGN_MIN_CORRELATION {
        best_shift_frames * ALIGN_HOP_SAMPLES as isize
    } else {
        0
    };

    (
        shift_and_pad(reference, shift_samples, mic.len()),
        ReferenceAlignment {
            shift_samples,
            correlation: best_corr,
        },
    )
}

fn envelope(samples: &[f32], hop: usize, max_frames: usize) -> Vec<f32> {
    samples
        .chunks(hop)
        .take(max_frames)
        .map(|chunk| {
            let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
            (sum_sq / chunk.len() as f32).sqrt()
        })
        .collect()
}

fn best_shift_by_normalized_correlation(
    mic_env: &[f32],
    ref_env: &[f32],
    max_lag_frames: usize,
    min_overlap_frames: usize,
) -> (isize, f32) {
    let mut best_shift = 0isize;
    let mut best_corr = -1.0f32;

    for shift in -(max_lag_frames as isize)..=(max_lag_frames as isize) {
        let Some((mic_start, ref_start, overlap)) =
            overlap_for_shift(mic_env.len(), ref_env.len(), shift)
        else {
            continue;
        };

        if overlap < min_overlap_frames {
            continue;
        }

        let mut dot = 0.0f32;
        let mut mic_pow = 0.0f32;
        let mut ref_pow = 0.0f32;

        for i in 0..overlap {
            let a = mic_env[mic_start + i];
            let b = ref_env[ref_start + i];
            dot += a * b;
            mic_pow += a * a;
            ref_pow += b * b;
        }

        if mic_pow <= f32::EPSILON || ref_pow <= f32::EPSILON {
            continue;
        }

        let corr = dot / (mic_pow.sqrt() * ref_pow.sqrt());
        if corr > best_corr {
            best_corr = corr;
            best_shift = shift;
        }
    }

    if best_corr.is_finite() {
        (best_shift, best_corr)
    } else {
        (0, 0.0)
    }
}

fn overlap_for_shift(len_a: usize, len_b: usize, shift: isize) -> Option<(usize, usize, usize)> {
    // Correlation definition: compare a[t] with b[t - shift].
    // shift > 0 means b is delayed.
    let (a_start, b_start) = if shift >= 0 {
        (shift as usize, 0usize)
    } else {
        (0usize, (-shift) as usize)
    };

    if a_start >= len_a || b_start >= len_b {
        return None;
    }

    let overlap = (len_a - a_start).min(len_b - b_start);
    if overlap == 0 {
        None
    } else {
        Some((a_start, b_start, overlap))
    }
}

fn shift_and_pad(reference: &[f32], shift_samples: isize, target_len: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; target_len];

    if target_len == 0 || reference.is_empty() {
        return out;
    }

    if shift_samples >= 0 {
        let shift = shift_samples as usize;
        if shift >= target_len {
            return out;
        }

        let copy_len = (target_len - shift).min(reference.len());
        out[shift..shift + copy_len].copy_from_slice(&reference[..copy_len]);
        return out;
    }

    let advance = (-shift_samples) as usize;
    if advance >= reference.len() {
        return out;
    }

    let src = &reference[advance..];
    let copy_len = target_len.min(src.len());
    out[..copy_len].copy_from_slice(&src[..copy_len]);
    out
}

/// Suppress residual coherent echo that may remain after adaptive AEC.
///
/// This pass attenuates microphone frames that remain highly correlated with the
/// system-audio reference while preserving mostly-uncorrelated local speech.
pub fn suppress_residual_echo(mic: &[f32], reference: &[f32], sample_rate: u32) -> Vec<f32> {
    if mic.is_empty() || reference.is_empty() || sample_rate == 0 {
        return mic.to_vec();
    }

    let frame_samples = ((sample_rate as usize) / 100).max(80);
    let processable = mic.len().min(reference.len());
    let mut output = mic.to_vec();
    let mut hangover = 0usize;

    for start in (0..processable).step_by(frame_samples) {
        let end = (start + frame_samples).min(processable);
        let mic_frame = &mic[start..end];
        let ref_frame = &reference[start..end];

        let mic_rms = rms(mic_frame);
        let ref_rms = rms(ref_frame);

        let mut gain = 1.0f32;
        if mic_rms >= RESIDUAL_MIN_MIC_RMS && ref_rms >= RESIDUAL_MIN_REF_RMS {
            let corr = normalized_correlation(mic_frame, ref_frame).abs();
            let likely_echo = corr >= RESIDUAL_CORRELATION_THRESHOLD;

            if likely_echo {
                hangover = RESIDUAL_HANGOVER_FRAMES;
            } else {
                hangover = hangover.saturating_sub(1);
            }

            if likely_echo || hangover > 0 {
                let echo_strength = ((corr - RESIDUAL_CORRELATION_THRESHOLD)
                    / (1.0 - RESIDUAL_CORRELATION_THRESHOLD))
                    .clamp(0.0, 1.0);
                let attenuation = echo_strength * RESIDUAL_MAX_ATTENUATION;
                gain = (1.0 - attenuation).clamp(RESIDUAL_MIN_GAIN, 1.0);

                // Preserve more local speech when mic energy clearly dominates.
                let energy_ratio = mic_rms / (ref_rms + f32::EPSILON);
                if energy_ratio > 1.4 {
                    let relief = ((energy_ratio - 1.4) / 3.0).clamp(0.0, 0.35);
                    gain = (gain + relief).min(1.0);
                }
            }
        } else {
            hangover = hangover.saturating_sub(1);
        }

        for sample in &mut output[start..end] {
            *sample *= gain;
        }
    }

    output
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|x| x * x).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

fn normalized_correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }

    let (mut dot, mut a_pow, mut b_pow) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        let av = a[i];
        let bv = b[i];
        dot += av * bv;
        a_pow += av * av;
        b_pow += bv * bv;
    }

    if a_pow <= f32::EPSILON || b_pow <= f32::EPSILON {
        0.0
    } else {
        dot / (a_pow.sqrt() * b_pow.sqrt())
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

    fn pseudo_noise(len: usize) -> Vec<f32> {
        let mut state: u32 = 0x1234_5678;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let unit = ((state >> 8) as f32) / ((1u32 << 24) - 1) as f32;
            out.push((unit * 2.0 - 1.0) * 0.5);
        }
        out
    }

    #[test]
    fn test_echo_backend_parse_aliases() {
        assert_eq!(
            EchoCancellationBackend::parse("webrtc"),
            Some(EchoCancellationBackend::WebRtcAec3)
        );
        assert_eq!(
            EchoCancellationBackend::parse("webrtc_aec3"),
            Some(EchoCancellationBackend::WebRtcAec3)
        );
        assert_eq!(
            EchoCancellationBackend::parse("speex"),
            Some(EchoCancellationBackend::Speex)
        );
        assert_eq!(EchoCancellationBackend::parse("invalid"), None);
    }

    #[test]
    fn test_process_echo_cancellation_keeps_length_webrtc() {
        let reference = pseudo_noise(3200);
        let mic: Vec<f32> = reference.iter().map(|x| x * 0.5).collect();
        let (output, info) =
            process_echo_cancellation(&mic, &reference, 16000, EchoCancellationBackend::WebRtcAec3);

        assert_eq!(output.len(), mic.len());
        assert_eq!(info.requested_backend, EchoCancellationBackend::WebRtcAec3);
    }

    #[test]
    fn test_process_echo_cancellation_keeps_length_speex() {
        let reference = pseudo_noise(3200);
        let mic: Vec<f32> = reference.iter().map(|x| x * 0.5).collect();
        let (output, info) =
            process_echo_cancellation(&mic, &reference, 16000, EchoCancellationBackend::Speex);

        assert_eq!(output.len(), mic.len());
        assert_eq!(info.requested_backend, EchoCancellationBackend::Speex);
    }

    #[test]
    fn test_realtime_echo_canceller_preserves_length_with_flush() {
        let reference = pseudo_noise(6400);
        let mic: Vec<f32> = reference.iter().map(|x| x * 0.7).collect();

        let mut realtime = RealtimeEchoCanceller::new(16000, EchoCancellationBackend::Speex)
            .expect("realtime aec init");

        let mut output = Vec::new();
        for start in (0..mic.len()).step_by(113) {
            let end = (start + 113).min(mic.len());
            output.extend(realtime.process_chunk(&mic[start..end], &reference[start..end]));
        }
        output.extend(realtime.flush());

        assert_eq!(output.len(), mic.len());
    }

    #[test]
    fn test_realtime_echo_canceller_passthrough_without_reference() {
        let mic = vec![0.2f32; 333];
        let mut realtime = RealtimeEchoCanceller::new(16000, EchoCancellationBackend::Speex)
            .expect("realtime aec init");

        let output = realtime.process_chunk(&mic, &[]);
        assert_eq!(output, mic);
    }

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

    #[test]
    fn test_align_reference_detects_positive_delay() {
        let shift = 320usize; // 20ms at 16kHz
        let reference = pseudo_noise(16000);

        let mut mic = vec![0.0f32; reference.len()];
        mic[shift..].copy_from_slice(&reference[..reference.len() - shift]);

        let (_aligned, alignment) = align_reference_for_aec(&mic, &reference, 16000);
        let error = (alignment.shift_samples - shift as isize).abs();

        assert!(error <= ALIGN_HOP_SAMPLES as isize);
        assert!(alignment.correlation > 0.1);
    }

    #[test]
    fn test_align_reference_detects_negative_delay() {
        let shift = 160usize; // 10ms at 16kHz
        let reference = pseudo_noise(16000);

        let mut mic = vec![0.0f32; reference.len()];
        mic[..reference.len() - shift].copy_from_slice(&reference[shift..]);

        let (_aligned, alignment) = align_reference_for_aec(&mic, &reference, 16000);
        let expected = -(shift as isize);
        let error = (alignment.shift_samples - expected).abs();

        assert!(error <= ALIGN_HOP_SAMPLES as isize);
        assert!(alignment.correlation > 0.1);
    }

    #[test]
    fn test_align_reference_output_length_matches_mic() {
        let mic = vec![0.0f32; 1024];
        let reference = vec![0.0f32; 512];
        let (aligned, _) = align_reference_for_aec(&mic, &reference, 16000);
        assert_eq!(aligned.len(), mic.len());
    }

    #[test]
    fn test_alignment_shift_ms_conversion() {
        let alignment = ReferenceAlignment {
            shift_samples: 160,
            correlation: 0.5,
        };
        assert!((alignment.shift_ms(16000) - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_residual_suppression_reduces_correlated_echo() {
        let reference = pseudo_noise(16000);
        let mic: Vec<f32> = reference.iter().map(|s| s * 0.8).collect();
        let output = suppress_residual_echo(&mic, &reference, 16000);

        let in_rms = rms(&mic);
        let out_rms = rms(&output);

        assert!(out_rms < in_rms * 0.65);
    }

    #[test]
    fn test_residual_suppression_preserves_uncorrelated_speech() {
        let reference = pseudo_noise(16000);
        let local_speech: Vec<f32> = (0..16000)
            .map(|i| {
                let t = i as f32 / 16000.0;
                (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.45
            })
            .collect();

        let mic: Vec<f32> = local_speech
            .iter()
            .zip(reference.iter())
            .map(|(speech, echo)| speech + echo * 0.05)
            .collect();

        let output = suppress_residual_echo(&mic, &reference, 16000);
        let in_rms = rms(&mic);
        let out_rms = rms(&output);

        assert!(out_rms > in_rms * 0.8);
    }
}
