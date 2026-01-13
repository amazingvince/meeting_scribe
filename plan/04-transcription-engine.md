# 04 - Transcription Engine

> **Goal:** Integrate transcribe-rs for multi-engine speech-to-text with Parakeet as default  
> **Time Estimate:** 4-5 days  
> **Prerequisites:** [03-audio-preprocessing.md](./03-audio-preprocessing.md) completed

---

## Table of Contents

1. [Overview](#overview)
2. [Engine Comparison](#engine-comparison)
3. [Dependencies](#dependencies)
4. [Model Downloads](#model-downloads)
5. [Transcription Service](#transcription-service)
6. [Speaker Labeling](#speaker-labeling)
7. [Processing Pipeline](#processing-pipeline)
8. [Tauri Integration](#tauri-integration)
9. [Frontend Components](#frontend-components)
10. [Performance Optimization](#performance-optimization)
11. [Testing](#testing)
12. [Troubleshooting](#troubleshooting)
13. [Acceptance Criteria](#acceptance-criteria)

---

## Overview

The transcription engine is the core intelligence of Meeting Scribe. We use **transcribe-rs**, a unified Rust library that provides a consistent API across multiple speech-to-text engines:

```
                    ┌─────────────────────────────────────┐
                    │          transcribe-rs              │
                    │    Unified Transcription API        │
                    └──────────────┬──────────────────────┘
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
         ▼                         ▼                         ▼
┌─────────────────┐   ┌─────────────────────┐   ┌─────────────────┐
│   Parakeet V3   │   │   Whisper Large V3  │   │   Moonshine     │
│   (Default)     │   │      Turbo          │   │                 │
│                 │   │                     │   │                 │
│ • 4x faster     │   │ • 100+ languages    │   │ • 8 languages   │
│ • English-opt   │   │ • Most accurate     │   │ • Compact       │
│ • 450MB         │   │ • 1.6GB             │   │ • 300MB         │
└─────────────────┘   └─────────────────────┘   └─────────────────┘
```

### Why transcribe-rs?

| Benefit | Description |
|---------|-------------|
| **Unified API** | Same code works with any engine - swap in settings |
| **Battle-tested** | Extracted from Handy, a production meeting app |
| **GPU Support** | Metal, Vulkan, CUDA all supported |
| **Active Development** | Regular updates from cjpais |

**Repository:** https://github.com/cjpais/transcribe-rs

---

## Engine Comparison

### Performance Benchmarks

| Engine | 1-Hour Audio | Speed Factor | VRAM (GPU) | RAM (CPU) |
|--------|--------------|--------------|------------|-----------|
| **Parakeet V3** | ~3 min | 20x realtime | 2GB | 4GB |
| **Whisper large-v3-turbo** | ~12 min | 5x realtime | 4GB | 8GB |
| **Moonshine** | ~6 min | 10x realtime | 1.5GB | 3GB |

### Feature Comparison

| Feature | Parakeet | Whisper | Moonshine |
|---------|----------|---------|-----------|
| **Languages** | English only | 100+ | 8 major |
| **Word timestamps** | ✅ | ✅ | ✅ |
| **Sentence timestamps** | ✅ | ✅ | ✅ |
| **Punctuation** | ✅ | ✅ | ✅ |
| **GPU Acceleration** | ✅ | ✅ | ✅ |

### Recommendation Matrix

| Use Case | Recommended Engine |
|----------|-------------------|
| English meetings (default) | **Parakeet** |
| International meetings | **Whisper** |
| Limited hardware | **Moonshine** |
| Maximum accuracy | **Whisper** |
| Fastest processing | **Parakeet** |

---

## Dependencies

### Update Cargo.toml

```toml
[dependencies]
# Transcription - unified interface over multiple engines
# Note: This is a git dependency - check for releases
transcribe-rs = { 
    git = "https://github.com/cjpais/transcribe-rs",
    features = ["parakeet", "whisper", "moonshine"]
}

# Additional dependencies for transcription
ort = { version = "2", features = ["load-dynamic"] }  # ONNX Runtime for Parakeet
```

### Feature Flags

```toml
[features]
default = ["parakeet"]
parakeet = ["transcribe-rs/parakeet"]
whisper = ["transcribe-rs/whisper"]
moonshine = ["transcribe-rs/moonshine"]
all-engines = ["parakeet", "whisper", "moonshine"]
```

### Platform-Specific Setup

#### Windows (CUDA)

```powershell
# Install CUDA Toolkit 12.x from NVIDIA
# Set environment variable
$env:CUDA_PATH = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.0"
```

#### macOS (Metal)

Metal support is automatic on Apple Silicon and Intel Macs with AMD GPUs.

#### Linux (Vulkan/CUDA)

```bash
# For Vulkan
sudo apt install libvulkan-dev vulkan-tools

# For CUDA
sudo apt install nvidia-cuda-toolkit
```

**References:**
- [ONNX Runtime GPU](https://onnxruntime.ai/docs/execution-providers/)
- [whisper.cpp GPU support](https://github.com/ggerganov/whisper.cpp#nvidia-gpu-support)

---

## Model Downloads

### Model Registry

Create `src-tauri/src/models/registry.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Information about a downloadable model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub model_type: ModelType,
    pub size_bytes: u64,
    pub download_url: String,
    pub checksum_sha256: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelType {
    Transcription,
    Embedding,
    LLM,
    VAD,
}

/// Supported transcription backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TranscriptionBackend {
    #[default]
    Parakeet,
    Whisper,
    Moonshine,
}

impl TranscriptionBackend {
    pub fn model_info(&self) -> ModelInfo {
        match self {
            Self::Parakeet => ModelInfo {
                id: "parakeet-v3".to_string(),
                name: "Parakeet V3".to_string(),
                model_type: ModelType::Transcription,
                size_bytes: 450 * 1024 * 1024, // 450MB
                download_url: "https://cdn.handy.ai/models/parakeet-v3-int8.tar.gz".to_string(),
                checksum_sha256: None, // Add actual checksum
                description: "NVIDIA's fast English transcription model".to_string(),
            },
            Self::Whisper => ModelInfo {
                id: "whisper-large-v3-turbo".to_string(),
                name: "Whisper Large V3 Turbo".to_string(),
                model_type: ModelType::Transcription,
                size_bytes: 1600 * 1024 * 1024, // 1.6GB
                download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".to_string(),
                checksum_sha256: None,
                description: "OpenAI's multilingual transcription model".to_string(),
            },
            Self::Moonshine => ModelInfo {
                id: "moonshine-base".to_string(),
                name: "Moonshine Base".to_string(),
                model_type: ModelType::Transcription,
                size_bytes: 300 * 1024 * 1024, // 300MB
                download_url: "https://huggingface.co/UsefulSensors/moonshine/resolve/main/moonshine-base-onnx.tar.gz".to_string(),
                checksum_sha256: None,
                description: "UsefulSensors multilingual model".to_string(),
            },
        }
    }
    
    pub fn model_filename(&self) -> &'static str {
        match self {
            Self::Parakeet => "parakeet-v3",
            Self::Whisper => "ggml-large-v3-turbo.bin",
            Self::Moonshine => "moonshine-base",
        }
    }
}
```

### Model Downloader

Create `src-tauri/src/models/downloader.rs`:

```rust
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tauri::{AppHandle, Emitter};
use serde::Serialize;

use super::registry::{ModelInfo, TranscriptionBackend};

/// Progress update for model downloads
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub progress_percent: f32,
    pub status: DownloadStatus,
}

#[derive(Debug, Clone, Serialize)]
pub enum DownloadStatus {
    Starting,
    Downloading,
    Extracting,
    Complete,
    Error(String),
}

pub struct ModelDownloader {
    client: Client,
    models_dir: PathBuf,
}

impl ModelDownloader {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            client: Client::builder()
                .user_agent("meeting-scribe/1.0")
                .build()
                .expect("Failed to create HTTP client"),
            models_dir,
        }
    }
    
    /// Check if a model is already downloaded
    pub fn is_model_downloaded(&self, backend: TranscriptionBackend) -> bool {
        let model_path = self.get_model_path(backend);
        model_path.exists()
    }
    
    /// Get the local path for a model
    pub fn get_model_path(&self, backend: TranscriptionBackend) -> PathBuf {
        self.models_dir.join("transcription").join(backend.model_filename())
    }
    
    /// Download a transcription model with progress updates
    pub async fn download_transcription_model(
        &self,
        backend: TranscriptionBackend,
        app_handle: Option<&AppHandle>,
    ) -> Result<PathBuf> {
        let info = backend.model_info();
        let target_dir = self.models_dir.join("transcription");
        fs::create_dir_all(&target_dir).await?;
        
        let target_path = target_dir.join(backend.model_filename());
        
        // Skip if already downloaded
        if target_path.exists() {
            tracing::info!("Model already downloaded: {:?}", target_path);
            return Ok(target_path);
        }
        
        // Emit starting event
        if let Some(app) = app_handle {
            let _ = app.emit("model-download-progress", DownloadProgress {
                model_id: info.id.clone(),
                downloaded_bytes: 0,
                total_bytes: info.size_bytes,
                progress_percent: 0.0,
                status: DownloadStatus::Starting,
            });
        }
        
        // Download file
        let response = self.client
            .get(&info.download_url)
            .send()
            .await
            .context("Failed to start download")?;
        
        let total_size = response
            .content_length()
            .unwrap_or(info.size_bytes);
        
        // Determine if we need to extract (tar.gz)
        let is_archive = info.download_url.ends_with(".tar.gz") 
            || info.download_url.ends_with(".zip");
        
        let download_path = if is_archive {
            target_dir.join(format!("{}.download", backend.model_filename()))
        } else {
            target_path.clone()
        };
        
        // Stream download
        let mut file = File::create(&download_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            
            // Emit progress
            if let Some(app) = app_handle {
                let progress = (downloaded as f32 / total_size as f32) * 100.0;
                let _ = app.emit("model-download-progress", DownloadProgress {
                    model_id: info.id.clone(),
                    downloaded_bytes: downloaded,
                    total_bytes: total_size,
                    progress_percent: progress,
                    status: DownloadStatus::Downloading,
                });
            }
        }
        
        file.flush().await?;
        drop(file);
        
        // Extract if archive
        if is_archive {
            if let Some(app) = app_handle {
                let _ = app.emit("model-download-progress", DownloadProgress {
                    model_id: info.id.clone(),
                    downloaded_bytes: total_size,
                    total_bytes: total_size,
                    progress_percent: 100.0,
                    status: DownloadStatus::Extracting,
                });
            }
            
            self.extract_archive(&download_path, &target_dir).await?;
            fs::remove_file(&download_path).await?;
        }
        
        // Emit complete
        if let Some(app) = app_handle {
            let _ = app.emit("model-download-progress", DownloadProgress {
                model_id: info.id.clone(),
                downloaded_bytes: total_size,
                total_bytes: total_size,
                progress_percent: 100.0,
                status: DownloadStatus::Complete,
            });
        }
        
        tracing::info!("Model downloaded successfully: {:?}", target_path);
        Ok(target_path)
    }
    
    /// Extract a tar.gz or zip archive
    async fn extract_archive(&self, archive_path: &Path, target_dir: &Path) -> Result<()> {
        use std::process::Command;
        
        let archive_str = archive_path.to_string_lossy();
        let target_str = target_dir.to_string_lossy();
        
        if archive_path.extension().map(|e| e == "gz").unwrap_or(false) {
            // Use tar for .tar.gz
            #[cfg(unix)]
            {
                Command::new("tar")
                    .args(["-xzf", &archive_str, "-C", &target_str])
                    .output()
                    .context("Failed to extract tar.gz")?;
            }
            
            #[cfg(windows)]
            {
                // Use PowerShell on Windows
                Command::new("powershell")
                    .args([
                        "-Command",
                        &format!(
                            "tar -xzf '{}' -C '{}'",
                            archive_str, target_str
                        ),
                    ])
                    .output()
                    .context("Failed to extract tar.gz")?;
            }
        }
        
        Ok(())
    }
}
```

### Model Manager State

Create `src-tauri/src/models/mod.rs`:

```rust
pub mod registry;
pub mod downloader;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub use registry::{ModelInfo, ModelType, TranscriptionBackend};
pub use downloader::{ModelDownloader, DownloadProgress, DownloadStatus};

/// Global model manager state
pub struct ModelManager {
    pub downloader: ModelDownloader,
    pub status: RwLock<HashMap<String, ModelStatus>>,
}

#[derive(Debug, Clone)]
pub enum ModelStatus {
    NotDownloaded,
    Downloading { progress: f32 },
    Ready { path: PathBuf },
    Error(String),
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            downloader: ModelDownloader::new(models_dir),
            status: RwLock::new(HashMap::new()),
        }
    }
    
    /// Initialize status for all known models
    pub fn init_status(&self) {
        let mut status = self.status.write();
        
        for backend in [
            TranscriptionBackend::Parakeet,
            TranscriptionBackend::Whisper,
            TranscriptionBackend::Moonshine,
        ] {
            let info = backend.model_info();
            let is_ready = self.downloader.is_model_downloaded(backend);
            
            status.insert(
                info.id,
                if is_ready {
                    ModelStatus::Ready {
                        path: self.downloader.get_model_path(backend),
                    }
                } else {
                    ModelStatus::NotDownloaded
                },
            );
        }
    }
    
    /// Get status of a specific model
    pub fn get_status(&self, model_id: &str) -> Option<ModelStatus> {
        self.status.read().get(model_id).cloned()
    }
    
    /// Update status of a model
    pub fn set_status(&self, model_id: &str, new_status: ModelStatus) {
        self.status.write().insert(model_id.to_string(), new_status);
    }
}
```

---

## Transcription Service

Create `src-tauri/src/inference/transcription.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;

use transcribe_rs::{
    TranscriptionEngine, 
    TranscriptionResult, 
    TranscriptionOptions,
    parakeet::ParakeetEngine,
    whisper::WhisperEngine,
    moonshine::MoonshineEngine,
};

use crate::models::{TranscriptionBackend, ModelManager};

/// A segment of transcribed speech
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
    /// Transcribed text
    pub text: String,
    /// Speaker label
    pub speaker: Speaker,
    /// Confidence score (0.0 - 1.0)
    pub confidence: Option<f32>,
}

/// Speaker identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speaker {
    /// Audio from microphone (user)
    You,
    /// Audio from system/others
    Others,
    /// Unknown source
    Unknown,
}

/// Configuration for transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Which backend to use
    pub backend: TranscriptionBackend,
    /// Language code (e.g., "en", "es", "fr")
    pub language: String,
    /// Enable word-level timestamps
    pub word_timestamps: bool,
    /// Use GPU if available
    pub use_gpu: bool,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            backend: TranscriptionBackend::Parakeet,
            language: "en".to_string(),
            word_timestamps: false,
            use_gpu: true,
        }
    }
}

/// The main transcription service
pub struct TranscriptionService {
    config: RwLock<TranscriptionConfig>,
    engine: RwLock<Option<Box<dyn TranscriptionEngine + Send + Sync>>>,
    model_manager: Arc<ModelManager>,
}

impl TranscriptionService {
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            config: RwLock::new(TranscriptionConfig::default()),
            engine: RwLock::new(None),
            model_manager,
        }
    }
    
    /// Initialize the transcription engine
    pub fn initialize(&self, config: TranscriptionConfig) -> Result<()> {
        // Check if model is downloaded
        let model_path = self.model_manager
            .downloader
            .get_model_path(config.backend);
        
        if !model_path.exists() {
            anyhow::bail!(
                "Model not downloaded: {:?}. Please download it first.",
                config.backend
            );
        }
        
        // Create the appropriate engine
        let mut engine: Box<dyn TranscriptionEngine + Send + Sync> = match config.backend {
            TranscriptionBackend::Parakeet => {
                Box::new(ParakeetEngine::new())
            }
            TranscriptionBackend::Whisper => {
                Box::new(WhisperEngine::new())
            }
            TranscriptionBackend::Moonshine => {
                Box::new(MoonshineEngine::new())
            }
        };
        
        // Load the model
        engine.load_model(&model_path)
            .context("Failed to load transcription model")?;
        
        // Store engine and config
        *self.engine.write() = Some(engine);
        *self.config.write() = config;
        
        tracing::info!("Transcription engine initialized");
        Ok(())
    }
    
    /// Transcribe an audio file
    pub fn transcribe_file(&self, audio_path: &PathBuf) -> Result<Vec<TranscriptSegment>> {
        let engine_guard = self.engine.read();
        let engine = engine_guard.as_ref()
            .context("Transcription engine not initialized")?;
        
        let config = self.config.read();
        
        let options = TranscriptionOptions {
            language: Some(config.language.clone()),
            word_timestamps: Some(config.word_timestamps),
            ..Default::default()
        };
        
        let result = engine.transcribe_file(audio_path, Some(options))
            .context("Transcription failed")?;
        
        // Convert to our segment format
        let segments = self.convert_result(result);
        Ok(segments)
    }
    
    /// Transcribe audio data directly (16kHz mono f32)
    pub fn transcribe_audio(&self, samples: &[f32], sample_rate: u32) -> Result<Vec<TranscriptSegment>> {
        // For direct audio, we need to write to a temp file
        // transcribe-rs currently only supports file-based transcription
        let temp_path = std::env::temp_dir().join(format!(
            "meeting-scribe-{}.wav",
            uuid::Uuid::new_v4()
        ));
        
        // Write WAV file
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        
        let mut writer = hound::WavWriter::create(&temp_path, spec)?;
        for &sample in samples {
            let sample_i16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            writer.write_sample(sample_i16)?;
        }
        writer.finalize()?;
        
        // Transcribe
        let result = self.transcribe_file(&temp_path);
        
        // Clean up
        let _ = std::fs::remove_file(&temp_path);
        
        result
    }
    
    /// Convert transcribe-rs result to our segment format
    fn convert_result(&self, result: TranscriptionResult) -> Vec<TranscriptSegment> {
        result.segments
            .into_iter()
            .map(|seg| TranscriptSegment {
                start_ms: (seg.start * 1000.0) as u64,
                end_ms: (seg.end * 1000.0) as u64,
                text: seg.text.trim().to_string(),
                speaker: Speaker::Unknown, // Will be set later based on audio source
                confidence: seg.confidence,
            })
            .filter(|seg| !seg.text.is_empty())
            .collect()
    }
    
    /// Change the transcription backend (requires re-initialization)
    pub fn set_backend(&self, backend: TranscriptionBackend) -> Result<()> {
        let mut config = self.config.write().clone();
        config.backend = backend;
        drop(self.config.write()); // Release lock before init
        self.initialize(config)
    }
    
    /// Get current configuration
    pub fn get_config(&self) -> TranscriptionConfig {
        self.config.read().clone()
    }
    
    /// Check if engine is ready
    pub fn is_ready(&self) -> bool {
        self.engine.read().is_some()
    }
}
```

---

## Speaker Labeling

Speaker labeling assigns "You" or "Others" based on the audio source (microphone vs system audio).

Create `src-tauri/src/inference/speaker.rs`:

```rust
use serde::{Deserialize, Serialize};

use super::transcription::{TranscriptSegment, Speaker};

/// Configuration for speaker labeling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerConfig {
    /// Label for microphone audio
    pub mic_label: String,
    /// Label for system audio
    pub system_label: String,
}

impl Default for SpeakerConfig {
    fn default() -> Self {
        Self {
            mic_label: "You".to_string(),
            system_label: "Others".to_string(),
        }
    }
}

/// Merge transcripts from two audio sources with speaker labels
pub fn merge_transcripts(
    mic_segments: Vec<TranscriptSegment>,
    system_segments: Vec<TranscriptSegment>,
) -> Vec<TranscriptSegment> {
    // Label segments by source
    let mut all_segments: Vec<TranscriptSegment> = Vec::new();
    
    for mut seg in mic_segments {
        seg.speaker = Speaker::You;
        all_segments.push(seg);
    }
    
    for mut seg in system_segments {
        seg.speaker = Speaker::Others;
        all_segments.push(seg);
    }
    
    // Sort by start time
    all_segments.sort_by_key(|s| s.start_ms);
    
    // Merge overlapping segments (handle simultaneous speech)
    merge_overlapping_segments(all_segments)
}

/// Merge segments that overlap in time
fn merge_overlapping_segments(segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    if segments.is_empty() {
        return segments;
    }
    
    let mut result: Vec<TranscriptSegment> = Vec::new();
    let mut current = segments[0].clone();
    
    for segment in segments.into_iter().skip(1) {
        // Check for overlap (within 500ms)
        let overlap_threshold_ms = 500;
        
        if segment.start_ms <= current.end_ms + overlap_threshold_ms 
            && segment.speaker == current.speaker 
        {
            // Same speaker, extend current segment
            current.end_ms = current.end_ms.max(segment.end_ms);
            current.text = format!("{} {}", current.text, segment.text);
        } else {
            // Different speaker or no overlap, push current and start new
            result.push(current);
            current = segment;
        }
    }
    
    result.push(current);
    result
}

/// Format transcript for display
pub fn format_transcript(segments: &[TranscriptSegment]) -> String {
    let mut output = String::new();
    
    for segment in segments {
        let speaker = match segment.speaker {
            Speaker::You => "[You]",
            Speaker::Others => "[Others]",
            Speaker::Unknown => "[Speaker]",
        };
        
        let timestamp = format_timestamp(segment.start_ms);
        output.push_str(&format!("{} {} {}\n\n", timestamp, speaker, segment.text));
    }
    
    output
}

/// Format milliseconds as MM:SS or HH:MM:SS
fn format_timestamp(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_merge_transcripts() {
        let mic = vec![
            TranscriptSegment {
                start_ms: 0,
                end_ms: 5000,
                text: "Hello from mic".to_string(),
                speaker: Speaker::Unknown,
                confidence: Some(0.9),
            },
        ];
        
        let system = vec![
            TranscriptSegment {
                start_ms: 6000,
                end_ms: 10000,
                text: "Hello from system".to_string(),
                speaker: Speaker::Unknown,
                confidence: Some(0.85),
            },
        ];
        
        let merged = merge_transcripts(mic, system);
        
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].speaker, Speaker::You);
        assert_eq!(merged[1].speaker, Speaker::Others);
    }
    
    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "00:00");
        assert_eq!(format_timestamp(65000), "01:05");
        assert_eq!(format_timestamp(3661000), "01:01:01");
    }
}
```

---

## Processing Pipeline

Complete meeting processing pipeline that combines preprocessing and transcription.

Create `src-tauri/src/inference/pipeline.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::audio::preprocessing::{AudioPipeline, PipelineConfig, SpeechSegment};
use super::transcription::{TranscriptionService, TranscriptSegment, TranscriptionConfig};
use super::speaker::merge_transcripts;

/// Complete processing result for a meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    /// Meeting ID
    pub meeting_id: String,
    /// All transcript segments (merged and sorted)
    pub transcript: Vec<TranscriptSegment>,
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Speech ratio (0.0 - 1.0)
    pub speech_ratio: f32,
    /// Backend used
    pub backend: String,
}

/// Progress updates during processing
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub meeting_id: String,
    pub stage: ProcessingStage,
    pub progress: f32, // 0.0 - 100.0
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum ProcessingStage {
    Loading,
    Preprocessing,
    TranscribingMic,
    TranscribingSystem,
    Merging,
    Saving,
    Complete,
    Error,
}

/// The complete meeting processing pipeline
pub struct MeetingProcessor {
    audio_pipeline: AudioPipeline,
    transcription: Arc<TranscriptionService>,
}

impl MeetingProcessor {
    pub fn new(transcription: Arc<TranscriptionService>) -> Self {
        Self {
            audio_pipeline: AudioPipeline::new(PipelineConfig::default()),
            transcription,
        }
    }
    
    /// Process a meeting from recorded audio files
    pub async fn process_meeting(
        &self,
        meeting_id: String,
        mic_audio_path: Option<PathBuf>,
        system_audio_path: Option<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProcessingProgress>>,
    ) -> Result<ProcessingResult> {
        let start_time = std::time::Instant::now();
        
        // Helper to send progress
        let send_progress = |stage: ProcessingStage, progress: f32, message: &str| {
            if let Some(tx) = &progress_tx {
                let _ = tx.try_send(ProcessingProgress {
                    meeting_id: meeting_id.clone(),
                    stage,
                    progress,
                    message: message.to_string(),
                });
            }
        };
        
        send_progress(ProcessingStage::Loading, 0.0, "Loading audio files...");
        
        let mut mic_segments = Vec::new();
        let mut system_segments = Vec::new();
        let mut total_duration_ms = 0u64;
        let mut total_speech_ratio = 0.0f32;
        let mut source_count = 0;
        
        // Process microphone audio
        if let Some(mic_path) = mic_audio_path {
            send_progress(ProcessingStage::Preprocessing, 10.0, "Preprocessing microphone audio...");
            
            let mic_audio = load_wav(&mic_path)?;
            let mic_preprocessed = self.audio_pipeline.process(&mic_audio)?;
            
            total_duration_ms = mic_preprocessed.duration_ms;
            total_speech_ratio += mic_preprocessed.speech_ratio;
            source_count += 1;
            
            send_progress(ProcessingStage::TranscribingMic, 30.0, "Transcribing microphone...");
            
            // Extract speech segments and transcribe each
            if !mic_preprocessed.speech_segments.is_empty() {
                let speech_audio = extract_speech_audio(
                    &mic_preprocessed.denoised_audio,
                    &mic_preprocessed.speech_segments,
                    16000,
                );
                
                mic_segments = self.transcription.transcribe_audio(&speech_audio, 16000)?;
                
                // Adjust timestamps based on speech segments
                adjust_segment_timestamps(&mut mic_segments, &mic_preprocessed.speech_segments);
            }
        }
        
        // Process system audio
        if let Some(system_path) = system_audio_path {
            send_progress(ProcessingStage::Preprocessing, 50.0, "Preprocessing system audio...");
            
            let system_audio = load_wav(&system_path)?;
            let system_preprocessed = self.audio_pipeline.process(&system_audio)?;
            
            if total_duration_ms == 0 {
                total_duration_ms = system_preprocessed.duration_ms;
            }
            total_speech_ratio += system_preprocessed.speech_ratio;
            source_count += 1;
            
            send_progress(ProcessingStage::TranscribingSystem, 70.0, "Transcribing system audio...");
            
            if !system_preprocessed.speech_segments.is_empty() {
                let speech_audio = extract_speech_audio(
                    &system_preprocessed.denoised_audio,
                    &system_preprocessed.speech_segments,
                    16000,
                );
                
                system_segments = self.transcription.transcribe_audio(&speech_audio, 16000)?;
                adjust_segment_timestamps(&mut system_segments, &system_preprocessed.speech_segments);
            }
        }
        
        // Merge transcripts
        send_progress(ProcessingStage::Merging, 90.0, "Merging transcripts...");
        let transcript = merge_transcripts(mic_segments, system_segments);
        
        let processing_time_ms = start_time.elapsed().as_millis() as u64;
        let avg_speech_ratio = if source_count > 0 {
            total_speech_ratio / source_count as f32
        } else {
            0.0
        };
        
        send_progress(ProcessingStage::Complete, 100.0, "Processing complete!");
        
        Ok(ProcessingResult {
            meeting_id,
            transcript,
            duration_ms: total_duration_ms,
            processing_time_ms,
            speech_ratio: avg_speech_ratio,
            backend: format!("{:?}", self.transcription.get_config().backend),
        })
    }
}

/// Load WAV file to f32 samples
fn load_wav(path: &PathBuf) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .context("Failed to open WAV file")?;
    
    let spec = reader.spec();
    
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>()
                .map(|s| s.unwrap_or(0.0))
                .collect()
        }
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / max_val)
                .collect()
        }
    };
    
    Ok(samples)
}

/// Extract speech segments from audio
fn extract_speech_audio(
    audio: &[f32],
    segments: &[SpeechSegment],
    sample_rate: u32,
) -> Vec<f32> {
    let mut result = Vec::new();
    
    for segment in segments {
        let start_sample = (segment.start_ms as f64 * sample_rate as f64 / 1000.0) as usize;
        let end_sample = (segment.end_ms as f64 * sample_rate as f64 / 1000.0) as usize;
        
        if start_sample < audio.len() {
            let end = end_sample.min(audio.len());
            result.extend_from_slice(&audio[start_sample..end]);
        }
    }
    
    result
}

/// Adjust transcript timestamps based on speech segments
fn adjust_segment_timestamps(
    transcript: &mut [TranscriptSegment],
    speech_segments: &[SpeechSegment],
) {
    if speech_segments.is_empty() || transcript.is_empty() {
        return;
    }
    
    // Build a mapping from concatenated audio time to original time
    let mut time_offset = 0u64;
    let mut mappings: Vec<(u64, u64, u64)> = Vec::new(); // (concat_start, concat_end, original_start)
    
    for segment in speech_segments {
        let duration = segment.end_ms - segment.start_ms;
        mappings.push((time_offset, time_offset + duration, segment.start_ms));
        time_offset += duration;
    }
    
    // Adjust each transcript segment
    for seg in transcript.iter_mut() {
        // Find which speech segment this falls into
        for (concat_start, concat_end, original_start) in &mappings {
            if seg.start_ms >= *concat_start && seg.start_ms < *concat_end {
                let offset_in_segment = seg.start_ms - concat_start;
                seg.start_ms = original_start + offset_in_segment;
                
                let duration = seg.end_ms - seg.start_ms;
                seg.end_ms = seg.start_ms + duration;
                break;
            }
        }
    }
}
```

---

## Tauri Integration

Create `src-tauri/src/commands/transcription.rs`:

```rust
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

use crate::models::{ModelManager, TranscriptionBackend};
use crate::inference::transcription::{TranscriptionService, TranscriptionConfig, TranscriptSegment};
use crate::inference::pipeline::{MeetingProcessor, ProcessingResult, ProcessingProgress};
use crate::storage::Database;

/// Download a transcription model
#[tauri::command]
pub async fn download_transcription_model(
    backend: TranscriptionBackend,
    model_manager: State<'_, Arc<ModelManager>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let path = model_manager
        .downloader
        .download_transcription_model(backend, Some(&app_handle))
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(path.to_string_lossy().to_string())
}

/// Get status of all transcription models
#[tauri::command]
pub fn get_model_status(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelStatusInfo>, String> {
    let mut result = Vec::new();
    
    for backend in [
        TranscriptionBackend::Parakeet,
        TranscriptionBackend::Whisper,
        TranscriptionBackend::Moonshine,
    ] {
        let info = backend.model_info();
        let is_downloaded = model_manager.downloader.is_model_downloaded(backend);
        
        result.push(ModelStatusInfo {
            id: info.id,
            name: info.name,
            size_bytes: info.size_bytes,
            is_downloaded,
            description: info.description,
        });
    }
    
    Ok(result)
}

#[derive(serde::Serialize)]
pub struct ModelStatusInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub is_downloaded: bool,
    pub description: String,
}

/// Initialize the transcription engine
#[tauri::command]
pub async fn init_transcription(
    backend: TranscriptionBackend,
    transcription: State<'_, Arc<TranscriptionService>>,
) -> Result<(), String> {
    let config = TranscriptionConfig {
        backend,
        ..Default::default()
    };
    
    transcription.initialize(config).map_err(|e| e.to_string())
}

/// Process a meeting (preprocess + transcribe)
#[tauri::command]
pub async fn process_meeting(
    meeting_id: String,
    mic_audio_path: Option<String>,
    system_audio_path: Option<String>,
    transcription: State<'_, Arc<TranscriptionService>>,
    app_handle: AppHandle,
) -> Result<ProcessingResult, String> {
    // Create progress channel
    let (progress_tx, mut progress_rx) = mpsc::channel::<ProcessingProgress>(32);
    
    // Spawn progress forwarder
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app_handle_clone.emit("processing-progress", &progress);
        }
    });
    
    // Create processor
    let processor = MeetingProcessor::new(Arc::clone(&transcription));
    
    // Process
    let result = processor
        .process_meeting(
            meeting_id,
            mic_audio_path.map(std::path::PathBuf::from),
            system_audio_path.map(std::path::PathBuf::from),
            Some(progress_tx),
        )
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(result)
}

/// Transcribe a single audio file
#[tauri::command]
pub async fn transcribe_file(
    audio_path: String,
    transcription: State<'_, Arc<TranscriptionService>>,
) -> Result<Vec<TranscriptSegment>, String> {
    let path = std::path::PathBuf::from(audio_path);
    transcription.transcribe_file(&path).map_err(|e| e.to_string())
}

/// Get current transcription configuration
#[tauri::command]
pub fn get_transcription_config(
    transcription: State<'_, Arc<TranscriptionService>>,
) -> TranscriptionConfig {
    transcription.get_config()
}

/// Check if transcription is ready
#[tauri::command]
pub fn is_transcription_ready(
    transcription: State<'_, Arc<TranscriptionService>>,
) -> bool {
    transcription.is_ready()
}
```

Update `src-tauri/src/commands/mod.rs`:

```rust
pub mod recording;
pub mod transcription;

pub use recording::*;
pub use transcription::*;
```

### Register Commands

Update `src-tauri/src/lib.rs`:

```rust
mod audio;
mod inference;
mod models;
mod storage;
mod commands;

use std::sync::Arc;
use tauri::Manager;

use models::ModelManager;
use inference::transcription::TranscriptionService;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Get data directory
            let app_data_dir = app.path().app_data_dir()
                .expect("Failed to get app data dir");
            
            // Initialize model manager
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;
            
            let model_manager = Arc::new(ModelManager::new(models_dir));
            model_manager.init_status();
            app.manage(model_manager.clone());
            
            // Initialize transcription service
            let transcription = Arc::new(TranscriptionService::new(model_manager));
            app.manage(transcription);
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Recording commands
            commands::start_recording,
            commands::stop_recording,
            commands::get_recording_state,
            commands::list_audio_devices,
            // Transcription commands
            commands::download_transcription_model,
            commands::get_model_status,
            commands::init_transcription,
            commands::process_meeting,
            commands::transcribe_file,
            commands::get_transcription_config,
            commands::is_transcription_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## Frontend Components

### TypeScript Types

Create `src/types/transcription.ts`:

```typescript
export type TranscriptionBackend = 'Parakeet' | 'Whisper' | 'Moonshine';

export interface ModelStatusInfo {
  id: string;
  name: string;
  size_bytes: number;
  is_downloaded: boolean;
  description: string;
}

export interface TranscriptionConfig {
  backend: TranscriptionBackend;
  language: string;
  word_timestamps: boolean;
  use_gpu: boolean;
}

export interface TranscriptSegment {
  start_ms: number;
  end_ms: number;
  text: string;
  speaker: 'You' | 'Others' | 'Unknown';
  confidence?: number;
}

export interface ProcessingResult {
  meeting_id: string;
  transcript: TranscriptSegment[];
  duration_ms: number;
  processing_time_ms: number;
  speech_ratio: number;
  backend: string;
}

export interface ProcessingProgress {
  meeting_id: string;
  stage: string;
  progress: number;
  message: string;
}

export interface DownloadProgress {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  progress_percent: number;
  status: string;
}
```

### Model Manager Component

Create `src/components/Settings/ModelManager.tsx`:

```tsx
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ModelStatusInfo, DownloadProgress, TranscriptionBackend } from '../../types/transcription';

export function ModelManager() {
  const [models, setModels] = useState<ModelStatusInfo[]>([]);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [progress, setProgress] = useState<number>(0);
  const [selectedBackend, setSelectedBackend] = useState<TranscriptionBackend>('Parakeet');

  useEffect(() => {
    loadModels();
    
    // Listen for download progress
    const unlisten = listen<DownloadProgress>('model-download-progress', (event) => {
      setProgress(event.payload.progress_percent);
      
      if (event.payload.status === 'Complete') {
        setDownloading(null);
        loadModels();
      }
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  async function loadModels() {
    try {
      const status = await invoke<ModelStatusInfo[]>('get_model_status');
      setModels(status);
    } catch (error) {
      console.error('Failed to load model status:', error);
    }
  }

  async function downloadModel(backend: TranscriptionBackend) {
    setDownloading(backend);
    setProgress(0);
    
    try {
      await invoke('download_transcription_model', { backend });
    } catch (error) {
      console.error('Download failed:', error);
      setDownloading(null);
    }
  }

  async function initializeEngine() {
    try {
      await invoke('init_transcription', { backend: selectedBackend });
      alert('Transcription engine initialized!');
    } catch (error) {
      console.error('Failed to initialize:', error);
      alert(`Failed to initialize: ${error}`);
    }
  }

  function formatSize(bytes: number): string {
    const mb = bytes / (1024 * 1024);
    if (mb >= 1024) {
      return `${(mb / 1024).toFixed(1)} GB`;
    }
    return `${mb.toFixed(0)} MB`;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold">Transcription Models</h2>
      
      <div className="space-y-4">
        {models.map((model) => {
          const backend = model.id.includes('parakeet') ? 'Parakeet' 
            : model.id.includes('whisper') ? 'Whisper' 
            : 'Moonshine';
          
          return (
            <div 
              key={model.id}
              className={`p-4 rounded-lg border ${
                selectedBackend === backend 
                  ? 'border-blue-500 bg-blue-50' 
                  : 'border-gray-200'
              }`}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <input
                    type="radio"
                    name="backend"
                    checked={selectedBackend === backend}
                    onChange={() => setSelectedBackend(backend as TranscriptionBackend)}
                    disabled={!model.is_downloaded}
                    className="w-4 h-4"
                  />
                  <div>
                    <h3 className="font-medium">{model.name}</h3>
                    <p className="text-sm text-gray-500">
                      {model.description} • {formatSize(model.size_bytes)}
                    </p>
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  {model.is_downloaded ? (
                    <span className="px-2 py-1 text-sm text-green-700 bg-green-100 rounded">
                      ✓ Downloaded
                    </span>
                  ) : downloading === backend ? (
                    <div className="w-32">
                      <div className="h-2 bg-gray-200 rounded-full">
                        <div 
                          className="h-2 bg-blue-500 rounded-full transition-all"
                          style={{ width: `${progress}%` }}
                        />
                      </div>
                      <p className="text-xs text-center mt-1">{progress.toFixed(0)}%</p>
                    </div>
                  ) : (
                    <button
                      onClick={() => downloadModel(backend as TranscriptionBackend)}
                      className="px-3 py-1 text-sm text-blue-600 border border-blue-600 rounded hover:bg-blue-50"
                    >
                      Download
                    </button>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
      
      <button
        onClick={initializeEngine}
        disabled={!models.find(m => 
          m.id.includes(selectedBackend.toLowerCase()) && m.is_downloaded
        )}
        className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
      >
        Initialize {selectedBackend} Engine
      </button>
    </div>
  );
}
```

### Processing View Component

Create `src/components/Processing/ProcessingView.tsx`:

```tsx
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ProcessingProgress, ProcessingResult } from '../../types/transcription';

interface ProcessingViewProps {
  meetingId: string;
  micAudioPath?: string;
  systemAudioPath?: string;
  onComplete: (result: ProcessingResult) => void;
}

export function ProcessingView({ 
  meetingId, 
  micAudioPath, 
  systemAudioPath,
  onComplete 
}: ProcessingViewProps) {
  const [progress, setProgress] = useState<ProcessingProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Listen for progress updates
    const unlisten = listen<ProcessingProgress>('processing-progress', (event) => {
      if (event.payload.meeting_id === meetingId) {
        setProgress(event.payload);
      }
    });
    
    // Start processing
    startProcessing();
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, [meetingId]);

  async function startProcessing() {
    try {
      const result = await invoke<ProcessingResult>('process_meeting', {
        meetingId,
        micAudioPath,
        systemAudioPath,
      });
      
      onComplete(result);
    } catch (err) {
      setError(String(err));
    }
  }

  const stageLabels: Record<string, string> = {
    Loading: 'Loading audio files',
    Preprocessing: 'Preprocessing audio',
    TranscribingMic: 'Transcribing microphone',
    TranscribingSystem: 'Transcribing system audio',
    Merging: 'Merging transcripts',
    Saving: 'Saving results',
    Complete: 'Complete!',
    Error: 'Error',
  };

  if (error) {
    return (
      <div className="p-6 text-center">
        <div className="text-red-500 text-4xl mb-4">⚠️</div>
        <h2 className="text-xl font-semibold text-red-600">Processing Failed</h2>
        <p className="text-gray-600 mt-2">{error}</p>
        <button
          onClick={startProcessing}
          className="mt-4 px-4 py-2 bg-blue-600 text-white rounded"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="p-6">
      <h2 className="text-xl font-semibold text-center mb-6">
        Processing Meeting
      </h2>
      
      {progress && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">
              {stageLabels[progress.stage] || progress.stage}
            </span>
            <span className="text-sm text-gray-500">
              {progress.progress.toFixed(0)}%
            </span>
          </div>
          
          <div className="h-3 bg-gray-200 rounded-full overflow-hidden">
            <div 
              className="h-full bg-blue-500 rounded-full transition-all duration-300"
              style={{ width: `${progress.progress}%` }}
            />
          </div>
          
          <p className="text-sm text-gray-500 text-center">
            {progress.message}
          </p>
        </div>
      )}
      
      {/* Stage indicators */}
      <div className="mt-8 flex justify-between text-xs text-gray-400">
        {['Loading', 'Preprocessing', 'Transcribing', 'Complete'].map((stage, i) => (
          <div key={stage} className="flex items-center">
            <div className={`w-2 h-2 rounded-full ${
              progress && progress.progress > (i * 33) 
                ? 'bg-blue-500' 
                : 'bg-gray-300'
            }`} />
            <span className="ml-1">{stage}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
```

### Transcript Display Component

Create `src/components/Meeting/TranscriptView.tsx`:

```tsx
import { useState } from 'react';
import { TranscriptSegment } from '../../types/transcription';

interface TranscriptViewProps {
  segments: TranscriptSegment[];
  onSegmentClick?: (segment: TranscriptSegment) => void;
}

export function TranscriptView({ segments, onSegmentClick }: TranscriptViewProps) {
  const [filter, setFilter] = useState<'all' | 'You' | 'Others'>('all');
  const [searchQuery, setSearchQuery] = useState('');

  const filteredSegments = segments.filter(seg => {
    if (filter !== 'all' && seg.speaker !== filter) return false;
    if (searchQuery && !seg.text.toLowerCase().includes(searchQuery.toLowerCase())) return false;
    return true;
  });

  function formatTime(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    
    if (hours > 0) {
      return `${hours}:${String(minutes % 60).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
    }
    return `${minutes}:${String(seconds % 60).padStart(2, '0')}`;
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center gap-4 p-3 border-b bg-gray-50">
        <input
          type="text"
          placeholder="Search transcript..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="flex-1 px-3 py-1.5 text-sm border rounded"
        />
        
        <div className="flex items-center gap-1">
          {(['all', 'You', 'Others'] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1 text-sm rounded ${
                filter === f 
                  ? 'bg-blue-600 text-white' 
                  : 'bg-white border hover:bg-gray-50'
              }`}
            >
              {f === 'all' ? 'All' : f}
            </button>
          ))}
        </div>
      </div>
      
      {/* Transcript */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {filteredSegments.length === 0 ? (
          <p className="text-center text-gray-500 py-8">
            {searchQuery ? 'No matching segments' : 'No transcript available'}
          </p>
        ) : (
          filteredSegments.map((segment, index) => (
            <div 
              key={index}
              onClick={() => onSegmentClick?.(segment)}
              className={`p-3 rounded-lg cursor-pointer transition-colors ${
                segment.speaker === 'You' 
                  ? 'bg-blue-50 hover:bg-blue-100' 
                  : 'bg-gray-50 hover:bg-gray-100'
              }`}
            >
              <div className="flex items-center gap-2 mb-1">
                <span className="text-xs font-mono text-gray-400">
                  {formatTime(segment.start_ms)}
                </span>
                <span className={`text-xs font-medium px-1.5 py-0.5 rounded ${
                  segment.speaker === 'You'
                    ? 'bg-blue-200 text-blue-800'
                    : 'bg-gray-200 text-gray-800'
                }`}>
                  {segment.speaker}
                </span>
                {segment.confidence !== undefined && (
                  <span className="text-xs text-gray-400">
                    {(segment.confidence * 100).toFixed(0)}%
                  </span>
                )}
              </div>
              <p className="text-sm">{segment.text}</p>
            </div>
          ))
        )}
      </div>
      
      {/* Stats */}
      <div className="p-2 border-t bg-gray-50 text-xs text-gray-500 flex justify-between">
        <span>{filteredSegments.length} segments</span>
        <span>
          {segments.filter(s => s.speaker === 'You').length} from you,{' '}
          {segments.filter(s => s.speaker === 'Others').length} from others
        </span>
      </div>
    </div>
  );
}
```

---

## Performance Optimization

### Batch Processing

For long meetings, process audio in chunks to provide better progress feedback:

```rust
/// Process audio in chunks for better progress reporting
pub async fn process_in_chunks(
    audio: &[f32],
    chunk_duration_sec: f32,
    sample_rate: u32,
) -> Vec<TranscriptSegment> {
    let chunk_samples = (chunk_duration_sec * sample_rate as f32) as usize;
    let chunks: Vec<&[f32]> = audio.chunks(chunk_samples).collect();
    
    let mut all_segments = Vec::new();
    let mut time_offset = 0u64;
    
    for chunk in chunks {
        let mut segments = transcribe_chunk(chunk)?;
        
        // Adjust timestamps
        for seg in &mut segments {
            seg.start_ms += time_offset;
            seg.end_ms += time_offset;
        }
        
        all_segments.extend(segments);
        time_offset += (chunk.len() as f64 / sample_rate as f64 * 1000.0) as u64;
    }
    
    all_segments
}
```

### GPU Detection

```rust
/// Detect available GPU acceleration
pub fn detect_gpu() -> GpuInfo {
    #[cfg(target_os = "macos")]
    {
        // Metal is always available on macOS
        GpuInfo {
            available: true,
            backend: "Metal".to_string(),
            device_name: "Apple GPU".to_string(),
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Check for CUDA first, then Vulkan
        if std::env::var("CUDA_PATH").is_ok() {
            GpuInfo {
                available: true,
                backend: "CUDA".to_string(),
                device_name: "NVIDIA GPU".to_string(),
            }
        } else {
            GpuInfo {
                available: true,
                backend: "Vulkan".to_string(),
                device_name: "GPU".to_string(),
            }
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Similar logic for Linux
        GpuInfo {
            available: true,
            backend: "Vulkan".to_string(),
            device_name: "GPU".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub available: bool,
    pub backend: String,
    pub device_name: String,
}
```

---

## Testing

### Unit Tests

Create `src-tauri/src/inference/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_segment_timestamp_format() {
        assert_eq!(format_timestamp(0), "00:00");
        assert_eq!(format_timestamp(65_000), "01:05");
        assert_eq!(format_timestamp(3_661_000), "01:01:01");
    }
    
    #[test]
    fn test_merge_transcripts_ordering() {
        let mic = vec![
            TranscriptSegment {
                start_ms: 5000,
                end_ms: 8000,
                text: "Hello".to_string(),
                speaker: Speaker::Unknown,
                confidence: None,
            },
        ];
        
        let system = vec![
            TranscriptSegment {
                start_ms: 1000,
                end_ms: 4000,
                text: "Hi there".to_string(),
                speaker: Speaker::Unknown,
                confidence: None,
            },
        ];
        
        let merged = merge_transcripts(mic, system);
        
        // System segment should come first (earlier start time)
        assert_eq!(merged[0].text, "Hi there");
        assert_eq!(merged[0].speaker, Speaker::Others);
        assert_eq!(merged[1].text, "Hello");
        assert_eq!(merged[1].speaker, Speaker::You);
    }
    
    #[tokio::test]
    async fn test_model_download_check() {
        let temp_dir = tempfile::tempdir().unwrap();
        let downloader = ModelDownloader::new(temp_dir.path().to_path_buf());
        
        // Should not be downloaded initially
        assert!(!downloader.is_model_downloaded(TranscriptionBackend::Parakeet));
    }
}
```

### Integration Test

```bash
# Test transcription with a sample audio file
cargo test --package meeting-scribe -- --nocapture integration_transcription

# Test with GPU
RUST_LOG=debug cargo run -- --test-transcription sample.wav
```

### Manual Testing Checklist

1. **Model Download**
   - [ ] Download progress shows in UI
   - [ ] Download can be cancelled
   - [ ] Resume works after interruption
   - [ ] Checksum validation passes

2. **Transcription**
   - [ ] Parakeet transcribes English correctly
   - [ ] Whisper handles multiple languages
   - [ ] Word timestamps are accurate (±500ms)
   - [ ] Long audio (>1 hour) processes without OOM

3. **Speaker Labels**
   - [ ] Mic audio labeled as "You"
   - [ ] System audio labeled as "Others"
   - [ ] Overlapping speech handled correctly

---

## Troubleshooting

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| `Model not found` | Model not downloaded | Run download command first |
| `CUDA error` | GPU driver mismatch | Update NVIDIA drivers |
| `Out of memory` | Audio too long | Process in chunks |
| `Slow transcription` | CPU fallback | Check GPU detection |
| `Empty transcript` | No speech detected | Check VAD settings |

### Debug Logging

```rust
// Enable detailed logging
RUST_LOG=meeting_scribe::inference=debug cargo run

// Check model loading
RUST_LOG=transcribe_rs=debug cargo run
```

### Performance Profiling

```rust
// Add timing instrumentation
use tracing::{instrument, info};

#[instrument(skip(audio))]
pub fn transcribe_with_timing(&self, audio: &[f32]) -> Result<Vec<TranscriptSegment>> {
    let start = std::time::Instant::now();
    let result = self.transcribe_audio(audio)?;
    info!(
        duration_ms = start.elapsed().as_millis(),
        segment_count = result.len(),
        "Transcription complete"
    );
    Ok(result)
}
```

---

## Acceptance Criteria

### Required

- [ ] Parakeet model downloads and loads successfully
- [ ] Transcription produces text output from WAV files
- [ ] Speaker labels correctly identify mic vs system audio
- [ ] Processing progress updates shown in UI
- [ ] Transcription completes for 1-hour meeting in <5 minutes (with GPU)

### Nice to Have

- [ ] Multiple engine support (Whisper, Moonshine)
- [ ] GPU acceleration working on Windows/macOS/Linux
- [ ] Word-level timestamps available
- [ ] Confidence scores displayed

---

## Next Steps

After completing the transcription engine:

1. **[05-storage-layer.md](./05-storage-layer.md)** - Store transcripts in SQLite + LanceDB
2. **[06-embedding-engine.md](./06-embedding-engine.md)** - Generate embeddings for RAG
3. **[07-llm-engine.md](./07-llm-engine.md)** - Summarization and chat

---

## References

### Libraries
- [transcribe-rs](https://github.com/cjpais/transcribe-rs) - Unified transcription API
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) - C++ Whisper implementation
- [ONNX Runtime](https://onnxruntime.ai/) - Cross-platform ML inference

### Models
- [Parakeet](https://huggingface.co/nvidia/parakeet-tdt-1.1b) - NVIDIA's fast ASR
- [Whisper Large V3 Turbo](https://huggingface.co/openai/whisper-large-v3-turbo) - OpenAI model
- [Moonshine](https://huggingface.co/UsefulSensors/moonshine) - Efficient multilingual

### Guides
- [Tauri State Management](https://tauri.app/v1/guides/features/command/#accessing-managed-state)
- [Rust Async Patterns](https://rust-lang.github.io/async-book/)
- [GPU Acceleration Guide](https://github.com/ggerganov/whisper.cpp#gpu-support)
