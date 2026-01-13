//! Meeting Scribe - Local-first meeting transcription and RAG
//!
//! This is the main library crate that contains all backend logic.

pub mod audio;
pub mod commands;
pub mod inference;
pub mod models;
pub mod storage;

use std::path::PathBuf;
use tracing::info;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Base data directory (~/.meeting-scribe)
    pub data_dir: PathBuf,
    /// Directory for audio files
    pub audio_dir: PathBuf,
    /// Directory for ML models
    pub models_dir: PathBuf,
    /// Directory for cache
    pub cache_dir: PathBuf,
}

impl AppConfig {
    /// Create config with default paths
    pub fn new() -> anyhow::Result<Self> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
            .join("meeting-scribe");

        Ok(Self {
            audio_dir: data_dir.join("audio"),
            models_dir: data_dir.join("models"),
            cache_dir: data_dir.join("cache"),
            data_dir,
        })
    }

    /// Ensure all directories exist
    pub fn ensure_dirs(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.audio_dir)?;
        std::fs::create_dir_all(&self.models_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        info!("Data directories initialized at {:?}", self.data_dir);
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new().expect("Failed to create default config")
    }
}
