//! Model management module
//!
//! Handles model download, verification, and status tracking.

pub mod downloader;
pub mod registry;

pub use downloader::{default_models_dir, DownloadProgress, DownloadStage, ModelDownloader};
pub use registry::{ArchiveFormat, ModelInfo, ModelType, TranscriptionBackend};

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

/// Status of a model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelStatus {
    /// Model is not downloaded
    NotDownloaded,
    /// Model is currently downloading
    Downloading { percent: u8 },
    /// Model is ready to use
    Ready,
    /// Model download or load failed
    Error(String),
}

impl Default for ModelStatus {
    fn default() -> Self {
        ModelStatus::NotDownloaded
    }
}

/// Manages model downloads and status
pub struct ModelManager {
    downloader: ModelDownloader,
    status: Arc<RwLock<HashMap<String, ModelStatus>>>,
}

impl ModelManager {
    /// Create a new model manager with the given models directory
    pub fn new(models_dir: PathBuf) -> Result<Self> {
        let downloader = ModelDownloader::new(models_dir)?;
        Ok(Self {
            downloader,
            status: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a model manager using the default models directory
    pub fn with_default_dir() -> Result<Self> {
        let models_dir = default_models_dir()?;
        Self::new(models_dir)
    }

    /// Get the models directory path
    pub fn models_dir(&self) -> &std::path::Path {
        self.downloader.models_dir()
    }

    /// Get the path for a specific transcription model
    pub fn get_model_path(&self, backend: TranscriptionBackend) -> PathBuf {
        self.downloader.get_model_path(backend)
    }

    /// Initialize status for all known models
    pub fn init_status(&self) {
        let mut status = self.status.write();

        for backend in TranscriptionBackend::all() {
            let model_info = backend.model_info();
            let current_status = if self.downloader.is_model_downloaded(*backend) {
                ModelStatus::Ready
            } else {
                ModelStatus::NotDownloaded
            };
            status.insert(model_info.id, current_status);
        }

        info!("Initialized model status for {} models", status.len());
    }

    /// Get the status of a specific model
    pub fn get_status(&self, model_id: &str) -> ModelStatus {
        self.status
            .read()
            .get(model_id)
            .cloned()
            .unwrap_or(ModelStatus::NotDownloaded)
    }

    /// Get the status of a transcription backend
    pub fn get_backend_status(&self, backend: TranscriptionBackend) -> ModelStatus {
        let model_info = backend.model_info();
        self.get_status(&model_info.id)
    }

    /// Set the status of a model
    pub fn set_status(&self, model_id: &str, status: ModelStatus) {
        self.status.write().insert(model_id.to_string(), status);
    }

    /// Check if a transcription model is ready
    pub fn is_model_ready(&self, backend: TranscriptionBackend) -> bool {
        self.get_backend_status(backend) == ModelStatus::Ready
    }

    /// Get all model statuses
    pub fn get_all_status(&self) -> HashMap<String, ModelStatus> {
        self.status.read().clone()
    }

    /// Get model info with current status
    pub fn get_model_info_with_status(&self) -> Vec<(ModelInfo, ModelStatus)> {
        TranscriptionBackend::all()
            .iter()
            .map(|backend| {
                let info = backend.model_info();
                let status = self.get_status(&info.id);
                (info, status)
            })
            .collect()
    }

    /// Download a transcription model
    ///
    /// Returns the path to the downloaded model.
    /// Progress is reported via the callback.
    pub async fn download_model<F>(
        &self,
        backend: TranscriptionBackend,
        progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: FnMut(DownloadProgress) + Send,
    {
        let model_info = backend.model_info();

        // Update status to downloading
        self.set_status(&model_info.id, ModelStatus::Downloading { percent: 0 });

        // Wrap callback to update status
        let status = Arc::clone(&self.status);
        let model_id = model_info.id.clone();

        let wrapped_callback = {
            let status = Arc::clone(&status);
            let model_id = model_id.clone();
            let mut inner_callback = progress_callback;

            move |progress: DownloadProgress| {
                // Update internal status
                let new_status = match &progress.stage {
                    DownloadStage::Complete => ModelStatus::Ready,
                    DownloadStage::Failed(msg) => ModelStatus::Error(msg.clone()),
                    _ => ModelStatus::Downloading {
                        percent: progress.percent as u8,
                    },
                };
                status.write().insert(model_id.clone(), new_status);

                // Call user callback
                inner_callback(progress);
            }
        };

        // Perform download
        match self
            .downloader
            .download_transcription_model(backend, wrapped_callback)
            .await
        {
            Ok(path) => {
                self.set_status(&model_info.id, ModelStatus::Ready);
                Ok(path)
            }
            Err(e) => {
                self.set_status(&model_info.id, ModelStatus::Error(e.to_string()));
                Err(e)
            }
        }
    }

    /// Check if a model is downloaded (synchronous check)
    pub fn is_model_downloaded(&self, backend: TranscriptionBackend) -> bool {
        self.downloader.is_model_downloaded(backend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_status_default() {
        assert_eq!(ModelStatus::default(), ModelStatus::NotDownloaded);
    }

    #[test]
    fn test_model_manager_creation() {
        let temp_dir = std::env::temp_dir().join("test_model_manager");
        let manager = ModelManager::new(temp_dir.clone());
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.models_dir(), temp_dir);
    }

    #[test]
    fn test_init_status() {
        let temp_dir = std::env::temp_dir().join("test_init_status");
        let manager = ModelManager::new(temp_dir).unwrap();
        manager.init_status();

        // All models should be NotDownloaded in a fresh temp dir
        for backend in TranscriptionBackend::all() {
            let status = manager.get_backend_status(*backend);
            assert_eq!(status, ModelStatus::NotDownloaded);
        }
    }
}
