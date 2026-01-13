/**
 * Model-related TypeScript types
 * Matches backend types from src-tauri/src/models/
 */

/** Types of models supported by the application */
export type ModelType = 'Transcription' | 'Embedding' | 'LLM' | 'VAD';

/** Transcription engine backends */
export type TranscriptionBackend = 'Parakeet' | 'Whisper' | 'Moonshine';

/** Embedding model variants */
export type EmbeddingModel =
  | 'EmbeddingGemmaQ8'
  | 'EmbeddingGemmaFP32'
  | 'EmbeddingGemmaQ4';

/** LLM model variants */
export type LlmModel = 'Qwen3_4B' | 'Qwen3_1_7B' | 'Qwen3_8B';

/** Model status */
export type ModelStatus =
  | { type: 'NotDownloaded' }
  | { type: 'Downloading'; percent: number }
  | { type: 'Ready' }
  | { type: 'Error'; message: string };

/** Archive format for compressed model downloads */
export type ArchiveFormat = 'TarGz' | 'Zip' | 'TarBz2';

/** Information about a downloadable model */
export interface ModelInfo {
  /** Unique identifier for the model */
  id: string;
  /** Human-readable name */
  name: string;
  /** Type of model */
  model_type: ModelType;
  /** Size in bytes (approximate) */
  size_bytes: number;
  /** URL to download the model from */
  download_url: string;
  /** Description of the model */
  description: string;
  /** Whether the download is an archive */
  is_archive: boolean;
  /** Archive format if is_archive is true */
  archive_format: ArchiveFormat | null;
  /** Name of the directory after extraction */
  extracted_dir_name: string | null;
}

/** Model status item with model info */
export interface ModelStatusItem {
  model_id: string;
  name: string;
  status: ModelStatus;
  size_formatted: string;
}

/** Model status response */
export interface ModelStatusResponse {
  models: ModelStatusItem[];
}

/** Download progress event */
export interface DownloadProgress {
  model_id: string;
  stage: DownloadStage;
  percent: number;
  downloaded: number;
  total: number;
  speed_bps: number | null;
  eta_seconds: number | null;
}

/** Download stage */
export type DownloadStage =
  | 'Starting'
  | 'Downloading'
  | 'Extracting'
  | 'Verifying'
  | 'Complete'
  | { Failed: string };

/** Transcription configuration */
export interface TranscriptionConfig {
  /** Which transcription backend to use */
  backend: TranscriptionBackend;
  /** Language code (e.g., "en" for English) */
  language: string;
  /** Whether to include word-level timestamps */
  word_timestamps: boolean;
  /** Whether to attempt GPU acceleration */
  use_gpu: boolean;
}

/** LLM status information */
export interface LlmStatus {
  /** Whether a model is currently loaded */
  loaded: boolean;
  /** The currently loaded model (if any) */
  current_model: LlmModel | null;
}

/** LLM model information with status */
export interface LlmModelInfo {
  /** Model variant */
  model: LlmModel;
  /** Human-readable name */
  name: string;
  /** Model size in bytes */
  size_bytes: number;
  /** Formatted size string */
  size_formatted: string;
  /** Context length in tokens */
  context_length: number;
  /** Whether the model is downloaded */
  downloaded: boolean;
}

/** LLM download progress event */
export interface LlmDownloadProgress {
  model: LlmModel;
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
  speed_bps: number | null;
}

/** Embedding model information */
export interface EmbeddingInfo {
  loaded: boolean;
  dimension: number;
  max_tokens: number;
  model_id: string;
  model_name: string;
  model_size: string;
}

/** Embedding download progress event */
export interface EmbeddingDownloadProgress {
  model_id: string;
  file: string;
  downloaded: number;
  total: number;
  percent: number;
  status: string;
}

/** Embedding task type */
export type EmbeddingTask = 'document' | 'search' | 'qa' | 'question';
