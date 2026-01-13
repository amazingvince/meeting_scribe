//! LLM model downloader with progress tracking
//!
//! Downloads GGUF models from Hugging Face with progress reporting.

use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tracing::info;

use crate::models::LlmModel;

/// Progress information during LLM model download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDownloadProgress {
    /// Model being downloaded
    pub model: LlmModel,
    /// Progress percentage (0.0 - 1.0)
    pub progress: f32,
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Total bytes to download
    pub total_bytes: u64,
    /// Download speed in bytes per second
    pub speed_bps: Option<u64>,
}

/// Download an LLM model with progress callback
pub async fn download_llm_model<F>(
    model: LlmModel,
    dest_dir: PathBuf,
    mut progress_callback: F,
) -> Result<PathBuf>
where
    F: FnMut(LlmDownloadProgress) + Send,
{
    let url = model.download_url();
    let filename = model.filename();
    let dest_path = dest_dir.join(filename);

    // Create directory if needed
    fs::create_dir_all(&dest_dir)
        .await
        .context("Failed to create LLM models directory")?;

    // Check if already downloaded
    if dest_path.exists() {
        let metadata = fs::metadata(&dest_path).await?;
        // Consider complete if at least 90% of expected size
        if metadata.len() > model.size_bytes() * 9 / 10 {
            info!("LLM model already exists: {:?}", dest_path);
            return Ok(dest_path);
        }
        // Remove partial download
        fs::remove_file(&dest_path).await?;
    }

    info!("Downloading LLM model: {} from {}", model, url);

    let client = Client::builder()
        .user_agent("meeting-scribe/0.1.0")
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to start download")?
        .error_for_status()
        .context("Download request failed")?;

    let total_bytes = response.content_length().unwrap_or(model.size_bytes());

    let mut file = File::create(&dest_path)
        .await
        .context("Failed to create model file")?;

    let mut downloaded_bytes: u64 = 0;
    let start_time = std::time::Instant::now();

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error while downloading")?;
        file.write_all(&chunk)
            .await
            .context("Failed to write chunk")?;
        downloaded_bytes += chunk.len() as u64;

        let elapsed = start_time.elapsed().as_secs_f64();
        let speed_bps = if elapsed > 0.0 {
            Some((downloaded_bytes as f64 / elapsed) as u64)
        } else {
            None
        };

        let progress = LlmDownloadProgress {
            model,
            progress: downloaded_bytes as f32 / total_bytes as f32,
            downloaded_bytes,
            total_bytes,
            speed_bps,
        };
        progress_callback(progress);
    }

    file.flush().await.context("Failed to flush file")?;

    info!("LLM model download complete: {:?}", dest_path);
    Ok(dest_path)
}

/// Check if an LLM model is downloaded
pub fn is_llm_downloaded(model: LlmModel, models_dir: &PathBuf) -> bool {
    let path = models_dir.join(model.model_dir_name()).join(model.filename());
    if path.exists() {
        // Check file size (at least 90% of expected)
        if let Ok(metadata) = std::fs::metadata(&path) {
            return metadata.len() > model.size_bytes() * 9 / 10;
        }
    }
    false
}

/// Check if an LLM model is downloaded (async version)
pub async fn is_llm_downloaded_async(model: LlmModel, models_dir: &PathBuf) -> bool {
    let path = models_dir.join(model.model_dir_name()).join(model.filename());
    if let Ok(metadata) = fs::metadata(&path).await {
        return metadata.len() > model.size_bytes() * 9 / 10;
    }
    false
}

/// Get total disk space used by LLM models
pub async fn get_llm_models_size(models_dir: &PathBuf) -> Result<u64> {
    let llm_dir = models_dir.join("llm");
    if !llm_dir.exists() {
        return Ok(0);
    }

    let mut total = 0u64;
    let mut entries = fs::read_dir(&llm_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        if let Ok(metadata) = entry.metadata().await {
            total += metadata.len();
        }
    }

    Ok(total)
}

/// List all downloaded LLM models
pub async fn list_downloaded_models(models_dir: &PathBuf) -> Vec<LlmModel> {
    LlmModel::all()
        .iter()
        .filter(|model| is_llm_downloaded(**model, models_dir))
        .copied()
        .collect()
}

/// Delete a downloaded LLM model
pub async fn delete_llm_model(model: LlmModel, models_dir: &PathBuf) -> Result<()> {
    let path = models_dir.join(model.model_dir_name()).join(model.filename());
    if path.exists() {
        fs::remove_file(&path)
            .await
            .context("Failed to delete model file")?;
        info!("Deleted LLM model: {:?}", path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path() {
        let models_dir = PathBuf::from("/test/models");
        let model = LlmModel::Qwen3_4B;
        let expected = models_dir.join("llm").join("qwen3-4b-q4_k_m.gguf");
        let actual = models_dir
            .join(model.model_dir_name())
            .join(model.filename());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_default_models_dir() {
        let model = LlmModel::default();
        assert_eq!(model, LlmModel::Qwen3_4B);
    }
}
