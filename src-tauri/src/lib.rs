//! Meeting Scribe - Local-first meeting transcription and RAG
//!
//! This is the main library crate that contains all backend logic.

pub mod audio;
pub mod commands;
pub mod inference;
pub mod models;
pub mod storage;

use std::path::PathBuf;
use std::sync::Once;
use tracing::{info, warn};

static ORT_ENV_INIT: Once = Once::new();

/// Configure ONNX Runtime dynamic library path when needed.
///
/// Packaged desktop apps often launch without shell-inherited environment, so
/// `ORT_DYLIB_PATH` may be missing even when the runtime is bundled next to the
/// executable/resources. We resolve common per-platform locations once.
pub fn ensure_onnx_runtime_env() {
    ORT_ENV_INIT.call_once(|| {
        if let Some(existing) = std::env::var_os("ORT_DYLIB_PATH") {
            let path = PathBuf::from(&existing);
            if path.exists() {
                info!("Using ONNX Runtime from ORT_DYLIB_PATH={}", path.display());
                return;
            }
            warn!(
                "ORT_DYLIB_PATH is set but file does not exist: {}",
                path.display()
            );
        }

        if let Some(path) = resolve_onnx_runtime_path() {
            std::env::set_var("ORT_DYLIB_PATH", &path);
            info!("Using ONNX Runtime from {}", path.display());
            return;
        }

        warn!(
            "ONNX Runtime library not found in known locations; transcription/embedding init may fail"
        );
    });
}

fn resolve_onnx_runtime_path() -> Option<PathBuf> {
    let mut candidate_dirs = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        for dir in onnx_dirs_near_executable(&exe) {
            push_unique_path(&mut candidate_dirs, dir);
        }
    }

    for dir in onnx_system_dirs() {
        push_unique_path(&mut candidate_dirs, dir);
    }

    for dir in onnx_path_env_dirs() {
        push_unique_path(&mut candidate_dirs, dir);
    }

    find_library_in_dirs(
        &candidate_dirs,
        onnx_runtime_library_name(),
        onnx_runtime_library_prefix(),
    )
}

fn onnx_dirs_near_executable(exe_path: &std::path::Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let Some(exe_dir) = exe_path.parent() else {
        return dirs;
    };

    dirs.push(exe_dir.to_path_buf());
    dirs.push(exe_dir.join("resources"));
    dirs.push(exe_dir.join("resources/runtime"));
    dirs.push(exe_dir.join("Resources"));
    dirs.push(exe_dir.join("Resources/runtime"));
    dirs.push(exe_dir.join("lib"));

    if let Some(parent) = exe_dir.parent() {
        dirs.push(parent.join("Resources"));
        dirs.push(parent.join("Resources/runtime"));
        dirs.push(parent.join("Frameworks"));
        dirs.push(parent.join("MacOS"));
        dirs.push(parent.join("lib"));
        dirs.push(parent.join("lib/meeting-scribe"));
        dirs.push(parent.join("lib/meeting-scribe/resources"));
        dirs.push(parent.join("lib/meeting-scribe/resources/runtime"));
        dirs.push(parent.join("lib/meeting_scribe"));
        dirs.push(parent.join("lib/meeting_scribe/resources"));
        dirs.push(parent.join("lib/meeting_scribe/resources/runtime"));
    }

    dirs
}

fn onnx_system_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/opt/homebrew/lib"),
            PathBuf::from("/usr/local/lib"),
        ]
    }

    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from("/usr/lib/meeting-scribe"),
            PathBuf::from("/usr/lib/meeting-scribe/resources"),
            PathBuf::from("/usr/lib/meeting-scribe/resources/runtime"),
            PathBuf::from("/usr/lib/meeting_scribe"),
            PathBuf::from("/usr/lib/meeting_scribe/resources"),
            PathBuf::from("/usr/lib/meeting_scribe/resources/runtime"),
            PathBuf::from("/usr/local/lib"),
            PathBuf::from("/usr/lib"),
        ]
    }

    #[cfg(target_os = "windows")]
    {
        Vec::new()
    }
}

fn onnx_path_env_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if !dir.as_os_str().is_empty() {
                dirs.push(dir);
            }
        }
    }
    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
}

fn find_library_in_dirs(dirs: &[PathBuf], exact_name: &str, prefix_name: &str) -> Option<PathBuf> {
    for dir in dirs {
        let exact = dir.join(exact_name);
        if exact.is_file() {
            return Some(exact);
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| is_runtime_library_name(name, exact_name, prefix_name))
            {
                return Some(path);
            }
        }
    }

    None
}

fn is_runtime_library_name(name: &str, exact_name: &str, prefix_name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // On Windows, avoid accidentally selecting provider DLLs.
        return name.eq_ignore_ascii_case(exact_name);
    }

    #[cfg(not(target_os = "windows"))]
    {
        name == exact_name || name.starts_with(prefix_name)
    }
}

fn onnx_runtime_library_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "onnxruntime.dll"
    }

    #[cfg(target_os = "linux")]
    {
        "libonnxruntime.so"
    }

    #[cfg(target_os = "macos")]
    {
        "libonnxruntime.dylib"
    }
}

fn onnx_runtime_library_prefix() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "onnxruntime"
    }

    #[cfg(target_os = "linux")]
    {
        "libonnxruntime.so."
    }

    #[cfg(target_os = "macos")]
    {
        "libonnxruntime."
    }
}

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

#[cfg(test)]
mod tests {
    use super::{find_library_in_dirs, is_runtime_library_name};
    use std::path::PathBuf;

    #[test]
    fn find_library_prefers_exact_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exact = tmp.path().join("libonnxruntime.so");
        let versioned = tmp.path().join("libonnxruntime.so.1.22.0");
        std::fs::write(&exact, b"exact").expect("write exact");
        std::fs::write(&versioned, b"versioned").expect("write versioned");

        let found = find_library_in_dirs(
            &[PathBuf::from(tmp.path())],
            "libonnxruntime.so",
            "libonnxruntime.so",
        )
        .expect("must find library");

        assert_eq!(found, exact);
    }

    #[test]
    fn find_library_accepts_versioned_shared_object() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let versioned = tmp.path().join("libonnxruntime.so.1.22.0");
        std::fs::write(&versioned, b"versioned").expect("write versioned");

        let found = find_library_in_dirs(
            &[PathBuf::from(tmp.path())],
            "libonnxruntime.so",
            "libonnxruntime.so",
        )
        .expect("must find versioned library");

        assert_eq!(found, versioned);
    }

    #[test]
    fn find_library_accepts_versioned_dylib() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let versioned = tmp.path().join("libonnxruntime.1.22.0.dylib");
        std::fs::write(&versioned, b"versioned").expect("write versioned");

        let found = find_library_in_dirs(
            &[PathBuf::from(tmp.path())],
            "libonnxruntime.dylib",
            "libonnxruntime.",
        )
        .expect("must find versioned library");

        assert_eq!(found, versioned);
    }

    #[test]
    fn runtime_name_matching_skips_provider_library() {
        assert!(!is_runtime_library_name(
            "libonnxruntime_providers_shared.dylib",
            "libonnxruntime.dylib",
            "libonnxruntime.",
        ));
        assert!(is_runtime_library_name(
            "libonnxruntime.1.22.0.dylib",
            "libonnxruntime.dylib",
            "libonnxruntime.",
        ));
    }
}
