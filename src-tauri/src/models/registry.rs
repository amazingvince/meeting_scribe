//! Model registry with metadata and download URLs
//!
//! Contains information about available models for transcription, embedding, and LLM.

use serde::{Deserialize, Serialize};

/// Types of models supported by the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelType {
    /// Speech-to-text transcription models
    Transcription,
    /// Text embedding models for semantic search
    Embedding,
    /// Large language models for summarization
    LLM,
    /// Voice activity detection models
    VAD,
}

/// Transcription engine backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TranscriptionBackend {
    /// NVIDIA Parakeet (default, best performance)
    #[default]
    Parakeet,
    /// OpenAI Whisper (wider language support)
    Whisper,
    /// UsefulSensors Moonshine (lightweight)
    Moonshine,
}

impl TranscriptionBackend {
    /// Get all available backends
    pub fn all() -> &'static [TranscriptionBackend] {
        &[
            TranscriptionBackend::Parakeet,
            TranscriptionBackend::Whisper,
            TranscriptionBackend::Moonshine,
        ]
    }

    /// Get the model info for this backend
    pub fn model_info(&self) -> ModelInfo {
        match self {
            TranscriptionBackend::Parakeet => ModelInfo {
                id: "parakeet-tdt-0.6b-v3-int8".to_string(),
                name: "Parakeet TDT 0.6B v3 (Int8)".to_string(),
                model_type: ModelType::Transcription,
                size_bytes: 650_000_000, // ~650MB compressed
                download_url: "https://blob.handy.computer/parakeet-v3-int8.tar.gz".to_string(),
                description: "NVIDIA's Parakeet model for fast, accurate transcription. Int8 quantized for better performance.".to_string(),
                is_archive: true,
                archive_format: Some(ArchiveFormat::TarGz),
                extracted_dir_name: Some("parakeet-tdt-0.6b-v3-int8".to_string()),
            },
            TranscriptionBackend::Whisper => ModelInfo {
                id: "whisper-medium-q4_1".to_string(),
                name: "Whisper Medium (Q4_1)".to_string(),
                model_type: ModelType::Transcription,
                size_bytes: 500_000_000, // ~500MB
                download_url: "https://blob.handy.computer/whisper-medium-q4_1.bin".to_string(),
                description: "OpenAI Whisper model, quantized for efficiency. Good multi-language support.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: None,
            },
            TranscriptionBackend::Moonshine => ModelInfo {
                id: "moonshine-tiny".to_string(),
                name: "Moonshine Tiny".to_string(),
                model_type: ModelType::Transcription,
                size_bytes: 50_000_000, // ~50MB total for all files
                download_url: "https://huggingface.co/UsefulSensors/moonshine/resolve/main/onnx/merged/tiny".to_string(),
                description: "Lightweight model for fast transcription. English only.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: Some("moonshine-tiny".to_string()),
            },
        }
    }

    /// Get the directory name where the model is stored
    pub fn model_dir_name(&self) -> &'static str {
        match self {
            TranscriptionBackend::Parakeet => "parakeet-tdt-0.6b-v3-int8",
            TranscriptionBackend::Whisper => "whisper-medium-q4_1",
            TranscriptionBackend::Moonshine => "moonshine-tiny",
        }
    }
}

impl std::fmt::Display for TranscriptionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranscriptionBackend::Parakeet => write!(f, "Parakeet"),
            TranscriptionBackend::Whisper => write!(f, "Whisper"),
            TranscriptionBackend::Moonshine => write!(f, "Moonshine"),
        }
    }
}

/// Archive format for compressed model downloads
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    /// .tar.gz format
    TarGz,
    /// .zip format
    Zip,
    /// .tar.bz2 format
    TarBz2,
}

/// Information about a downloadable model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique identifier for the model
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Type of model
    pub model_type: ModelType,
    /// Size in bytes (approximate, for progress display)
    pub size_bytes: u64,
    /// URL to download the model from
    pub download_url: String,
    /// Description of the model
    pub description: String,
    /// Whether the download is an archive that needs extraction
    pub is_archive: bool,
    /// Archive format if is_archive is true
    pub archive_format: Option<ArchiveFormat>,
    /// Name of the directory after extraction (if different from archive name)
    pub extracted_dir_name: Option<String>,
}

impl ModelInfo {
    /// Get the size formatted as a human-readable string
    pub fn size_formatted(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if self.size_bytes >= GB {
            format!("{:.1} GB", self.size_bytes as f64 / GB as f64)
        } else if self.size_bytes >= MB {
            format!("{:.0} MB", self.size_bytes as f64 / MB as f64)
        } else if self.size_bytes >= KB {
            format!("{:.0} KB", self.size_bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", self.size_bytes)
        }
    }
}

/// Moonshine model files that need to be downloaded separately
pub struct MoonshineFiles;

impl MoonshineFiles {
    /// Base URL for moonshine model files
    pub const BASE_URL: &'static str =
        "https://huggingface.co/UsefulSensors/moonshine/resolve/main/onnx/merged/tiny";

    /// Required files for the moonshine model
    pub fn required_files() -> Vec<(&'static str, &'static str)> {
        vec![
            ("encoder_model.onnx", "encoder_model.onnx"),
            ("decoder_model_merged.onnx", "decoder_model_merged.onnx"),
            ("tokenizer.json", "tokenizer.json"),
        ]
    }

    /// Get full download URLs for all moonshine files
    pub fn download_urls() -> Vec<(String, &'static str)> {
        Self::required_files()
            .into_iter()
            .map(|(remote, local)| (format!("{}/{}", Self::BASE_URL, remote), local))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parakeet_model_info() {
        let info = TranscriptionBackend::Parakeet.model_info();
        assert_eq!(info.id, "parakeet-tdt-0.6b-v3-int8");
        assert!(info.is_archive);
        assert_eq!(info.archive_format, Some(ArchiveFormat::TarGz));
    }

    #[test]
    fn test_size_formatting() {
        let info = ModelInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            model_type: ModelType::Transcription,
            size_bytes: 650_000_000,
            download_url: "http://example.com".to_string(),
            description: "Test model".to_string(),
            is_archive: false,
            archive_format: None,
            extracted_dir_name: None,
        };

        assert!(info.size_formatted().contains("MB"));
    }

    #[test]
    fn test_all_backends() {
        let backends = TranscriptionBackend::all();
        assert_eq!(backends.len(), 3);
    }
}
