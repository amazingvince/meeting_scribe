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

/// Runtime capabilities for the current platform's system-audio backend.
#[derive(Debug, Clone, Copy)]
pub struct SystemAudioBackendCapabilities {
    pub backend_name: &'static str,
    pub supported: bool,
    pub requirements: &'static str,
}

pub fn system_audio_backend_capabilities() -> SystemAudioBackendCapabilities {
    #[cfg(target_os = "windows")]
    {
        SystemAudioBackendCapabilities {
            backend_name: "WASAPI loopback",
            supported: true,
            requirements: "Default output device must be available.",
        }
    }

    #[cfg(target_os = "macos")]
    {
        SystemAudioBackendCapabilities {
            backend_name: "CoreAudio Process Tap (with loopback fallback)",
            supported: true,
            requirements:
                "macOS 14.2+ for native Process Tap, or loopback input device (BlackHole/Loopback/Soundflower/Background Music) as fallback.",
        }
    }

    #[cfg(target_os = "linux")]
    {
        SystemAudioBackendCapabilities {
            backend_name: "PipeWire/Pulse monitor input",
            supported: true,
            requirements:
                "Requires a monitor/loopback input device (for example 'Monitor of ...') exposed by PipeWire/PulseAudio.",
        }
    }
}
