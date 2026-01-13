# Meeting Scribe - Architecture & Planning Document v2.1

> **Project:** Local-first meeting transcription, summarization, and RAG app  
> **Stack:** Rust + Tauri + React  
> **Target Platforms:** Windows (primary), macOS, Linux  
> **Updated:** January 2026 - Combined from v1 and v2 with open source research

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Goals & Key Features](#goals--key-features)
3. [Open Source Landscape Analysis](#open-source-landscape-analysis)
4. [High-Level Architecture](#high-level-architecture)
5. [Audio Pipeline Deep Dive](#audio-pipeline-deep-dive)
6. [VAD Integration Strategy](#vad-integration-strategy)
7. [Audio Preprocessing & Denoising](#audio-preprocessing--denoising)
8. [Opus Encoding Strategy](#opus-encoding-strategy)
9. [Transcription Engine](#transcription-engine)
10. [Embedding Engine](#embedding-engine)
11. [LLM Engine](#llm-engine)
12. [Storage Strategy](#storage-strategy)
13. [Data Models](#data-models)
14. [User Interface Design](#user-interface-design)
15. [Model Management](#model-management)
16. [Build & Deployment](#build--deployment)
17. [Development Phases](#development-phases)
18. [Development Stories](#development-stories)
19. [Risk Assessment](#risk-assessment)
20. [Open Questions](#open-questions)
21. [References](#references)

---

## Executive Summary

Meeting Scribe is a privacy-first desktop application for recording, transcribing, summarizing, and searching meeting content. All processing happens locally using state-of-the-art open-source models.

This document combines the original architecture design (v1) with implementation research from successful open-source projects (v2):

| Project | Stars | Key Learnings |
|---------|-------|---------------|
| **screenpipe** (mediar-ai) | 16.1k | Audio capture architecture, VAD integration, FFmpeg encoding |
| **Meetily** (Zackriya-Solutions) | 8.2k | Tauri + Rust + Next.js stack, Whisper/Parakeet integration |
| **Vibe** (thewh1teagle) | - | Clean Tauri + Whisper implementation |
| **Handy** (cjpais) | - | Silero VAD, Parakeet V3, cross-platform audio |
| **kalosm-sound** | - | Elegant audio pipeline API design |

### Key Architecture Decisions

1. **Transcription**: Use `transcribe-rs` with Parakeet (4x faster) as default, Whisper as fallback
2. **VAD**: Use `voice_activity_detector` crate (Silero VAD V5 via ONNX)
3. **Denoising**: Use `nnnoiseless` (pure Rust RNNoise port)
4. **Audio Format**: WAV during recording → Opus for archival via FFmpeg
5. **Audio Capture**: `cpal` for mic, platform-specific for system audio
6. **Embeddings**: ONNX Runtime + EmbeddingGemma 300M
7. **LLM**: llama-cpp-2 for summarization and RAG chat

---

## Goals & Key Features

### Goals

- **Privacy-first**: All processing happens locally, no cloud dependencies
- **Real-time feedback**: Audio visualization and live transcription during recording
- **Meeting library**: Timeline view, searchable history, editable notes
- **RAG-enabled**: Chat with your meeting history using local embeddings and LLM
- **Cross-platform**: Windows → macOS → Linux (in priority order)

### Key Features

| Feature | MVP | V2 |
|---------|-----|-----|
| Dual audio capture (mic + system) | ✅ | |
| Audio waveform visualization | ✅ | |
| Voice Activity Detection (VAD) | ✅ | |
| Audio denoising | ✅ | |
| Post-meeting transcription | ✅ | |
| Live transcription | | ✅ |
| Speaker labeling (you vs others) | ✅ | |
| Speaker diarization (multiple speakers) | | ✅ |
| LLM summarization | ✅ | |
| Meeting timeline/library | ✅ | |
| Editable notes & context | ✅ | |
| Vector search (RAG) | ✅ | |
| Chat with meetings | ✅ | |
| Model download on first run | ✅ | |
| Opus audio archival | ✅ | |

---

## Open Source Landscape Analysis

### Screenpipe Architecture (mediar-ai/screenpipe)

Screenpipe has solved many of the hard problems we face:

```
screenpipe/
├── screenpipe-audio/        # Audio capture & STT
│   ├── src/
│   │   ├── core.rs          # Main audio pipeline
│   │   ├── stt.rs           # Speech-to-text (Whisper Distil large v3)
│   │   ├── vad.rs           # Voice activity detection (Silero)
│   │   └── encode.rs        # FFmpeg encoding
├── screenpipe-core/         # Shared utilities
├── screenpipe-db/           # SQLite storage
└── screenpipe-server/       # HTTP API
```

**Key Insights:**
- Uses Whisper Distil large v3 as default STT
- Manual Silero VAD implementation branch exists
- FFmpeg for mp4/audio encoding
- SQLite for all metadata storage
- ~10% CPU, 4GB RAM, 15GB/month storage footprint

### transcribe-rs (cjpais/transcribe-rs) ⭐ **Our Choice**

A clean abstraction over multiple transcription engines, extracted from the Handy meeting app:

```rust
// Unified transcription interface - swap engines without code changes
pub trait TranscriptionEngine {
    fn load_model(&mut self, path: &PathBuf) -> Result<()>;
    fn transcribe_file(&self, path: &PathBuf, options: Option<TranscriptionOptions>) -> Result<TranscriptionResult>;
}

// Supported engines (all with same interface):
// - ParakeetEngine: NVIDIA's model, 4x faster than Whisper
// - WhisperEngine: OpenAI Whisper via whisper.cpp
// - WhisperfileEngine: Mozilla's llamafile-based Whisper
// - MoonshineEngine: UsefulSensors multilingual model
```

**Why we chose transcribe-rs:**
- **Speed**: Parakeet is 4x faster than Whisper on CPU
- **Flexibility**: Users can switch engines in settings
- **Battle-tested**: Extracted from Handy, a real meeting app
- **Same GPU support**: Metal, Vulkan, CUDA all work
- **Clean API**: Unified trait makes our code simpler

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                 TAURI SHELL                                      │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                         REACT FRONTEND                                      │ │
│  │                                                                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │ │
│  │  │  Recording  │  │  Meeting    │  │  Meeting    │  │    RAG      │       │ │
│  │  │    View     │  │  Library    │  │   Detail    │  │    Chat     │       │ │
│  │  │             │  │  (Timeline) │  │   Editor    │  │  Interface  │       │ │
│  │  │ - Waveform  │  │             │  │             │  │             │       │ │
│  │  │ - Live text │  │ - Calendar  │  │ - Transcript│  │ - Query     │       │ │
│  │  │ - Controls  │  │ - Search    │  │ - Notes     │  │ - Sources   │       │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘       │ │
│  │                                                                             │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                    │                                             │
│                           Tauri Commands (IPC)                                   │
│                                    │                                             │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                          RUST BACKEND                                       │ │
│  │                                                                             │ │
│  │  ┌───────────────────────────────────────────────────────────────────────┐ │ │
│  │  │                    AUDIO CAPTURE LAYER                                 │ │ │
│  │  │                                                                        │ │ │
│  │  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐           │ │ │
│  │  │   │  Mic Input   │    │System Audio  │    │  Audio Mixer │           │ │ │
│  │  │   │   (cpal)     │    │ (platform)   │    │  & Buffer    │           │ │ │
│  │  │   │              │    │              │    │              │           │ │ │
│  │  │   │ Label: "you" │    │Label:"others"│    │ Ring buffers │           │ │ │
│  │  │   └──────┬───────┘    └──────┬───────┘    │ WAV encoder  │           │ │ │
│  │  │          │                   │            └──────────────┘           │ │ │
│  │  │          └─────────┬─────────┘                                       │ │ │
│  │  │                    ▼                                                 │ │ │
│  │  │  ┌─────────────────────────────────────────────────────────────┐    │ │ │
│  │  │  │              AUDIO PREPROCESSING PIPELINE                    │    │ │ │
│  │  │  │                                                              │    │ │ │
│  │  │  │  ┌────────────┐   ┌────────────┐   ┌────────────┐          │    │ │ │
│  │  │  │  │ Resampler  │──▶│  Denoiser  │──▶│    VAD     │          │    │ │ │
│  │  │  │  │  (rubato)  │   │(nnnoiseless│   │  (Silero)  │          │    │ │ │
│  │  │  │  │            │   │   48kHz)   │   │            │          │    │ │ │
│  │  │  │  │  → 16kHz   │   │            │   │ Timestamps │          │    │ │ │
│  │  │  │  └────────────┘   └────────────┘   └─────┬──────┘          │    │ │ │
│  │  │  │                                          │                  │    │ │ │
│  │  │  │                      Speech Chunks ◀─────┘                  │    │ │ │
│  │  │  └─────────────────────────────────────────────────────────────┘    │ │ │
│  │  └───────────────────────────────────────────────────────────────────────┘ │ │
│  │                                                                             │ │
│  │  ┌───────────────────────────────────────────────────────────────────────┐ │ │
│  │  │                     INFERENCE SUBSYSTEM                                │ │ │
│  │  │                                                                        │ │ │
│  │  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐       │ │ │
│  │  │  │  Transcription  │  │    Embedding    │  │   LLM Engine    │       │ │ │
│  │  │  │    Engine       │  │     Engine      │  │                 │       │ │ │
│  │  │  │                 │  │                 │  │                 │       │ │ │
│  │  │  │  transcribe-rs  │  │ ort + gemma-300m│  │  llama-cpp-2    │       │ │ │
│  │  │  │  (Parakeet/     │  │ (ONNX Runtime)  │  │  (llama.cpp)    │       │ │ │
│  │  │  │   Whisper/Moon) │  │                 │  │                 │       │ │ │
│  │  │  └─────────────────┘  └─────────────────┘  └─────────────────┘       │ │ │
│  │  └───────────────────────────────────────────────────────────────────────┘ │ │
│  │                                                                             │ │
│  │  ┌───────────────────────────────────────────────────────────────────────┐ │ │
│  │  │                      STORAGE SUBSYSTEM                                 │ │ │
│  │  │                                                                        │ │ │
│  │  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐       │ │ │
│  │  │  │     SQLite      │  │    LanceDB      │  │   Audio Files   │       │ │ │
│  │  │  │   (rusqlite)    │  │   (embedded)    │  │  WAV → Opus     │       │ │ │
│  │  │  │                 │  │                 │  │                 │       │ │ │
│  │  │  │ - Meetings      │  │ - Transcript    │  │ - meeting_xxx/  │       │ │ │
│  │  │  │ - Transcripts   │  │   embeddings    │  │   - you.opus    │       │ │ │
│  │  │  │ - Notes         │  │ - Note vectors  │  │   - others.opus │       │ │ │
│  │  │  │ - Settings      │  │ - 768-dim       │  │                 │       │ │ │
│  │  │  └─────────────────┘  └─────────────────┘  └─────────────────┘       │ │ │
│  │  └───────────────────────────────────────────────────────────────────────┘ │ │
│  │                                                                             │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Audio Pipeline Deep Dive

### Platform-Specific Audio Capture

| Platform | Mic Input | System Audio Loopback | Crate/API |
|----------|-----------|----------------------|-----------|
| **Windows** | `cpal` default input | WASAPI loopback | `cpal` with loopback feature |
| **macOS** | `cpal` default input | ScreenCaptureKit | `cidre` crate or `cpal` fork |
| **Linux** | `cpal` default input | PipeWire/PulseAudio monitor | `cpal` + pipewire config |

### Audio Format Standards

```rust
/// Recording format - optimized for Whisper
pub const WHISPER_SAMPLE_RATE: u32 = 16000;
pub const CHANNELS: u16 = 1;  // Mono
pub const BITS_PER_SAMPLE: u16 = 16;  // i16 samples

/// Denoising format - nnnoiseless requires 48kHz
pub const DENOISE_SAMPLE_RATE: u32 = 48000;

/// VAD format - Silero V5 requirements
pub const VAD_SAMPLE_RATE: u32 = 16000;  // or 8000
pub const VAD_CHUNK_SIZE_16K: usize = 512;  // Only valid size for 16kHz
pub const VAD_CHUNK_SIZE_8K: usize = 256;   // Only valid size for 8kHz
```

### Audio Buffer Architecture

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use ringbuf::{HeapRb, Producer, Consumer};

/// Thread-safe audio buffer manager
pub struct AudioBufferManager {
    /// Mic audio ring buffer ("you")
    mic_producer: Producer<f32, Arc<HeapRb<f32>>>,
    mic_consumer: Consumer<f32, Arc<HeapRb<f32>>>,
    
    /// System audio ring buffer ("others")
    system_producer: Producer<f32, Arc<HeapRb<f32>>>,
    system_consumer: Consumer<f32, Arc<HeapRb<f32>>>,
    
    /// Buffer configuration
    config: BufferConfig,
}

pub struct BufferConfig {
    /// Size of ring buffer in samples (default: 30 seconds at 16kHz)
    pub buffer_size: usize,
    /// Chunk size for processing (default: 512 for 16kHz VAD)
    pub chunk_size: usize,
    /// Sample rate
    pub sample_rate: u32,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            buffer_size: 16000 * 30,  // 30 seconds
            chunk_size: 512,
            sample_rate: 16000,
        }
    }
}
```

### Waveform Visualization Data

```rust
use serde::{Deserialize, Serialize};

/// Emitted to frontend every ~50ms for smooth visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformUpdate {
    pub timestamp_ms: u64,
    pub mic: ChannelMetrics,
    pub system: ChannelMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMetrics {
    /// Root mean square (0.0 - 1.0)
    pub rms: f32,
    /// Peak amplitude (0.0 - 1.0)
    pub peak: f32,
    /// Downsampled waveform for rendering (typically 64-128 points)
    pub samples: Vec<f32>,
    /// VAD speech probability (if available)
    pub speech_probability: Option<f32>,
}

impl ChannelMetrics {
    pub fn from_samples(samples: &[f32], downsample_to: usize) -> Self {
        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        
        // Downsample for visualization
        let step = samples.len() / downsample_to;
        let downsampled: Vec<f32> = samples
            .chunks(step.max(1))
            .map(|chunk| chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max))
            .collect();
        
        Self {
            rms: rms.min(1.0),
            peak: peak.min(1.0),
            samples: downsampled,
            speech_probability: None,
        }
    }
}
```

---

## VAD Integration Strategy

### Recommended Crate: `voice_activity_detector`

This crate wraps Silero VAD V5 with a clean Rust API and includes the ONNX model bundled.

```toml
[dependencies]
voice_activity_detector = "0.2"
```

### Basic VAD Usage

```rust
use voice_activity_detector::{VoiceActivityDetector, LabeledAudio, IteratorExt};

/// Initialize VAD for 16kHz audio
pub fn create_vad() -> Result<VoiceActivityDetector, voice_activity_detector::Error> {
    VoiceActivityDetector::builder()
        .sample_rate(16000)
        .chunk_size(512usize)  // Required size for 16kHz in Silero V5
        .build()
}

/// Process audio samples and get speech timestamps
pub fn detect_speech(
    vad: &mut VoiceActivityDetector,
    samples: &[i16],
    threshold: f32,
    padding_chunks: usize,
) -> Vec<SpeechSegment> {
    let labels = samples.iter().copied().label(vad, threshold, padding_chunks);
    
    let mut segments = Vec::new();
    let mut current_start: Option<usize> = None;
    let mut sample_idx = 0;
    
    for label in labels {
        match label {
            LabeledAudio::Speech(chunk) => {
                if current_start.is_none() {
                    current_start = Some(sample_idx);
                }
                sample_idx += chunk.len();
            }
            LabeledAudio::NonSpeech(chunk) => {
                if let Some(start) = current_start.take() {
                    segments.push(SpeechSegment {
                        start_sample: start,
                        end_sample: sample_idx,
                    });
                }
                sample_idx += chunk.len();
            }
        }
    }
    
    // Handle trailing speech
    if let Some(start) = current_start {
        segments.push(SpeechSegment {
            start_sample: start,
            end_sample: sample_idx,
        });
    }
    
    segments
}

#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub start_sample: usize,
    pub end_sample: usize,
}

impl SpeechSegment {
    pub fn start_ms(&self, sample_rate: u32) -> u64 {
        (self.start_sample as u64 * 1000) / sample_rate as u64
    }
    
    pub fn end_ms(&self, sample_rate: u32) -> u64 {
        (self.end_sample as u64 * 1000) / sample_rate as u64
    }
}
```

### VAD Configuration Guidelines

```rust
/// VAD parameters tuned for meeting transcription
pub struct VadConfig {
    /// Speech detection threshold (0.0-1.0)
    /// Higher = stricter (fewer false positives, might miss quiet speech)
    /// Recommended: 0.5 for general use, 0.65 for noisy environments
    pub threshold: f32,
    
    /// Minimum speech duration in milliseconds
    /// Filters out very short sounds (clicks, etc.)
    /// Recommended: 250ms for meetings
    pub min_speech_duration_ms: u32,
    
    /// Minimum silence duration before ending a speech segment
    /// Higher = more context kept together, fewer segments
    /// Recommended: 300-500ms for natural speech grouping
    pub min_silence_duration_ms: u32,
    
    /// Padding added before/after speech segments
    /// Prevents cutting off word beginnings/endings
    /// Recommended: 30-50ms
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
```

---

## Audio Preprocessing & Denoising

### Using nnnoiseless (Pure Rust RNNoise)

```toml
[dependencies]
nnnoiseless = "0.2"
rubato = "0.14"  # For resampling
```

```rust
use nnnoiseless::DenoiseState;
use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

/// Audio denoiser wrapper
pub struct AudioDenoiser {
    state: DenoiseState<'static>,
    // nnnoiseless uses 480-sample frames at 48kHz
}

impl AudioDenoiser {
    pub fn new() -> Self {
        Self {
            state: DenoiseState::new(),
        }
    }
    
    /// Process 480 samples at 48kHz, returns denoised samples
    pub fn denoise_frame(&mut self, input: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), 480, "nnnoiseless requires 480-sample frames at 48kHz");
        let mut output = vec![0.0f32; 480];
        self.state.process_frame(&mut output, input);
        output
    }
}

/// Resampler for converting between sample rates
pub fn create_resampler(from_rate: u32, to_rate: u32) -> SincFixedIn<f32> {
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
        1,
    ).unwrap()
}
```

---

## Opus Encoding Strategy

### Using FFmpeg CLI (Recommended)

```rust
use std::process::Command;
use std::path::Path;

/// Encode WAV to Opus using FFmpeg
pub fn encode_to_opus(
    input_path: &Path,
    output_path: &Path,
    bitrate_kbps: u32,
) -> std::io::Result<()> {
    let status = Command::new("ffmpeg")
        .args([
            "-i", input_path.to_str().unwrap(),
            "-c:a", "libopus",
            "-b:a", &format!("{}k", bitrate_kbps),
            "-vbr", "on",
            "-compression_level", "10",
            "-application", "voip",         // Optimized for speech
            output_path.to_str().unwrap(),
        ])
        .output()?;
    
    if !status.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&status.stderr),
        ));
    }
    
    Ok(())
}

/// Recommended bitrates for speech
pub struct OpusBitrates;

impl OpusBitrates {
    /// Minimum quality, smallest files (~6 KB/min)
    pub const SPEECH_LOW: u32 = 16;
    /// Good quality for meetings (~12 KB/min)
    pub const SPEECH_MEDIUM: u32 = 32;
    /// High quality (~24 KB/min)
    pub const SPEECH_HIGH: u32 = 48;
}
```

### Storage Size Comparison

| Format | Bitrate | 1 Hour Meeting | 8 Hours/Day | 1 Month |
|--------|---------|---------------|-------------|---------|
| WAV 16kHz mono | 256 kbps | 115 MB | 920 MB | 27.6 GB |
| Opus 32kbps | 32 kbps | 14 MB | 112 MB | 3.4 GB |
| Opus 48kbps | 48 kbps | 21 MB | 168 MB | 5.0 GB |
| **Compression ratio** | - | **5-8x smaller** | - | - |

---

## Transcription Engine

### transcribe-rs Integration

```toml
[dependencies]
transcribe-rs = { git = "https://github.com/cjpais/transcribe-rs", features = ["parakeet", "whisper", "moonshine"] }
```

```rust
use transcribe_rs::{TranscriptionEngine, TranscriptionResult, TranscriptionOptions};
use std::path::PathBuf;

/// Supported transcription backends
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TranscriptionBackend {
    /// NVIDIA Parakeet - 4x faster than Whisper on CPU, English-optimized
    Parakeet,
    /// OpenAI Whisper - More language support, widely known
    Whisper,
    /// UsefulSensors Moonshine - Multilingual (8 languages), efficient
    Moonshine,
}

impl Default for TranscriptionBackend {
    fn default() -> Self {
        Self::Parakeet // Fastest for meetings
    }
}

/// Wrapper around transcribe-rs engines
pub struct TranscriptionService {
    engine: Box<dyn TranscriptionEngine + Send + Sync>,
    backend: TranscriptionBackend,
    models_dir: PathBuf,
}

impl TranscriptionService {
    /// Transcribe an audio file
    pub fn transcribe(&self, audio_path: &PathBuf) -> Result<Vec<TranscriptSegment>, TranscriptionError> {
        let options = TranscriptionOptions {
            language: Some("en".to_string()),
            ..Default::default()
        };
        
        let result = self.engine.transcribe_file(audio_path, Some(options))?;
        let segments = self.convert_result(result);
        Ok(segments)
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Speaker,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Speaker {
    You,
    Others,
    Unknown,
}
```

### Engine Comparison

| Engine | Speed | Size | Languages | Best For |
|--------|-------|------|-----------|----------|
| **Parakeet V3** | ⚡⚡⚡⚡ | 450MB | English | **Default** - meetings usually English |
| **Whisper large-v3-turbo** | ⚡⚡ | 1.6GB | 100+ | Multilingual meetings |
| **Moonshine** | ⚡⚡⚡ | 300MB | 8 | Balance of speed + languages |

### GPU Acceleration

| Platform | Parakeet | Whisper | Moonshine |
|----------|----------|---------|-----------|
| **macOS (Metal)** | ✅ | ✅ | ✅ |
| **Windows (Vulkan)** | ✅ | ✅ | ✅ |
| **Windows (CUDA)** | ✅ | ✅ | ✅ |
| **Linux (Vulkan)** | ✅ | ✅ | ✅ |
| **Linux (CUDA)** | ✅ | ✅ | ✅ |

---

## Embedding Engine

### Library: `ort` (ONNX Runtime)

### Model: `embeddinggemma-300m-ONNX`

- **Size:** ~600MB (fp32), ~300MB (q8), ~150MB (q4)
- **Dimensions:** 768 (can truncate to 512/256/128 via MRL)
- **Context:** 2048 tokens max
- **Languages:** 100+

**Important:** Does NOT support fp16. Use fp32, q8, or q4.

### Task-Specific Prompts

```rust
// For transcript chunks (documents)
fn document_prompt(text: &str) -> String {
    format!("title: none | text: {}", text)
}

// For search queries
fn query_prompt(text: &str) -> String {
    format!("task: search result | query: {}", text)
}

// For question answering
fn qa_prompt(text: &str) -> String {
    format!("task: question answering | query: {}", text)
}
```

### Hardware Acceleration

```toml
[dependencies.ort]
version = "2.0"
features = ["load-dynamic"]  # Recommended for flexibility

# Platform-specific acceleration
[target.'cfg(windows)'.dependencies.ort]
features = ["directml"]  # Works with any DirectX 12 GPU

[target.'cfg(target_os = "macos")'.dependencies.ort]
features = ["coreml"]

[target.'cfg(target_os = "linux")'.dependencies.ort]
features = ["cuda"]  # or "rocm" for AMD
```

---

## LLM Engine

### Library: `llama-cpp-2`

Low-level bindings to llama.cpp for GGUF model inference.

### Recommended Models

| Model | Size | Context | Use Case |
|-------|------|---------|----------|
| Llama 3.2 3B Q4 | ~2GB | 8K | Fast summaries |
| Mistral 7B Q4 | ~4GB | 8K | Better quality |
| Llama 3.1 8B Q4 | ~5GB | 8K | Best local quality |
| Qwen2.5 7B Q4 | ~4GB | 32K | Long context |

### Usage Patterns

```rust
// Summarization prompt
const SUMMARY_PROMPT: &str = r#"
You are a meeting assistant. Summarize the following transcript.
Extract:
1. Key discussion points
2. Decisions made
3. Action items with owners
4. Unresolved questions

Transcript:
{transcript}

Summary:
"#;

// RAG chat prompt
const RAG_PROMPT: &str = r#"
Use the following meeting excerpts to answer the question.
If the answer isn't in the excerpts, say so.

Excerpts:
{context}

Question: {question}

Answer:
"#;
```

---

## Storage Strategy

### File System Layout

```
~/.meeting-scribe/
├── data/
│   ├── meetings.db              # SQLite database
│   └── vectors/                 # LanceDB directory
│       └── embeddings.lance
├── audio/
│   └── {meeting_id}/
│       ├── you.wav              # During recording
│       ├── others.wav           # During recording
│       ├── you.opus             # After archival
│       └── others.opus          # After archival
├── models/
│   ├── parakeet/
│   │   └── parakeet-v3-int8/
│   ├── whisper/
│   │   └── ggml-large-v3-turbo.bin
│   ├── vad/
│   │   └── silero_vad.onnx
│   ├── embedding/
│   │   ├── model.onnx
│   │   └── model.onnx_data
│   └── llm/
│       └── llama-3.2-3b-q4.gguf
├── cache/
│   └── waveform/                # Cached waveform data
└── config.json
```

### Audio Lifecycle

```
Recording Start
      │
      ▼
┌─────────────────┐
│  WAV Recording  │ ◀── Real-time capture
│  (16kHz, mono)  │     Both channels
└────────┬────────┘
         │
    Meeting End
         │
         ▼
┌─────────────────┐
│  Preprocessing  │ ◀── VAD, denoising
│  & Transcription│
└────────┬────────┘
         │
    After X days (configurable)
         │
         ▼
┌─────────────────┐
│  Opus Archival  │ ◀── 5-8x compression
│  (48kbps)       │
└────────┬────────┘
         │
    Delete WAV
         │
         ▼
┌─────────────────┐
│  Archived       │ ◀── Long-term storage
│  (Opus only)    │
└─────────────────┘
```

---

## Data Models

### SQLite Schema

```sql
-- Core meeting data
CREATE TABLE meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    duration_ms INTEGER,
    
    -- Audio file references
    audio_path_you TEXT,
    audio_path_others TEXT,
    audio_format TEXT DEFAULT 'wav',  -- 'wav' or 'opus'
    
    -- Processing status
    status TEXT CHECK(status IN ('recording', 'processing', 'ready', 'archived', 'error')),
    error_message TEXT,
    
    -- Metadata
    tags TEXT,  -- JSON array
    notes_count INTEGER DEFAULT 0
);

-- Transcript segments with speaker labels
CREATE TABLE transcript_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    text TEXT NOT NULL,
    speaker TEXT CHECK(speaker IN ('you', 'others', 'unknown')),
    confidence REAL,
    embedding_id TEXT  -- Reference to LanceDB
);

-- User notes attached to meetings
CREATE TABLE notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    embedding_id TEXT
);

-- Generated summaries
CREATE TABLE summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    summary_type TEXT NOT NULL,  -- 'key_points', 'action_items', 'full'
    content TEXT NOT NULL,
    model_used TEXT,
    created_at INTEGER NOT NULL
);

-- Application settings
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Model download status
CREATE TABLE models (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,  -- 'transcription', 'embedding', 'llm', 'vad'
    path TEXT,
    size_bytes INTEGER,
    status TEXT CHECK(status IN ('not_downloaded', 'downloading', 'ready', 'error')),
    download_progress REAL,
    error_message TEXT
);

-- Indexes
CREATE INDEX idx_meetings_created ON meetings(created_at DESC);
CREATE INDEX idx_meetings_status ON meetings(status);
CREATE INDEX idx_segments_meeting ON transcript_segments(meeting_id);
CREATE INDEX idx_segments_time ON transcript_segments(meeting_id, start_ms);
CREATE INDEX idx_notes_meeting ON notes(meeting_id);

-- Full-text search
CREATE VIRTUAL TABLE transcript_fts USING fts5(
    text,
    content='transcript_segments',
    content_rowid='id'
);
```

### LanceDB Structure

```rust
// Vector table schema
struct EmbeddingRecord {
    id: String,           // UUID
    meeting_id: String,   // FK to SQLite
    chunk_type: String,   // "transcript" | "note" | "summary"
    text: String,         // Original text for display
    start_ms: Option<i64>,// For transcript chunks
    vector: Vec<f32>,     // 768-dim embedding
}
```

---

## User Interface Design

### 1. Recording View (Main)

```
┌─────────────────────────────────────────────────────────────────┐
│  Meeting Scribe                                    [─] [□] [×]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              RECORDING: 00:45:23                         │   │
│  │                                                          │   │
│  │  You:    ▁▃▅▇▅▃▁▂▄▆▄▂▁▃▅▇▅▃▁▂▄▆▄▂▁▃▅▇▅▃▁              │   │
│  │  Others: ▁▁▂▃▄▃▂▁▁▂▃▄▃▂▁▁▂▃▄▃▂▁▁▂▃▄▃▂▁▁              │   │
│  │                                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Live Transcript (optional - V2)                         │   │
│  │                                                          │   │
│  │  [You] So I think we should focus on the API first...    │   │
│  │  [Others] That makes sense. What about the timeline?     │   │
│  │  [You] Let's aim for two weeks for the MVP...            │   │
│  │  █                                                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│            [ ⏹ Stop Recording ]    [ ⏸ Pause ]                  │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  [🎙 Record] [📚 Library] [💬 Chat] [⚙ Settings]               │
└─────────────────────────────────────────────────────────────────┘
```

### 2. Meeting Library (Timeline)

```
┌─────────────────────────────────────────────────────────────────┐
│  Meeting Scribe                                    [─] [□] [×]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  🔍 Search meetings...                      [Filter ▼] [Sort ▼]│
│                                                                 │
│  ─────────────── January 2026 ───────────────                  │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  📅 Jan 12, 2026 • 10:30 AM                    45 min   │   │
│  │  Sprint Planning Meeting                                 │   │
│  │  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄   │   │
│  │  Discussed Q1 priorities and assigned tasks...           │   │
│  │  🏷 #planning #sprint                           [→]      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  📅 Jan 10, 2026 • 2:00 PM                     30 min   │   │
│  │  Client Sync - Acme Corp                                 │   │
│  │  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄   │   │
│  │  Reviewed project timeline and deliverables...           │   │
│  │  🏷 #client #acme                               [→]      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ─────────────── December 2025 ───────────────                 │
│  ...                                                            │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  [🎙 Record] [📚 Library] [💬 Chat] [⚙ Settings]               │
└─────────────────────────────────────────────────────────────────┘
```

### 3. Meeting Detail / Editor

```
┌─────────────────────────────────────────────────────────────────┐
│  ← Back                                        [─] [□] [×]      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Title: [Sprint Planning Meeting____________]            │   │
│  │  Date:  Jan 12, 2026 • 10:30 AM              Duration: 45m│   │
│  │  Tags:  [planning] [sprint] [+ Add tag]                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ [Transcript] [Summary] [Notes] [Audio]                   │   │
│  ├─────────────────────────────────────────────────────────┤   │
│  │                                                          │   │
│  │  00:00:12 [You]                                          │   │
│  │  Alright, let's get started with the sprint planning.    │   │
│  │  We have a lot to cover today.                           │   │
│  │                                                          │   │
│  │  00:00:28 [Others]                                       │   │
│  │  Sounds good. I've prepared the backlog items we         │   │
│  │  discussed last week.                                    │   │
│  │                                                          │   │
│  │  00:00:45 [You]                                          │   │
│  │  Great. Let's start with the API refactoring task.       │   │
│  │  I think we should prioritize that this sprint.         │   │
│  │                                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  📝 My Notes                                    [Edit]   │   │
│  │  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄   │   │
│  │  Key decisions:                                          │   │
│  │  - API refactoring is top priority                       │   │
│  │  - Sarah will lead the effort                            │   │
│  │  - Target completion: Jan 26                             │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  [🎙 Record] [📚 Library] [💬 Chat] [⚙ Settings]               │
└─────────────────────────────────────────────────────────────────┘
```

### 4. RAG Chat Interface

```
┌─────────────────────────────────────────────────────────────────┐
│  Meeting Scribe - Chat                             [─] [□] [×]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  🤖 Ask anything about your meetings...                  │   │
│  │                                                          │   │
│  │  ────────────────────────────────────────────────────   │   │
│  │                                                          │   │
│  │  👤 What did we decide about the API timeline?           │   │
│  │                                                          │   │
│  │  🤖 Based on your Sprint Planning meeting on Jan 12:     │   │
│  │                                                          │   │
│  │  The team decided to prioritize API refactoring this     │   │
│  │  sprint with a target completion date of January 26.     │   │
│  │  Sarah was assigned to lead the effort.                  │   │
│  │                                                          │   │
│  │  📎 Sources:                                             │   │
│  │  └─ Sprint Planning Meeting (Jan 12) @ 00:00:45         │   │
│  │  └─ Your notes on Sprint Planning                        │   │
│  │                                                          │   │
│  │  ────────────────────────────────────────────────────   │   │
│  │                                                          │   │
│  │  👤 Who mentioned concerns about the deadline?           │   │
│  │                                                          │   │
│  │  🤖 ...                                                  │   │
│  │                                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  [_____________________________________________] [Send]  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  [🎙 Record] [📚 Library] [💬 Chat] [⚙ Settings]               │
└─────────────────────────────────────────────────────────────────┘
```

### 5. Settings - Models

```
┌─────────────────────────────────────────────────────────────────┐
│  Settings > Models                                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Transcription Engine                                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  [✓] Parakeet V3 (450 MB)  ✅ Downloaded                 │   │
│  │  [ ] Whisper Large V3 Turbo (1.6 GB)  ⬇ Download        │   │
│  │  [ ] Moonshine (300 MB)                                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Embedding Model                                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  [✓] EmbeddingGemma 300M (q8)  ⬇ Download (300 MB)      │   │
│  │      ████████████░░░░░░░░ 60%                            │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Language Model (Summarization & Chat)                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  [✓] Llama 3.2 3B Q4 (2 GB)  ✅ Downloaded               │   │
│  │  [ ] Mistral 7B Q4 (4 GB)                                │   │
│  │  [ ] Llama 3.1 8B Q4 (5 GB)                              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  Hardware Acceleration                                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Detected: NVIDIA RTX 3080 (CUDA)                        │   │
│  │  [✓] Use GPU acceleration                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Model Management

### Download Strategy

Models are downloaded on first run or when explicitly requested by the user.

### Download Sources

| Model Type | Source | Files |
|------------|--------|-------|
| Parakeet | Handy CDN | `parakeet-v3-int8.tar.gz` |
| Whisper | HuggingFace (ggerganov/whisper.cpp) | `ggml-large-v3-turbo.bin` |
| Moonshine | HuggingFace (UsefulSensors) | ONNX merged model |
| Embedding | HuggingFace (onnx-community) | `model.onnx`, `model.onnx_data` |
| LLM | HuggingFace (various) | `*.gguf` |

### Download Flow

```rust
enum ModelStatus {
    NotDownloaded,
    Downloading { progress: f32 },
    Downloaded,
    Error(String),
}

struct ModelManager {
    transcription: HashMap<TranscriptionBackend, ModelStatus>,
    embedding: ModelStatus,
    llm: ModelStatus,
}
```

---

## Build & Deployment

### Project Structure

```
meeting-scribe/
├── src-tauri/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── audio/
│   │   │   ├── mod.rs
│   │   │   ├── capture.rs
│   │   │   ├── buffer.rs
│   │   │   ├── vad.rs
│   │   │   ├── denoise.rs
│   │   │   └── platform/
│   │   │       ├── windows.rs
│   │   │       ├── macos.rs
│   │   │       └── linux.rs
│   │   ├── inference/
│   │   │   ├── mod.rs
│   │   │   ├── transcription.rs
│   │   │   ├── embedding.rs
│   │   │   └── llm.rs
│   │   ├── storage/
│   │   │   ├── mod.rs
│   │   │   ├── sqlite.rs
│   │   │   └── vectors.rs
│   │   ├── models/
│   │   │   ├── mod.rs
│   │   │   └── downloader.rs
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── recording.rs
│   │       ├── meetings.rs
│   │       └── chat.rs
│   └── tauri.conf.json
├── src/                          # React frontend
│   ├── App.tsx
│   ├── components/
│   │   ├── Recording/
│   │   ├── Library/
│   │   ├── MeetingDetail/
│   │   ├── Chat/
│   │   └── Settings/
│   ├── hooks/
│   ├── stores/
│   └── styles/
├── package.json
└── README.md
```

### Cargo.toml Dependencies

```toml
[package]
name = "meeting-scribe"
version = "0.1.0"
edition = "2021"

[dependencies]
# Tauri
tauri = { version = "2", features = ["macos-private-api"] }
tauri-plugin-shell = "2"

# Async runtime
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Audio capture
cpal = "0.15"
hound = "3.5"              # WAV encoding
ringbuf = "0.3"            # Ring buffers

# Audio preprocessing
rubato = "0.14"            # Resampling
nnnoiseless = "0.2"        # Denoising (48kHz)

# Voice Activity Detection
voice_activity_detector = "0.2"

# Transcription - unified interface over multiple engines
transcribe-rs = { git = "https://github.com/cjpais/transcribe-rs", features = ["parakeet", "whisper", "moonshine"] }

# Embedding (ONNX Runtime)
ort = { version = "2", features = ["load-dynamic"] }
tokenizers = "0.19"

# LLM
llama-cpp-2 = "0.1"

# Storage
rusqlite = { version = "0.31", features = ["bundled", "backup"] }
lancedb = "0.10"
arrow-array = "52"

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.11", features = ["stream", "json"] }
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
parking_lot = "0.12"
dirs = "5"

# Platform-specific
[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = ["Win32_Media_Audio", "Win32_System_Com"] }

[target.'cfg(target_os = "macos")'.dependencies]
cidre = "0.4"  # ScreenCaptureKit bindings

[features]
default = ["parakeet"]
parakeet = []      # NVIDIA Parakeet - fastest, English
whisper = []       # OpenAI Whisper - most languages  
moonshine = []     # UsefulSensors Moonshine - multilingual, efficient
cuda = ["ort/cuda"]
metal = []
directml = ["ort/directml"]
coreml = ["ort/coreml"]

[profile.release]
lto = true
codegen-units = 1
```

### Build Commands

```bash
# Development
npm run tauri dev

# Production builds
npm run tauri build                    # Current platform
npm run tauri build -- --target x86_64-pc-windows-msvc
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri build -- --target x86_64-unknown-linux-gnu

# With GPU features
RUSTFLAGS="--cfg cuda" npm run tauri build  # NVIDIA
RUSTFLAGS="--cfg metal" npm run tauri build # Apple Silicon
```

---

## Development Phases

### Phase 1: Foundation (Weeks 1-2)
- [ ] Tauri + React project scaffolding
- [ ] Basic UI shell with navigation
- [ ] Audio capture (mic only, single platform - Windows)
- [ ] Audio waveform visualization
- [ ] SQLite database setup

### Phase 2: Core Recording (Weeks 3-4)
- [ ] System audio loopback (Windows WASAPI)
- [ ] Dual-stream recording with speaker labels
- [ ] VAD integration (Silero V5)
- [ ] Audio denoising (nnnoiseless)
- [ ] WAV file saving
- [ ] Recording controls (start/stop/pause)
- [ ] Meeting library basic view

### Phase 3: Transcription (Weeks 5-6)
- [ ] transcribe-rs integration (Parakeet default)
- [ ] Model download manager
- [ ] Post-meeting transcription pipeline
- [ ] Transcript storage and display
- [ ] Meeting detail view with transcript

### Phase 4: Intelligence (Weeks 7-8)
- [ ] llama-cpp-2 integration
- [ ] Summary generation
- [ ] EmbeddingGemma + ort integration
- [ ] LanceDB vector storage
- [ ] Embedding generation for transcripts

### Phase 5: RAG & Polish (Weeks 9-10)
- [ ] Vector search implementation
- [ ] RAG chat interface
- [ ] Notes editing and embedding
- [ ] Search functionality (FTS + vector)
- [ ] Settings UI
- [ ] Opus archival service

### Phase 6: Cross-Platform (Weeks 11-12)
- [ ] macOS audio capture (ScreenCaptureKit)
- [ ] Linux audio capture (PipeWire)
- [ ] Platform-specific builds and testing
- [ ] Installer/packaging

### Phase 7: V2 Features (Future)
- [ ] Live transcription during recording
- [ ] Speaker diarization (pyannote via ONNX)
- [ ] Custom vocabulary/names
- [ ] Export options (PDF, Markdown)
- [ ] Meeting calendar integration

---

## Development Stories

### Epic 1: Audio Foundation

#### Story 1.1: Basic Audio Capture
**Points:** 5  
**Acceptance Criteria:**
- [ ] Capture audio from default microphone
- [ ] Store in ring buffer (30 second capacity)
- [ ] Support 16kHz sample rate
- [ ] Emit waveform data every 50ms

#### Story 1.2: System Audio Capture (Windows)
**Points:** 8  
**Acceptance Criteria:**
- [ ] Capture system audio output via WASAPI loopback
- [ ] Label as "others" channel
- [ ] Handle audio device changes gracefully
- [ ] Synchronize with mic capture timing

#### Story 1.3: WAV Recording
**Points:** 3  
**Acceptance Criteria:**
- [ ] Create meeting directory on recording start
- [ ] Save you.wav and others.wav
- [ ] Handle disk full errors gracefully
- [ ] Support pause/resume

### Epic 2: Audio Preprocessing

#### Story 2.1: VAD Integration
**Points:** 5  
**Acceptance Criteria:**
- [ ] Detect speech/silence boundaries
- [ ] Return timestamps in milliseconds
- [ ] Configurable threshold and padding
- [ ] Process in real-time without blocking

#### Story 2.2: Audio Denoising
**Points:** 5  
**Acceptance Criteria:**
- [ ] Reduce background noise
- [ ] Preserve speech quality
- [ ] Toggle-able in settings
- [ ] Handle resampling (48kHz requirement)

#### Story 2.3: Complete Preprocessing Pipeline
**Points:** 8  
**Acceptance Criteria:**
- [ ] Resample → Denoise → VAD in single pass
- [ ] Output 16kHz audio ready for transcription
- [ ] Extract speech segments with timestamps
- [ ] Efficient memory usage

### Epic 3: Transcription

#### Story 3.1: transcribe-rs Integration
**Points:** 5  
**Acceptance Criteria:**
- [ ] Transcribe audio files with Parakeet (default)
- [ ] Return timestamped segments
- [ ] Support engine switching in settings
- [ ] GPU acceleration on all platforms

#### Story 3.2: Model Download & Management
**Points:** 5  
**Acceptance Criteria:**
- [ ] Download Parakeet V3 INT8 (~450MB) as default
- [ ] Optional download of Whisper/Moonshine
- [ ] Progress UI with cancel support
- [ ] Verify model integrity

#### Story 3.3: Speaker Assignment
**Points:** 3  
**Acceptance Criteria:**
- [ ] Label mic audio as "you"
- [ ] Label system audio as "others"
- [ ] Merge overlapping segments intelligently
- [ ] Handle crosstalk

### Epic 4: Storage & Archival

#### Story 4.1: SQLite Setup
**Points:** 3  
**Acceptance Criteria:**
- [ ] Create database on first launch
- [ ] Run migrations automatically
- [ ] Full-text search working
- [ ] Proper foreign key constraints

#### Story 4.2: Opus Archival
**Points:** 5  
**Acceptance Criteria:**
- [ ] Convert WAV → Opus after configurable delay
- [ ] Verify conversion before deleting WAV
- [ ] Background processing
- [ ] Space savings reported to user

### Epic 5: UI Integration

#### Story 5.1: Recording View
**Points:** 8  
**Acceptance Criteria:**
- [ ] Dual waveform visualization
- [ ] Recording timer
- [ ] Start/stop/pause controls
- [ ] Audio level meters

#### Story 5.2: Meeting Library
**Points:** 5  
**Acceptance Criteria:**
- [ ] Timeline view grouped by date
- [ ] Full-text search
- [ ] Filter by tags
- [ ] Sort options

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **macOS ScreenCaptureKit** | Medium | High | Start with Windows, use cidre crate, test early |
| **Linux audio (PipeWire)** | Medium | Medium | Provide PulseAudio fallback, clear setup docs |
| **transcribe-rs GPU issues** | Medium | High | Multiple engine fallbacks (Parakeet→Whisper→Moonshine) |
| **Silero VAD accuracy** | Low | Medium | Tune thresholds, add user adjustment |
| **FFmpeg deployment** | Medium | Medium | Bundle static binary, fallback to WAV only |
| **llama-cpp-2 API changes** | Medium | Medium | Pin to specific version, wrap in abstraction |
| **LanceDB Rust API stability** | Medium | Medium | Pin version, minimal surface area |
| **Model download sizes** | Low | Low | Clear download UI, progress indicators |
| **Cross-platform builds** | Medium | Medium | GitHub Actions with platform-specific jobs |
| **Memory usage** | Medium | Medium | Ring buffers, streaming pipeline, profiling |

---

## Open Questions

1. **Audio format for archival**: Use Opus (5-8x smaller) or keep WAV for simplicity?  
   *Decision: Opus with FFmpeg, configurable delay before archival*

2. **Live transcription approach**: 
   - Stream to whisper in chunks (complex state management)
   - Or use faster-whisper approach with VAD?

3. **Meeting detection**: Manual start/stop only, or auto-detect when conferencing apps launch?

4. **Speaker diarization (V2)**: 
   - Build ONNX bindings for pyannote models?
   - Or find alternative Rust-native approach?

5. **Backup/sync**: Local-only, or optional encrypted cloud sync in future?

6. **Licensing**: 
   - LLM models have various licenses (Llama community license, etc.)
   - EmbeddingGemma uses Gemma license
   - Parakeet uses CC-BY-4.0
   - Need to display appropriate notices

---

## References

### Key Repositories
- [transcribe-rs](https://github.com/cjpais/transcribe-rs) - **Our transcription engine** (Parakeet/Whisper/Moonshine)
- [screenpipe](https://github.com/mediar-ai/screenpipe) - Audio capture patterns
- [meeting-minutes](https://github.com/Zackriya-Solutions/meeting-minutes) - Tauri + Whisper integration
- [voice_activity_detector](https://github.com/nkeenan38/voice_activity_detector) - Silero VAD Rust
- [nnnoiseless](https://github.com/jneem/nnnoiseless) - RNNoise Rust port
- [kalosm-sound](https://github.com/floneum/floneum) - Audio pipeline patterns

### Documentation
- [Tauri v2](https://tauri.app/v2/)
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- [llama.cpp](https://github.com/ggerganov/llama.cpp)
- [ONNX Runtime Rust](https://github.com/pykeio/ort)
- [LanceDB](https://lancedb.github.io/lancedb/)

---

*Document Version: 2.1 (Combined)*  
*Last Updated: January 2026*  
*Based on v1 architecture design + v2 open-source research*
