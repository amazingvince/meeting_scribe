//! Platform-specific audio capture

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

// Re-export platform-specific loopback capture
#[cfg(target_os = "windows")]
pub use windows::SystemAudioCapture;

#[cfg(target_os = "macos")]
pub use macos::SystemAudioCapture;

#[cfg(target_os = "linux")]
pub use linux::SystemAudioCapture;
