//! Model downloader with progress reporting
//!
//! Downloads models from remote URLs with progress events for UI feedback.

use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

use super::registry::{ArchiveFormat, ModelInfo, TranscriptionBackend};

/// Progress information during download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Model being downloaded
    pub model_id: String,
    /// Current stage of the download
    pub stage: DownloadStage,
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Total bytes to download (if known)
    pub total_bytes: Option<u64>,
    /// Progress percentage (0-100)
    pub percent: f32,
    /// Current file being downloaded (for multi-file downloads)
    pub current_file: Option<String>,
}

/// Stages of the download process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadStage {
    /// Starting download
    Starting,
    /// Currently downloading
    Downloading,
    /// Extracting archive
    Extracting,
    /// Verifying files
    Verifying,
    /// Download complete
    Complete,
    /// Download failed
    Failed(String),
}

/// Model downloader with HTTP client
pub struct ModelDownloader {
    client: Client,
    models_dir: PathBuf,
}

impl ModelDownloader {
    /// Create a new model downloader
    pub fn new(models_dir: PathBuf) -> Result<Self> {
        let client = Client::builder()
            .user_agent("meeting-scribe/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, models_dir })
    }

    /// Get the models directory
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Ensure models directory exists
    pub async fn ensure_models_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.models_dir)
            .await
            .context("Failed to create models directory")?;
        Ok(())
    }

    /// Check if a transcription model is already downloaded
    pub fn is_model_downloaded(&self, backend: TranscriptionBackend) -> bool {
        let model_path = self.get_model_path(backend);
        model_path.exists() && self.verify_model_files(backend, &model_path)
    }

    /// Get the path where a model should be stored
    pub fn get_model_path(&self, backend: TranscriptionBackend) -> PathBuf {
        self.models_dir.join(backend.model_dir_name())
    }

    /// Verify that required model files exist
    fn verify_model_files(&self, backend: TranscriptionBackend, model_path: &Path) -> bool {
        match backend {
            TranscriptionBackend::Parakeet => {
                // Parakeet requires these files
                let required = [
                    "encoder-model.int8.onnx",
                    "decoder_joint-model.int8.onnx",
                    "nemo128.onnx",
                    "vocab.txt",
                ];
                required.iter().all(|f| model_path.join(f).exists())
            }
            TranscriptionBackend::Whisper => {
                // Whisper is a single file
                model_path.exists() && model_path.is_file()
            }
            TranscriptionBackend::Moonshine => {
                // Moonshine requires these files
                let required = [
                    "encoder_model.onnx",
                    "decoder_model_merged.onnx",
                    "tokenizer.json",
                ];
                required.iter().all(|f| model_path.join(f).exists())
            }
        }
    }

    /// Download a transcription model with progress callback
    pub async fn download_transcription_model<F>(
        &self,
        backend: TranscriptionBackend,
        mut progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: FnMut(DownloadProgress) + Send,
    {
        let model_info = backend.model_info();
        let model_path = self.get_model_path(backend);

        // Check if already downloaded
        if self.is_model_downloaded(backend) {
            info!("Model {} already downloaded", model_info.id);
            progress_callback(DownloadProgress {
                model_id: model_info.id.clone(),
                stage: DownloadStage::Complete,
                downloaded_bytes: model_info.size_bytes,
                total_bytes: Some(model_info.size_bytes),
                percent: 100.0,
                current_file: None,
            });
            return Ok(model_path);
        }

        // Ensure models directory exists
        self.ensure_models_dir().await?;

        // Report starting
        progress_callback(DownloadProgress {
            model_id: model_info.id.clone(),
            stage: DownloadStage::Starting,
            downloaded_bytes: 0,
            total_bytes: Some(model_info.size_bytes),
            percent: 0.0,
            current_file: None,
        });

        // Download based on backend type
        match backend {
            TranscriptionBackend::Parakeet => {
                self.download_archive(&model_info, &model_path, &mut progress_callback)
                    .await?;
            }
            TranscriptionBackend::Whisper => {
                self.download_single_file(&model_info, &model_path, &mut progress_callback)
                    .await?;
            }
            TranscriptionBackend::Moonshine => {
                self.download_moonshine(&model_info, &model_path, &mut progress_callback)
                    .await?;
            }
        }

        // Report complete
        progress_callback(DownloadProgress {
            model_id: model_info.id.clone(),
            stage: DownloadStage::Complete,
            downloaded_bytes: model_info.size_bytes,
            total_bytes: Some(model_info.size_bytes),
            percent: 100.0,
            current_file: None,
        });

        info!("Model {} downloaded successfully", model_info.id);
        Ok(model_path)
    }

    /// Download and extract an archive (tar.gz)
    async fn download_archive<F>(
        &self,
        model_info: &ModelInfo,
        model_path: &Path,
        progress_callback: &mut F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress),
    {
        // Download to temp file
        let temp_path = self.models_dir.join(format!("{}.tmp", model_info.id));
        self.download_file(
            &model_info.download_url,
            &temp_path,
            model_info,
            progress_callback,
        )
        .await?;

        // Report extracting
        progress_callback(DownloadProgress {
            model_id: model_info.id.clone(),
            stage: DownloadStage::Extracting,
            downloaded_bytes: model_info.size_bytes,
            total_bytes: Some(model_info.size_bytes),
            percent: 95.0,
            current_file: None,
        });

        // Extract archive
        self.extract_archive(&temp_path, &self.models_dir, model_info.archive_format)
            .await?;

        // Clean up temp file
        let _ = fs::remove_file(&temp_path).await;

        // Verify extraction
        progress_callback(DownloadProgress {
            model_id: model_info.id.clone(),
            stage: DownloadStage::Verifying,
            downloaded_bytes: model_info.size_bytes,
            total_bytes: Some(model_info.size_bytes),
            percent: 98.0,
            current_file: None,
        });

        if !model_path.exists() {
            anyhow::bail!(
                "Model extraction failed: expected directory {} not found",
                model_path.display()
            );
        }

        Ok(())
    }

    /// Download a single file
    async fn download_single_file<F>(
        &self,
        model_info: &ModelInfo,
        model_path: &Path,
        progress_callback: &mut F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress),
    {
        self.download_file(
            &model_info.download_url,
            model_path,
            model_info,
            progress_callback,
        )
        .await
    }

    /// Download moonshine model (multiple files)
    async fn download_moonshine<F>(
        &self,
        model_info: &ModelInfo,
        model_path: &Path,
        progress_callback: &mut F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress),
    {
        use super::registry::MoonshineFiles;

        // Create model directory
        fs::create_dir_all(model_path)
            .await
            .context("Failed to create moonshine model directory")?;

        let files = MoonshineFiles::download_urls();
        let total_files = files.len();

        for (i, (url, filename)) in files.into_iter().enumerate() {
            let file_path = model_path.join(filename);

            progress_callback(DownloadProgress {
                model_id: model_info.id.clone(),
                stage: DownloadStage::Downloading,
                downloaded_bytes: 0,
                total_bytes: None,
                percent: (i as f32 / total_files as f32) * 100.0,
                current_file: Some(filename.to_string()),
            });

            self.download_file_simple(&url, &file_path).await?;
        }

        Ok(())
    }

    /// Download a file with progress tracking
    async fn download_file<F>(
        &self,
        url: &str,
        dest_path: &Path,
        model_info: &ModelInfo,
        progress_callback: &mut F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress),
    {
        debug!("Downloading {} to {:?}", url, dest_path);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to start download")?
            .error_for_status()
            .context("Server returned error status")?;

        let total_size = response.content_length().unwrap_or(model_info.size_bytes);

        // Create parent directory if needed
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file =
            std::fs::File::create(dest_path).context("Failed to create download file")?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk)
                .context("Failed to write to download file")?;

            downloaded += chunk.len() as u64;

            // Report progress (limit updates to avoid flooding)
            if downloaded % (1024 * 100) < chunk.len() as u64 {
                let percent = if total_size > 0 {
                    (downloaded as f32 / total_size as f32) * 90.0 // Leave 10% for extraction
                } else {
                    0.0
                };

                progress_callback(DownloadProgress {
                    model_id: model_info.id.clone(),
                    stage: DownloadStage::Downloading,
                    downloaded_bytes: downloaded,
                    total_bytes: Some(total_size),
                    percent,
                    current_file: None,
                });
            }
        }

        file.flush().context("Failed to flush download file")?;

        info!("Downloaded {} bytes to {:?}", downloaded, dest_path);
        Ok(())
    }

    /// Simple file download without detailed progress (for multi-file downloads)
    async fn download_file_simple(&self, url: &str, dest_path: &Path) -> Result<()> {
        debug!("Downloading {} to {:?}", url, dest_path);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to start download")?
            .error_for_status()
            .context("Server returned error status")?;

        let bytes = response.bytes().await.context("Failed to download file")?;

        fs::write(dest_path, &bytes)
            .await
            .context("Failed to write file")?;

        info!("Downloaded {} bytes to {:?}", bytes.len(), dest_path);
        Ok(())
    }

    /// Extract an archive file
    async fn extract_archive(
        &self,
        archive_path: &Path,
        dest_dir: &Path,
        format: Option<ArchiveFormat>,
    ) -> Result<()> {
        let format = format.unwrap_or(ArchiveFormat::TarGz);

        match format {
            ArchiveFormat::TarGz => self.extract_tar_gz(archive_path, dest_dir).await,
            ArchiveFormat::TarBz2 => self.extract_tar_bz2(archive_path, dest_dir).await,
            ArchiveFormat::Zip => self.extract_zip(archive_path, dest_dir).await,
        }
    }

    /// Extract a tar.gz archive
    async fn extract_tar_gz(&self, archive_path: &Path, dest_dir: &Path) -> Result<()> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let archive_path = archive_path.to_path_buf();
        let dest_dir = dest_dir.to_path_buf();

        // Run extraction in blocking task
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&archive_path).context("Failed to open archive file")?;
            let decoder = GzDecoder::new(file);
            let mut archive = Archive::new(decoder);

            archive
                .unpack(&dest_dir)
                .context("Failed to extract tar.gz archive")?;

            info!("Extracted archive to {:?}", dest_dir);
            Ok(())
        })
        .await
        .context("Extraction task failed")?
    }

    /// Extract a tar.bz2 archive
    async fn extract_tar_bz2(&self, archive_path: &Path, dest_dir: &Path) -> Result<()> {
        use bzip2::read::BzDecoder;
        use tar::Archive;

        let archive_path = archive_path.to_path_buf();
        let dest_dir = dest_dir.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&archive_path).context("Failed to open archive file")?;
            let decoder = BzDecoder::new(file);
            let mut archive = Archive::new(decoder);

            archive
                .unpack(&dest_dir)
                .context("Failed to extract tar.bz2 archive")?;

            info!("Extracted archive to {:?}", dest_dir);
            Ok(())
        })
        .await
        .context("Extraction task failed")?
    }

    /// Extract a zip archive
    async fn extract_zip(&self, archive_path: &Path, dest_dir: &Path) -> Result<()> {
        use zip::ZipArchive;

        let archive_path = archive_path.to_path_buf();
        let dest_dir = dest_dir.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&archive_path).context("Failed to open archive file")?;
            let mut archive = ZipArchive::new(file).context("Failed to read zip archive")?;

            archive
                .extract(&dest_dir)
                .context("Failed to extract zip archive")?;

            info!("Extracted archive to {:?}", dest_dir);
            Ok(())
        })
        .await
        .context("Extraction task failed")?
    }
}

/// Get the default models directory for the application
pub fn default_models_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .context("Failed to find data directory")?;

    Ok(data_dir.join("meeting-scribe").join("models"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_models_dir() {
        let dir = default_models_dir();
        assert!(dir.is_ok());
        let dir = dir.unwrap();
        assert!(dir.ends_with("models"));
    }

    #[test]
    fn test_model_path() {
        let temp_dir = std::env::temp_dir().join("test_models");
        let downloader = ModelDownloader::new(temp_dir.clone()).unwrap();

        let path = downloader.get_model_path(TranscriptionBackend::Parakeet);
        assert_eq!(path, temp_dir.join("parakeet-tdt-0.6b-v3-int8"));
    }
}
