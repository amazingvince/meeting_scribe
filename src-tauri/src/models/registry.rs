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

/// Embedding model variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EmbeddingModel {
    /// EmbeddingGemma 300M Q8 (default, best balance)
    #[default]
    EmbeddingGemmaQ8,
    /// EmbeddingGemma 300M FP32 (highest quality)
    EmbeddingGemmaFP32,
    /// EmbeddingGemma 300M Q4 (fastest, smallest)
    EmbeddingGemmaQ4,
}

impl EmbeddingModel {
    /// Get all available embedding models
    pub fn all() -> &'static [EmbeddingModel] {
        &[
            EmbeddingModel::EmbeddingGemmaQ8,
            EmbeddingModel::EmbeddingGemmaFP32,
            EmbeddingModel::EmbeddingGemmaQ4,
        ]
    }

    /// Get the model info for this embedding model
    pub fn model_info(&self) -> ModelInfo {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => ModelInfo {
                id: "embeddinggemma-300m-q8".to_string(),
                name: "EmbeddingGemma 300M (Quantized)".to_string(),
                model_type: ModelType::Embedding,
                size_bytes: 310_000_000, // ~310MB (model + data file)
                download_url: "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model_quantized.onnx".to_string(),
                description: "EmbeddingGemma 300M quantized. Best balance of quality and size for semantic search.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: Some("embeddinggemma-300m-q8".to_string()),
            },
            EmbeddingModel::EmbeddingGemmaFP32 => ModelInfo {
                id: "embeddinggemma-300m-fp32".to_string(),
                name: "EmbeddingGemma 300M (FP32)".to_string(),
                model_type: ModelType::Embedding,
                size_bytes: 1_230_000_000, // ~1.23GB (model + data file)
                download_url: "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model.onnx".to_string(),
                description: "EmbeddingGemma 300M full precision. Highest quality embeddings.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: Some("embeddinggemma-300m-fp32".to_string()),
            },
            EmbeddingModel::EmbeddingGemmaQ4 => ModelInfo {
                id: "embeddinggemma-300m-q4".to_string(),
                name: "EmbeddingGemma 300M (Q4)".to_string(),
                model_type: ModelType::Embedding,
                size_bytes: 198_000_000, // ~198MB (model + data file)
                download_url: "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model_q4.onnx".to_string(),
                description: "EmbeddingGemma 300M quantized to int4. Fastest and smallest, good for low-end hardware.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: Some("embeddinggemma-300m-q4".to_string()),
            },
        }
    }

    /// Get the tokenizer info (same for all EmbeddingGemma variants)
    /// Note: Using public Gemma tokenizer from pcuenq/gemma-tokenizer (no auth required)
    pub fn tokenizer_info() -> ModelInfo {
        ModelInfo {
            id: "embeddinggemma-tokenizer".to_string(),
            name: "EmbeddingGemma Tokenizer".to_string(),
            model_type: ModelType::Embedding,
            size_bytes: 18_000_000, // ~18MB
            download_url: "https://huggingface.co/pcuenq/gemma-tokenizer/resolve/main/tokenizer.json".to_string(),
            description: "Gemma tokenizer for EmbeddingGemma models.".to_string(),
            is_archive: false,
            archive_format: None,
            extracted_dir_name: None,
        }
    }

    /// Get the data file URL for external weights (ONNX models with external data)
    pub fn data_file_url(&self) -> &'static str {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model_quantized.onnx_data",
            EmbeddingModel::EmbeddingGemmaFP32 => "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model.onnx_data",
            EmbeddingModel::EmbeddingGemmaQ4 => "https://huggingface.co/onnx-community/embeddinggemma-300m-ONNX/resolve/main/onnx/model_q4.onnx_data",
        }
    }

    /// Get the data file name for this model
    pub fn data_file_name(&self) -> &'static str {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => "model_quantized.onnx_data",
            EmbeddingModel::EmbeddingGemmaFP32 => "model.onnx_data",
            EmbeddingModel::EmbeddingGemmaQ4 => "model_q4.onnx_data",
        }
    }

    /// Get the model file name for this variant
    pub fn model_file_name(&self) -> &'static str {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => "model_quantized.onnx",
            EmbeddingModel::EmbeddingGemmaFP32 => "model.onnx",
            EmbeddingModel::EmbeddingGemmaQ4 => "model_q4.onnx",
        }
    }

    /// Get the data file size in bytes
    pub fn data_file_size(&self) -> u64 {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => 309_000_000, // ~309MB
            EmbeddingModel::EmbeddingGemmaFP32 => 1_230_000_000, // ~1.23GB
            EmbeddingModel::EmbeddingGemmaQ4 => 197_000_000, // ~197MB
        }
    }

    /// Get the directory name where the model is stored
    pub fn model_dir_name(&self) -> &'static str {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => "embeddinggemma-300m-q8",
            EmbeddingModel::EmbeddingGemmaFP32 => "embeddinggemma-300m-fp32",
            EmbeddingModel::EmbeddingGemmaQ4 => "embeddinggemma-300m-q4",
        }
    }

    /// Get the embedding dimension for this model
    pub fn embedding_dim(&self) -> usize {
        // All EmbeddingGemma variants produce 768-dim embeddings
        768
    }

    /// Get the maximum context length in tokens
    pub fn max_tokens(&self) -> usize {
        // EmbeddingGemma supports up to 2048 tokens
        2048
    }
}

impl std::fmt::Display for EmbeddingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingModel::EmbeddingGemmaQ8 => write!(f, "EmbeddingGemma Q8"),
            EmbeddingModel::EmbeddingGemmaFP32 => write!(f, "EmbeddingGemma FP32"),
            EmbeddingModel::EmbeddingGemmaQ4 => write!(f, "EmbeddingGemma Q4"),
        }
    }
}

/// LLM model variants for summarization and chat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LlmModel {
    /// Qwen3 4B Q4_K_M (default, best balance)
    #[default]
    Qwen3_4B,
    /// Qwen3 1.7B Q4_K_M (lightweight, fastest)
    Qwen3_1_7B,
    /// Qwen3 8B Q4_K_M (highest quality)
    Qwen3_8B,
}

impl LlmModel {
    /// Get all available LLM models
    pub fn all() -> &'static [LlmModel] {
        &[LlmModel::Qwen3_4B, LlmModel::Qwen3_1_7B, LlmModel::Qwen3_8B]
    }

    /// Get the model info for this LLM
    pub fn model_info(&self) -> ModelInfo {
        match self {
            LlmModel::Qwen3_4B => ModelInfo {
                id: "qwen3-4b-q4_k_m".to_string(),
                name: "Qwen3 4B (Q4_K_M)".to_string(),
                model_type: ModelType::LLM,
                size_bytes: 2_500_000_000, // ~2.5GB
                download_url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/qwen3-4b-q4_k_m.gguf".to_string(),
                description: "Qwen3 4B quantized to Q4_K_M. Best balance of quality, speed, and size. 32K context.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: None,
            },
            LlmModel::Qwen3_1_7B => ModelInfo {
                id: "qwen3-1.7b-q4_k_m".to_string(),
                name: "Qwen3 1.7B (Q4_K_M)".to_string(),
                model_type: ModelType::LLM,
                size_bytes: 1_110_000_000, // ~1.11GB
                download_url: "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf".to_string(),
                description: "Qwen3 1.7B quantized to Q4_K_M. Lightweight and fast, good for limited hardware.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: None,
            },
            LlmModel::Qwen3_8B => ModelInfo {
                id: "qwen3-8b-q4_k_m".to_string(),
                name: "Qwen3 8B (Q4_K_M)".to_string(),
                model_type: ModelType::LLM,
                size_bytes: 4_900_000_000, // ~4.9GB
                download_url: "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/qwen3-8b-q4_k_m.gguf".to_string(),
                description: "Qwen3 8B quantized to Q4_K_M. Highest quality summaries, needs 8GB+ VRAM.".to_string(),
                is_archive: false,
                archive_format: None,
                extracted_dir_name: None,
            },
        }
    }

    /// Get the GGUF filename for this model
    pub fn filename(&self) -> &'static str {
        match self {
            LlmModel::Qwen3_4B => "qwen3-4b-q4_k_m.gguf",
            LlmModel::Qwen3_1_7B => "Qwen3-1.7B-Q4_K_M.gguf",
            LlmModel::Qwen3_8B => "qwen3-8b-q4_k_m.gguf",
        }
    }

    /// Get the download URL for this model
    pub fn download_url(&self) -> &'static str {
        match self {
            LlmModel::Qwen3_4B => "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/qwen3-4b-q4_k_m.gguf",
            LlmModel::Qwen3_1_7B => "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf",
            LlmModel::Qwen3_8B => "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/qwen3-8b-q4_k_m.gguf",
        }
    }

    /// Get the approximate size in bytes
    pub fn size_bytes(&self) -> u64 {
        match self {
            LlmModel::Qwen3_4B => 2_500_000_000,
            LlmModel::Qwen3_1_7B => 1_110_000_000, // ~1.11GB
            LlmModel::Qwen3_8B => 4_900_000_000,
        }
    }

    /// Get the native context length in tokens
    pub fn context_length(&self) -> u32 {
        // All Qwen3 models support 32K native context
        32768
    }

    /// Get the directory name where the model is stored
    pub fn model_dir_name(&self) -> &'static str {
        "llm"
    }

    /// Get a human-readable size string
    pub fn size_formatted(&self) -> String {
        let gb = self.size_bytes() as f64 / 1_000_000_000.0;
        format!("{:.1} GB", gb)
    }
}

impl std::fmt::Display for LlmModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmModel::Qwen3_4B => write!(f, "Qwen3 4B"),
            LlmModel::Qwen3_1_7B => write!(f, "Qwen3 1.7B"),
            LlmModel::Qwen3_8B => write!(f, "Qwen3 8B"),
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
