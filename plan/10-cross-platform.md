# 10. Cross-Platform Audio Capture

**Goal:** Implement system audio capture for macOS and Linux platforms to complement the existing Windows WASAPI implementation.

**Estimated Time:** 5-6 days

**Prerequisites:**
- Document 02 (Audio Capture) completed with Windows implementation
- Understanding of platform-specific audio APIs
- Access to macOS and Linux test machines

## Table of Contents

1. [Platform Overview](#platform-overview)
2. [macOS Implementation](#macos-implementation)
3. [Linux Implementation](#linux-implementation)
4. [Unified Platform Abstraction](#unified-platform-abstraction)
5. [Build Configuration](#build-configuration)
6. [Testing Strategy](#testing-strategy)
7. [Troubleshooting](#troubleshooting)

---

## Platform Overview

### System Audio Capture Comparison

| Platform | API | Crate | Latency | Permissions |
|----------|-----|-------|---------|-------------|
| Windows | WASAPI Loopback | windows-rs | ~10ms | None required |
| macOS | ScreenCaptureKit | cidre/objc2 | ~20ms | Screen Recording permission |
| Linux | PipeWire | pipewire-rs | ~10ms | None (user session) |
| Linux (fallback) | PulseAudio | libpulse-binding | ~30ms | None |

### Architecture

```
src-tauri/src/audio/
├── platform/
│   ├── mod.rs              # Platform detection and trait
│   ├── windows.rs          # WASAPI loopback (existing)
│   ├── macos.rs            # ScreenCaptureKit
│   └── linux.rs            # PipeWire + PulseAudio fallback
├── capture.rs              # Unified capture interface
└── mod.rs
```

---

## macOS Implementation

### References

- [ScreenCaptureKit Documentation](https://developer.apple.com/documentation/screencapturekit)
- [ScreenCaptureKit Audio Capture](https://developer.apple.com/documentation/screencapturekit/capturing_screen_content_in_macos)
- [cidre crate](https://github.com/aspect-rs/cidre) - Safe Rust bindings for Apple frameworks
- [screenpipe macOS implementation](https://github.com/mediar-ai/screenpipe/blob/main/screenpipe-audio/src/stt.rs)

### Permission Requirements

ScreenCaptureKit requires the "Screen Recording" permission in macOS 10.15+. This is because audio capture is bundled with screen capture permissions.

#### Requesting Permission

```rust
// src-tauri/src/audio/platform/macos.rs

use std::process::Command;

/// Check if screen recording permission is granted
pub fn check_screen_capture_permission() -> bool {
    // Use CGPreflightScreenCaptureAccess (macOS 10.15+)
    #[cfg(target_os = "macos")]
    {
        use core_graphics::access::CGPreflightScreenCaptureAccess;
        CGPreflightScreenCaptureAccess()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// Request screen recording permission
/// Returns true if permission was already granted
pub fn request_screen_capture_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::access::CGRequestScreenCaptureAccess;
        CGRequestScreenCaptureAccess()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// Open System Preferences to the Screen Recording pane
pub fn open_screen_recording_preferences() -> anyhow::Result<()> {
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn()?;
    Ok(())
}
```

### ScreenCaptureKit Implementation

#### Dependencies

```toml
# Cargo.toml - macOS-specific dependencies

[target.'cfg(target_os = "macos")'.dependencies]
# Option 1: cidre (higher-level, safer)
cidre = { version = "0.4", features = ["sc", "cm", "av"] }

# Option 2: objc2 ecosystem (more manual but complete)
objc2 = "0.5"
objc2-foundation = "0.2"
objc2-screen-capture-kit = "0.2"
objc2-core-media = "0.2"
objc2-avf-audio = "0.2"

# Core Graphics for permissions
core-graphics = "0.24"

# Dispatch for async operations
dispatch = "0.2"
```

#### Using cidre (Recommended)

```rust
// src-tauri/src/audio/platform/macos.rs

use cidre::{
    arc,
    cm::{self, SampleBuffer},
    define_obj_type,
    dispatch::{self, Queue},
    objc::{self, Obj},
    sc::{
        self, ContentFilter, ShareableContent, Stream, StreamConfiguration,
        StreamOutput, StreamOutputType,
    },
};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Audio sample format from ScreenCaptureKit
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_ns: u64,
}

/// macOS system audio capture using ScreenCaptureKit
pub struct MacOSAudioCapture {
    stream: Option<arc::R<Stream>>,
    output_handler: Arc<AudioOutputHandler>,
    sender: mpsc::UnboundedSender<AudioFrame>,
}

/// Stream output delegate
struct AudioOutputHandler {
    sender: mpsc::UnboundedSender<AudioFrame>,
}

impl StreamOutput for AudioOutputHandler {
    fn stream_did_output_sample_buffer(
        &mut self,
        _stream: &Stream,
        sample_buffer: &SampleBuffer,
        of_type: StreamOutputType,
    ) {
        // Only process audio samples
        if of_type != StreamOutputType::Audio {
            return;
        }

        // Extract audio data from CMSampleBuffer
        if let Some(audio_frame) = self.extract_audio_samples(sample_buffer) {
            let _ = self.sender.send(audio_frame);
        }
    }
}

impl AudioOutputHandler {
    fn extract_audio_samples(&self, sample_buffer: &SampleBuffer) -> Option<AudioFrame> {
        // Get the audio buffer list
        let block_buffer = sample_buffer.data_buffer()?;
        let format_desc = sample_buffer.format_description()?;
        
        // Get audio format details
        let asbd = format_desc.audio_stream_basic_description()?;
        let sample_rate = asbd.sample_rate as u32;
        let channels = asbd.channels_per_frame as u16;
        
        // Get raw audio data
        let data_length = block_buffer.data_length();
        let mut data = vec![0u8; data_length];
        block_buffer.copy_data_bytes(0, data_length, data.as_mut_ptr())?;
        
        // Convert to f32 samples (assuming 32-bit float input)
        let samples: Vec<f32> = data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        
        // Get timestamp
        let pts = sample_buffer.presentation_time_stamp();
        let timestamp_ns = (pts.seconds() * 1_000_000_000.0) as u64;
        
        Some(AudioFrame {
            samples,
            sample_rate,
            channels,
            timestamp_ns,
        })
    }
}

impl MacOSAudioCapture {
    /// Create new macOS audio capture
    pub async fn new() -> anyhow::Result<(Self, mpsc::UnboundedReceiver<AudioFrame>)> {
        // Check permission first
        if !check_screen_capture_permission() {
            if !request_screen_capture_permission() {
                anyhow::bail!(
                    "Screen Recording permission required. Please grant permission in System Preferences."
                );
            }
        }
        
        let (sender, receiver) = mpsc::unbounded_channel();
        
        let output_handler = Arc::new(AudioOutputHandler {
            sender: sender.clone(),
        });
        
        Ok((
            Self {
                stream: None,
                output_handler,
                sender,
            },
            receiver,
        ))
    }
    
    /// Start capturing system audio
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // Get shareable content (available windows/displays)
        let content = ShareableContent::current()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get shareable content: {:?}", e))?;
        
        // Get the main display
        let displays = content.displays();
        let display = displays
            .first()
            .ok_or_else(|| anyhow::anyhow!("No displays found"))?;
        
        // Create content filter for the display
        let filter = ContentFilter::with_display_excluding_windows(
            display,
            &[], // No excluded windows
        );
        
        // Configure stream for audio only
        let mut config = StreamConfiguration::new();
        config.set_captures_audio(true);
        config.set_excludes_current_process_audio(true); // Don't capture our own audio
        config.set_width(1); // Minimal video (required but we ignore it)
        config.set_height(1);
        config.set_minimum_frame_interval(cm::Time::new(1, 1)); // 1 FPS (minimal)
        
        // Set audio sample rate to 48kHz for quality
        config.set_sample_rate(48000);
        config.set_channel_count(2); // Stereo
        
        // Create the stream
        let stream = Stream::new(&filter, &config)
            .map_err(|e| anyhow::anyhow!("Failed to create stream: {:?}", e))?;
        
        // Create dispatch queue for callbacks
        let queue = Queue::new("com.meeting-scribe.audio-capture", dispatch::QueueAttr::Serial);
        
        // Add output handler
        stream.add_stream_output(
            self.output_handler.as_ref(),
            StreamOutputType::Audio,
            Some(&queue),
        )?;
        
        // Start the stream
        stream
            .start()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start stream: {:?}", e))?;
        
        self.stream = Some(stream);
        
        tracing::info!("macOS audio capture started");
        Ok(())
    }
    
    /// Stop capturing
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(stream) = self.stream.take() {
            stream
                .stop()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to stop stream: {:?}", e))?;
        }
        
        tracing::info!("macOS audio capture stopped");
        Ok(())
    }
}
```

#### Alternative: Using objc2 Directly

For more control or if cidre doesn't fit your needs:

```rust
// src-tauri/src/audio/platform/macos_objc2.rs

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::ProtocolObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_foundation::{
    NSArray, NSError, NSObject, NSObjectProtocol, NSString,
};
use objc2_screen_capture_kit::{
    SCContentFilter, SCDisplay, SCShareableContent, SCStream,
    SCStreamConfiguration, SCStreamDelegate, SCStreamOutput,
    SCStreamOutputType,
};
use objc2_core_media::CMSampleBufferRef;
use std::sync::mpsc;

// Declare the stream output handler class
declare_class!(
    struct AudioStreamOutput {
        sender: mpsc::Sender<Vec<f32>>,
    }

    unsafe impl ClassType for AudioStreamOutput {
        type Super = NSObject;
        type Mutability = mutability::Mutable;
        const NAME: &'static str = "AudioStreamOutput";
    }

    impl DeclaredClass for AudioStreamOutput {
        type Ivars = AudioStreamOutputIvars;
    }

    unsafe impl NSObjectProtocol for AudioStreamOutput {}

    unsafe impl SCStreamOutput for AudioStreamOutput {
        #[method(stream:didOutputSampleBuffer:ofType:)]
        fn stream_did_output_sample_buffer(
            &self,
            stream: &SCStream,
            sample_buffer: CMSampleBufferRef,
            output_type: SCStreamOutputType,
        ) {
            if output_type != SCStreamOutputType::Audio {
                return;
            }
            
            // Extract samples and send
            if let Some(samples) = extract_samples_from_buffer(sample_buffer) {
                let _ = self.ivars().sender.send(samples);
            }
        }
    }

    unsafe impl SCStreamDelegate for AudioStreamOutput {
        #[method(stream:didStopWithError:)]
        fn stream_did_stop_with_error(&self, _stream: &SCStream, error: Option<&NSError>) {
            if let Some(err) = error {
                tracing::error!("Stream stopped with error: {:?}", err);
            }
        }
    }
);

/// Start audio capture using objc2
pub async fn start_capture_objc2() -> anyhow::Result<mpsc::Receiver<Vec<f32>>> {
    let (sender, receiver) = mpsc::channel();
    
    autoreleasepool(|_| {
        // Get shareable content
        let content = unsafe {
            SCShareableContent::getShareableContentWithCompletionHandler(|content, error| {
                // Handle content
            })
        };
        
        // Configuration and stream setup...
    });
    
    Ok(receiver)
}
```

### Handling Audio Format Conversion

ScreenCaptureKit outputs audio in various formats. Convert to our standard format:

```rust
// src-tauri/src/audio/platform/macos_convert.rs

use rubato::{FftFixedIn, Resampler};

/// Convert macOS audio to our standard format (16kHz mono f32)
pub struct MacOSAudioConverter {
    resampler: Option<FftFixedIn<f32>>,
    input_sample_rate: u32,
    input_channels: u16,
}

impl MacOSAudioConverter {
    pub fn new(input_sample_rate: u32, input_channels: u16) -> Self {
        let resampler = if input_sample_rate != 16000 {
            Some(
                FftFixedIn::<f32>::new(
                    input_sample_rate as usize,
                    16000,
                    1024,
                    2,
                    input_channels as usize,
                )
                .expect("Failed to create resampler"),
            )
        } else {
            None
        };
        
        Self {
            resampler,
            input_sample_rate,
            input_channels,
        }
    }
    
    /// Convert stereo audio to mono and resample to 16kHz
    pub fn convert(&mut self, input: &[f32]) -> Vec<f32> {
        // First convert to mono if stereo
        let mono = if self.input_channels == 2 {
            input
                .chunks_exact(2)
                .map(|chunk| (chunk[0] + chunk[1]) * 0.5)
                .collect::<Vec<f32>>()
        } else {
            input.to_vec()
        };
        
        // Then resample if needed
        if let Some(resampler) = &mut self.resampler {
            let input_frames = vec![mono];
            match resampler.process(&input_frames, None) {
                Ok(output) => output.into_iter().next().unwrap_or_default(),
                Err(e) => {
                    tracing::error!("Resampling error: {:?}", e);
                    vec![]
                }
            }
        } else {
            mono
        }
    }
}
```

### App Entitlements

Create the entitlements file for macOS:

```xml
<!-- src-tauri/entitlements.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Allow screen/audio capture -->
    <key>com.apple.security.screen-capture.allow</key>
    <true/>
    
    <!-- Audio input (for microphone) -->
    <key>com.apple.security.device.audio-input</key>
    <true/>
    
    <!-- App Sandbox (required for distribution) -->
    <key>com.apple.security.app-sandbox</key>
    <true/>
    
    <!-- Allow user-selected files -->
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    
    <!-- Network access (for model downloads) -->
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>
```

Update Tauri config:

```json
// src-tauri/tauri.conf.json
{
  "bundle": {
    "macOS": {
      "entitlements": "entitlements.plist",
      "minimumSystemVersion": "12.3",
      "frameworks": [],
      "providerShortName": null,
      "signingIdentity": null
    }
  }
}
```

---

## Linux Implementation

### References

- [PipeWire Documentation](https://docs.pipewire.org/)
- [pipewire-rs crate](https://gitlab.freedesktop.org/pipewire/pipewire-rs)
- [PulseAudio Simple API](https://www.freedesktop.org/wiki/Software/PulseAudio/Documentation/Developer/)
- [libpulse-binding crate](https://docs.rs/libpulse-binding/)
- [screenpipe Linux audio](https://github.com/mediar-ai/screenpipe/blob/main/screenpipe-audio/src/core.rs)

### PipeWire vs PulseAudio

| Feature | PipeWire | PulseAudio |
|---------|----------|------------|
| Latency | ~10ms | ~30ms |
| Screen capture | Native | Via module |
| Modern distros | Default | Legacy |
| API complexity | Higher | Lower |
| Bluetooth | Excellent | Good |

**Strategy:** Try PipeWire first, fall back to PulseAudio.

### PipeWire Implementation

#### Dependencies

```toml
# Cargo.toml - Linux-specific dependencies

[target.'cfg(target_os = "linux")'.dependencies]
# PipeWire bindings
pipewire = "0.8"

# PulseAudio fallback
libpulse-binding = "2.28"
libpulse-simple-binding = "2.28"

# For detecting audio system
which = "6.0"
```

#### System Requirements

```bash
# Ubuntu/Debian
sudo apt install libpipewire-0.3-dev pipewire-audio-client-libraries

# Fedora
sudo dnf install pipewire-devel pipewire-libs

# Arch
sudo pacman -S pipewire pipewire-audio
```

#### PipeWire Capture

```rust
// src-tauri/src/audio/platform/linux.rs

use pipewire::{
    context::Context,
    core::Core,
    main_loop::MainLoop,
    properties::properties,
    spa::{
        param::audio::{AudioFormat, AudioInfoRaw},
        pod::Pod,
        utils::Direction,
    },
    stream::{Stream, StreamFlags, StreamListener, StreamState},
};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Audio sample from PipeWire
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_ns: u64,
}

/// Linux system audio capture using PipeWire
pub struct LinuxAudioCapture {
    main_loop: Option<MainLoop>,
    sender: mpsc::UnboundedSender<AudioFrame>,
    _thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl LinuxAudioCapture {
    /// Create new Linux audio capture
    pub fn new() -> anyhow::Result<(Self, mpsc::UnboundedReceiver<AudioFrame>)> {
        // Check if PipeWire is available
        if !Self::is_pipewire_available() {
            anyhow::bail!("PipeWire not available, use PulseAudio fallback");
        }
        
        let (sender, receiver) = mpsc::unbounded_channel();
        
        Ok((
            Self {
                main_loop: None,
                sender,
                _thread_handle: None,
            },
            receiver,
        ))
    }
    
    /// Check if PipeWire is available
    fn is_pipewire_available() -> bool {
        // Check if pipewire socket exists
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        
        let socket_path = format!("{}/pipewire-0", runtime_dir);
        std::path::Path::new(&socket_path).exists()
    }
    
    /// Start capturing system audio
    pub fn start(&mut self) -> anyhow::Result<()> {
        let sender = self.sender.clone();
        
        // PipeWire runs on its own thread
        let handle = std::thread::spawn(move || {
            if let Err(e) = run_pipewire_capture(sender) {
                tracing::error!("PipeWire capture error: {:?}", e);
            }
        });
        
        self._thread_handle = Some(handle);
        
        tracing::info!("Linux PipeWire audio capture started");
        Ok(())
    }
    
    /// Stop capturing
    pub fn stop(&mut self) -> anyhow::Result<()> {
        // Signal main loop to quit
        if let Some(main_loop) = self.main_loop.take() {
            main_loop.quit();
        }
        
        // Wait for thread
        if let Some(handle) = self._thread_handle.take() {
            let _ = handle.join();
        }
        
        tracing::info!("Linux audio capture stopped");
        Ok(())
    }
}

/// Run PipeWire capture in dedicated thread
fn run_pipewire_capture(sender: mpsc::UnboundedSender<AudioFrame>) -> anyhow::Result<()> {
    // Initialize PipeWire
    pipewire::init();
    
    // Create main loop
    let main_loop = MainLoop::new(None)?;
    let context = Context::new(&main_loop)?;
    let core = context.connect(None)?;
    
    // Audio format: 48kHz stereo float
    let audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::F32LE);
    audio_info.set_rate(48000);
    audio_info.set_channels(2);
    
    // Create capture stream
    let props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Capture",
        *pipewire::keys::MEDIA_ROLE => "Music",
        *pipewire::keys::NODE_NAME => "meeting-scribe-capture",
        *pipewire::keys::STREAM_CAPTURE_SINK => "true", // Capture sink output (system audio)
    };
    
    let stream = Stream::new(&core, "audio-capture", props)?;
    
    // Build params
    let mut params = Vec::new();
    let audio_format_pod = audio_info.build_pod()?;
    params.push(audio_format_pod);
    
    // Stream listener
    let sender_clone = sender.clone();
    let listener = stream
        .add_local_listener::<()>()
        .state_changed(|stream, _old, new| {
            tracing::debug!("PipeWire stream state: {:?}", new);
            match new {
                StreamState::Error(e) => tracing::error!("Stream error: {}", e),
                StreamState::Streaming => tracing::info!("Stream started"),
                _ => {}
            }
        })
        .process(move |stream, _| {
            // Get buffer from stream
            if let Some(buffer) = stream.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if let Some(data) = datas.first_mut() {
                    if let Some(chunk) = data.chunk() {
                        let offset = chunk.offset() as usize;
                        let size = chunk.size() as usize;
                        
                        if let Some(slice) = data.data() {
                            let audio_data = &slice[offset..offset + size];
                            
                            // Convert bytes to f32 samples
                            let samples: Vec<f32> = audio_data
                                .chunks_exact(4)
                                .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                                .collect();
                            
                            let frame = AudioFrame {
                                samples,
                                sample_rate: 48000,
                                channels: 2,
                                timestamp_ns: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_nanos() as u64,
                            };
                            
                            let _ = sender_clone.send(frame);
                        }
                    }
                }
            }
        })
        .register()?;
    
    // Connect stream
    stream.connect(
        Direction::Input,
        None, // No specific node, use default
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
        &mut params.iter().map(|p| p.as_ref()),
    )?;
    
    // Run main loop
    main_loop.run();
    
    Ok(())
}
```

### PulseAudio Fallback

```rust
// src-tauri/src/audio/platform/linux_pulse.rs

use libpulse_binding as pulse;
use libpulse_simple_binding as simple;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Audio frame from PulseAudio
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// PulseAudio capture for older systems
pub struct PulseAudioCapture {
    running: Arc<AtomicBool>,
    sender: mpsc::UnboundedSender<AudioFrame>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl PulseAudioCapture {
    /// Create new PulseAudio capture
    pub fn new() -> anyhow::Result<(Self, mpsc::UnboundedReceiver<AudioFrame>)> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let running = Arc::new(AtomicBool::new(false));
        
        Ok((
            Self {
                running,
                sender,
                thread_handle: None,
            },
            receiver,
        ))
    }
    
    /// Start capturing
    pub fn start(&mut self) -> anyhow::Result<()> {
        self.running.store(true, Ordering::SeqCst);
        
        let running = self.running.clone();
        let sender = self.sender.clone();
        
        let handle = std::thread::spawn(move || {
            if let Err(e) = run_pulse_capture(running, sender) {
                tracing::error!("PulseAudio capture error: {:?}", e);
            }
        });
        
        self.thread_handle = Some(handle);
        
        tracing::info!("PulseAudio capture started");
        Ok(())
    }
    
    /// Stop capturing
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.running.store(false, Ordering::SeqCst);
        
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        
        tracing::info!("PulseAudio capture stopped");
        Ok(())
    }
}

/// Run PulseAudio capture loop
fn run_pulse_capture(
    running: Arc<AtomicBool>,
    sender: mpsc::UnboundedSender<AudioFrame>,
) -> anyhow::Result<()> {
    // Sample specification: 48kHz stereo float
    let spec = Spec {
        format: Format::F32le,
        rate: 48000,
        channels: 2,
    };
    
    assert!(spec.is_valid());
    
    // Connect to PulseAudio
    // Use the monitor source of the default sink for system audio
    let source_name = get_default_monitor_source()?;
    
    let s = simple::Simple::new(
        None,                          // Default server
        "meeting-scribe",              // Application name
        Direction::Record,             // Recording
        Some(&source_name),            // Monitor source
        "system-audio-capture",        // Stream description
        &spec,                         // Sample format
        None,                          // Default channel map
        None,                          // Default buffering
    )
    .map_err(|e| anyhow::anyhow!("Failed to create PulseAudio stream: {:?}", e))?;
    
    // Buffer for reading audio (10ms at 48kHz stereo)
    let buffer_size = (48000 * 2 * 4 * 10) / 1000; // 10ms of audio
    let mut buffer = vec![0u8; buffer_size];
    
    while running.load(Ordering::SeqCst) {
        // Read audio data
        match s.read(&mut buffer) {
            Ok(()) => {
                // Convert bytes to f32 samples
                let samples: Vec<f32> = buffer
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect();
                
                let frame = AudioFrame {
                    samples,
                    sample_rate: 48000,
                    channels: 2,
                };
                
                if sender.send(frame).is_err() {
                    break;
                }
            }
            Err(e) => {
                tracing::error!("PulseAudio read error: {:?}", e);
                break;
            }
        }
    }
    
    Ok(())
}

/// Get the monitor source of the default sink
fn get_default_monitor_source() -> anyhow::Result<String> {
    use pulse::context::Context;
    use pulse::mainloop::standard::Mainloop;
    use std::cell::RefCell;
    use std::rc::Rc;
    
    let mainloop = Rc::new(RefCell::new(
        Mainloop::new().ok_or_else(|| anyhow::anyhow!("Failed to create mainloop"))?,
    ));
    
    let context = Rc::new(RefCell::new(
        Context::new(&*mainloop.borrow(), "meeting-scribe-query")
            .ok_or_else(|| anyhow::anyhow!("Failed to create context"))?,
    ));
    
    // Connect to server
    context
        .borrow_mut()
        .connect(None, pulse::context::FlagSet::NOFLAGS, None)
        .map_err(|e| anyhow::anyhow!("Failed to connect: {:?}", e))?;
    
    // Wait for connection
    loop {
        mainloop.borrow_mut().iterate(true);
        match context.borrow().get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                anyhow::bail!("Context connection failed");
            }
            _ => {}
        }
    }
    
    // Get server info to find default sink
    let monitor_source: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let monitor_source_clone = monitor_source.clone();
    
    let op = context.borrow().introspect().get_server_info(move |info| {
        if let Some(sink_name) = &info.default_sink_name {
            // Monitor source is usually sink_name.monitor
            *monitor_source_clone.borrow_mut() = Some(format!("{}.monitor", sink_name));
        }
    });
    
    // Wait for operation
    while op.get_state() == pulse::operation::State::Running {
        mainloop.borrow_mut().iterate(true);
    }
    
    monitor_source
        .borrow()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Could not find default monitor source"))
}
```

### Unified Linux Capture

```rust
// src-tauri/src/audio/platform/linux_unified.rs

use super::{linux::LinuxAudioCapture, linux_pulse::PulseAudioCapture};
use tokio::sync::mpsc;

/// Audio frame (shared between implementations)
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_ns: u64,
}

/// Unified Linux audio capture with automatic backend selection
pub enum LinuxCapture {
    PipeWire(LinuxAudioCapture),
    PulseAudio(PulseAudioCapture),
}

impl LinuxCapture {
    /// Create capture with best available backend
    pub fn new() -> anyhow::Result<(Self, mpsc::UnboundedReceiver<AudioFrame>)> {
        // Try PipeWire first
        match LinuxAudioCapture::new() {
            Ok((capture, receiver)) => {
                tracing::info!("Using PipeWire for audio capture");
                return Ok((Self::PipeWire(capture), receiver));
            }
            Err(e) => {
                tracing::warn!("PipeWire not available: {:?}, trying PulseAudio", e);
            }
        }
        
        // Fall back to PulseAudio
        match PulseAudioCapture::new() {
            Ok((capture, receiver)) => {
                tracing::info!("Using PulseAudio for audio capture");
                
                // Convert to unified AudioFrame type
                let (sender, unified_receiver) = mpsc::unbounded_channel();
                tokio::spawn(async move {
                    while let Some(frame) = receiver.recv().await {
                        let unified = AudioFrame {
                            samples: frame.samples,
                            sample_rate: frame.sample_rate,
                            channels: frame.channels,
                            timestamp_ns: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_nanos() as u64,
                        };
                        if sender.send(unified).is_err() {
                            break;
                        }
                    }
                });
                
                return Ok((Self::PulseAudio(capture), unified_receiver));
            }
            Err(e) => {
                anyhow::bail!("No audio backend available. PulseAudio error: {:?}", e);
            }
        }
    }
    
    /// Start capturing
    pub fn start(&mut self) -> anyhow::Result<()> {
        match self {
            Self::PipeWire(capture) => capture.start(),
            Self::PulseAudio(capture) => capture.start(),
        }
    }
    
    /// Stop capturing
    pub fn stop(&mut self) -> anyhow::Result<()> {
        match self {
            Self::PipeWire(capture) => capture.stop(),
            Self::PulseAudio(capture) => capture.stop(),
        }
    }
    
    /// Get backend name
    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::PipeWire(_) => "PipeWire",
            Self::PulseAudio(_) => "PulseAudio",
        }
    }
}
```

---

## Unified Platform Abstraction

### Platform Trait

```rust
// src-tauri/src/audio/platform/mod.rs

use tokio::sync::mpsc;

/// Platform-agnostic audio frame
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_ns: u64,
}

/// System audio capture trait
#[async_trait::async_trait]
pub trait SystemAudioCapture: Send + Sync {
    /// Start capturing system audio
    async fn start(&mut self) -> anyhow::Result<()>;
    
    /// Stop capturing
    async fn stop(&mut self) -> anyhow::Result<()>;
    
    /// Check if capturing is active
    fn is_capturing(&self) -> bool;
    
    /// Get platform name
    fn platform_name(&self) -> &'static str;
    
    /// Check if system audio capture is available
    fn is_available() -> bool where Self: Sized;
    
    /// Get required permissions (if any)
    fn required_permissions() -> Vec<Permission> where Self: Sized;
}

/// Permission required for capture
#[derive(Debug, Clone)]
pub struct Permission {
    pub name: String,
    pub description: String,
    pub how_to_grant: String,
}

// Platform-specific modules
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod linux_pulse;
#[cfg(target_os = "linux")]
pub mod linux_unified;

/// Create platform-appropriate system audio capture
pub fn create_system_capture() -> anyhow::Result<(
    Box<dyn SystemAudioCapture>,
    mpsc::UnboundedReceiver<AudioFrame>,
)> {
    #[cfg(target_os = "windows")]
    {
        let (capture, receiver) = windows::WasapiLoopbackCapture::new()?;
        Ok((Box::new(capture), receiver))
    }
    
    #[cfg(target_os = "macos")]
    {
        let (capture, receiver) = macos::MacOSAudioCapture::new()?;
        Ok((Box::new(capture), receiver))
    }
    
    #[cfg(target_os = "linux")]
    {
        let (capture, receiver) = linux_unified::LinuxCapture::new()?;
        Ok((Box::new(capture), receiver))
    }
}

/// Check if system audio capture is available on this platform
pub fn is_system_capture_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        true // WASAPI is always available on Windows
    }
    
    #[cfg(target_os = "macos")]
    {
        macos::check_screen_capture_permission()
    }
    
    #[cfg(target_os = "linux")]
    {
        linux_unified::LinuxCapture::new().is_ok()
    }
}

/// Get required permissions for current platform
pub fn get_required_permissions() -> Vec<Permission> {
    #[cfg(target_os = "windows")]
    {
        vec![] // No special permissions needed
    }
    
    #[cfg(target_os = "macos")]
    {
        vec![Permission {
            name: "Screen Recording".to_string(),
            description: "Required to capture system audio on macOS".to_string(),
            how_to_grant: "System Preferences → Security & Privacy → Privacy → Screen Recording → Enable Meeting Scribe".to_string(),
        }]
    }
    
    #[cfg(target_os = "linux")]
    {
        vec![] // No special permissions, just runtime dependencies
    }
}
```

### Platform Detection Utility

```rust
// src-tauri/src/audio/platform/detect.rs

/// Platform capabilities
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformCapabilities {
    pub platform: String,
    pub system_audio_available: bool,
    pub audio_backend: String,
    pub required_permissions: Vec<PermissionInfo>,
    pub missing_dependencies: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionInfo {
    pub name: String,
    pub granted: bool,
    pub how_to_grant: String,
}

/// Detect platform capabilities
pub fn detect_capabilities() -> PlatformCapabilities {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };
    
    let (system_audio_available, audio_backend, permissions, dependencies) = 
        get_platform_details();
    
    PlatformCapabilities {
        platform: platform.to_string(),
        system_audio_available,
        audio_backend,
        required_permissions: permissions,
        missing_dependencies: dependencies,
    }
}

#[cfg(target_os = "windows")]
fn get_platform_details() -> (bool, String, Vec<PermissionInfo>, Vec<String>) {
    (true, "WASAPI".to_string(), vec![], vec![])
}

#[cfg(target_os = "macos")]
fn get_platform_details() -> (bool, String, Vec<PermissionInfo>, Vec<String>) {
    use super::macos::check_screen_capture_permission;
    
    let permission_granted = check_screen_capture_permission();
    
    let permissions = vec![PermissionInfo {
        name: "Screen Recording".to_string(),
        granted: permission_granted,
        how_to_grant: "System Preferences → Privacy & Security → Screen Recording".to_string(),
    }];
    
    (permission_granted, "ScreenCaptureKit".to_string(), permissions, vec![])
}

#[cfg(target_os = "linux")]
fn get_platform_details() -> (bool, String, Vec<PermissionInfo>, Vec<String>) {
    use std::process::Command;
    
    let mut missing = vec![];
    let mut backend = "unknown".to_string();
    
    // Check for PipeWire
    let has_pipewire = Command::new("pidof")
        .arg("pipewire")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    // Check for PulseAudio
    let has_pulse = Command::new("pidof")
        .arg("pulseaudio")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    if has_pipewire {
        backend = "PipeWire".to_string();
    } else if has_pulse {
        backend = "PulseAudio".to_string();
    } else {
        missing.push("PipeWire or PulseAudio required".to_string());
    }
    
    // Check for required libraries
    let lib_check = Command::new("ldconfig")
        .arg("-p")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    
    if !lib_check.contains("libpipewire") && has_pipewire {
        missing.push("libpipewire-0.3-dev".to_string());
    }
    
    (missing.is_empty(), backend, vec![], missing)
}
```

---

## Build Configuration

### Cargo.toml Platform Features

```toml
# src-tauri/Cargo.toml

[package]
name = "meeting-scribe"
version = "0.1.0"
edition = "2021"

[features]
default = []
# Enable all platform audio backends (for development)
all-audio-backends = []

[dependencies]
# ... common dependencies ...

# Audio processing (all platforms)
cpal = "0.15"
hound = "3.5"
ringbuf = "0.4"
rubato = "0.15"

# Platform-agnostic
tokio = { version = "1.37", features = ["full"] }
async-trait = "0.1"

# ============================================
# Windows-specific dependencies
# ============================================
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Media_Audio",
    "Win32_System_Com",
    "Win32_System_Threading",
    "Win32_Foundation",
]}

# ============================================
# macOS-specific dependencies
# ============================================
[target.'cfg(target_os = "macos")'.dependencies]
cidre = { version = "0.4", features = ["sc", "cm", "av"] }
core-graphics = "0.24"
dispatch = "0.2"

# Alternative: objc2 ecosystem
# objc2 = "0.5"
# objc2-foundation = "0.2"
# objc2-screen-capture-kit = "0.2"

# ============================================
# Linux-specific dependencies
# ============================================
[target.'cfg(target_os = "linux")'.dependencies]
pipewire = "0.8"
libpulse-binding = "2.28"
libpulse-simple-binding = "2.28"

[target.'cfg(target_os = "linux")'.build-dependencies]
pkg-config = "0.3"
```

### Build Script for Linux

```rust
// src-tauri/build.rs

fn main() {
    // Standard Tauri build
    tauri_build::build();
    
    // Linux-specific: Check for required libraries
    #[cfg(target_os = "linux")]
    {
        check_linux_dependencies();
    }
}

#[cfg(target_os = "linux")]
fn check_linux_dependencies() {
    use std::process::Command;
    
    // Check for pkg-config
    let pkg_config = Command::new("pkg-config")
        .arg("--version")
        .output();
    
    if pkg_config.is_err() {
        println!("cargo:warning=pkg-config not found. Install with: sudo apt install pkg-config");
    }
    
    // Check for PipeWire
    let pipewire = Command::new("pkg-config")
        .args(["--exists", "libpipewire-0.3"])
        .status();
    
    if pipewire.map(|s| !s.success()).unwrap_or(true) {
        println!("cargo:warning=libpipewire-0.3 not found. Install with: sudo apt install libpipewire-0.3-dev");
    }
    
    // Check for PulseAudio
    let pulse = Command::new("pkg-config")
        .args(["--exists", "libpulse"])
        .status();
    
    if pulse.map(|s| !s.success()).unwrap_or(true) {
        println!("cargo:warning=libpulse not found. Install with: sudo apt install libpulse-dev");
    }
    
    // Link libraries
    println!("cargo:rustc-link-lib=pipewire-0.3");
    println!("cargo:rustc-link-lib=pulse");
    println!("cargo:rustc-link-lib=pulse-simple");
}
```

### GitHub Actions Workflow

```yaml
# .github/workflows/build.yml

name: Build

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        
      - name: Install pnpm
        run: npm install -g pnpm
        
      - name: Install dependencies
        run: pnpm install
        
      - name: Build
        run: pnpm tauri build
        
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: windows-build
          path: src-tauri/target/release/bundle/

  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin, x86_64-apple-darwin
          
      - name: Install pnpm
        run: npm install -g pnpm
        
      - name: Install dependencies
        run: pnpm install
        
      # Build for Apple Silicon
      - name: Build (ARM64)
        run: pnpm tauri build --target aarch64-apple-darwin
        
      # Build for Intel
      - name: Build (x64)
        run: pnpm tauri build --target x86_64-apple-darwin
        
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: macos-build
          path: src-tauri/target/*/release/bundle/

  build-linux:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        
      - name: Install Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libappindicator3-dev \
            librsvg2-dev \
            patchelf \
            libpipewire-0.3-dev \
            libpulse-dev \
            libspa-0.2-dev \
            pipewire
            
      - name: Install pnpm
        run: npm install -g pnpm
        
      - name: Install dependencies
        run: pnpm install
        
      - name: Build
        run: pnpm tauri build
        
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: linux-build
          path: src-tauri/target/release/bundle/
```

---

## Testing Strategy

### Unit Tests

```rust
// src-tauri/src/audio/platform/tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_platform_detection() {
        let caps = detect_capabilities();
        
        #[cfg(target_os = "windows")]
        assert_eq!(caps.platform, "windows");
        
        #[cfg(target_os = "macos")]
        assert_eq!(caps.platform, "macos");
        
        #[cfg(target_os = "linux")]
        assert_eq!(caps.platform, "linux");
    }
    
    #[test]
    fn test_audio_frame_conversion() {
        // Test stereo to mono conversion
        let stereo = vec![0.5f32, -0.5, 0.3, -0.3, 0.1, -0.1];
        let mono = stereo_to_mono(&stereo);
        
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.0).abs() < 0.001); // (0.5 + -0.5) / 2
        assert!((mono[1] - 0.0).abs() < 0.001);
        assert!((mono[2] - 0.0).abs() < 0.001);
    }
    
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_wasapi_capture() {
        let result = windows::WasapiLoopbackCapture::new();
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn test_screencapturekit_permission() {
        // Just check that permission check doesn't crash
        let _ = macos::check_screen_capture_permission();
    }
    
    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_linux_backend_detection() {
        // Should find either PipeWire or PulseAudio
        let result = linux_unified::LinuxCapture::new();
        // This might fail in CI without audio services, so just check no panic
        let _ = result;
    }
}

fn stereo_to_mono(stereo: &[f32]) -> Vec<f32> {
    stereo
        .chunks_exact(2)
        .map(|chunk| (chunk[0] + chunk[1]) * 0.5)
        .collect()
}
```

### Integration Tests

```rust
// src-tauri/tests/audio_integration.rs

use std::time::Duration;
use tokio::time::timeout;

/// Test that we can start and stop capture without crashing
#[tokio::test]
async fn test_capture_lifecycle() {
    let result = meeting_scribe::audio::platform::create_system_capture();
    
    // This might fail on CI without audio hardware
    if let Ok((mut capture, mut receiver)) = result {
        // Start capture
        if capture.start().await.is_ok() {
            // Wait briefly for some audio
            let _ = timeout(Duration::from_millis(500), receiver.recv()).await;
            
            // Stop capture
            let _ = capture.stop().await;
        }
    }
}

/// Test audio format conversion
#[tokio::test]
async fn test_audio_resampling() {
    use meeting_scribe::audio::resample::Resampler;
    
    // Create 48kHz input
    let sample_rate = 48000;
    let duration_ms = 100;
    let samples: Vec<f32> = (0..sample_rate * duration_ms / 1000)
        .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / sample_rate as f32).sin())
        .collect();
    
    // Resample to 16kHz
    let mut resampler = Resampler::new(48000, 16000).unwrap();
    let output = resampler.process(&samples).unwrap();
    
    // Output should be 1/3 the length
    let expected_len = samples.len() / 3;
    assert!((output.len() as i32 - expected_len as i32).abs() < 10);
}
```

### Manual Test Checklist

```markdown
## Platform Testing Checklist

### Windows
- [ ] System audio capture works with speakers
- [ ] System audio capture works with headphones
- [ ] Microphone capture works simultaneously
- [ ] No audio glitches or dropouts over 1 hour
- [ ] Works with different audio sample rates

### macOS
- [ ] Permission dialog appears on first run
- [ ] Capture works after granting permission
- [ ] Capture fails gracefully if permission denied
- [ ] Works on Intel Macs
- [ ] Works on Apple Silicon Macs
- [ ] Works with AirPods and Bluetooth audio

### Linux
- [ ] PipeWire backend works (Ubuntu 22.04+, Fedora)
- [ ] PulseAudio fallback works (older systems)
- [ ] Works with ALSA output
- [ ] Works with Bluetooth audio
- [ ] No permissions required
- [ ] Build works with missing optional dependencies
```

---

## Troubleshooting

### Windows Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "No audio devices found" | Audio service not running | Restart Windows Audio service |
| Silent audio | Wrong device selected | Use `list_audio_devices()` to find correct device |
| High latency | Large buffer size | Reduce buffer in WASAPI config |

### macOS Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "Permission denied" | Screen Recording not granted | Open System Preferences → Privacy → Screen Recording |
| App not in list | Need to trigger permission | Run app once, restart System Preferences |
| Works in dev, not release | Missing entitlements | Check entitlements.plist is included |
| Notarization fails | Missing entitlements | Add hardened runtime + required entitlements |

```bash
# Reset Screen Recording permissions (for testing)
tccutil reset ScreenCapture com.meeting-scribe

# Check current permissions
sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db \
  "SELECT * FROM access WHERE service='kTCCServiceScreenCapture'"
```

### Linux Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| "PipeWire not found" | Service not running | `systemctl --user start pipewire` |
| "No monitor source" | PulseAudio misconfigured | `pactl load-module module-loopback` |
| Build fails | Missing dev packages | Install `libpipewire-0.3-dev libpulse-dev` |
| No system audio | Monitor not available | Check `pactl list sources` for .monitor |

```bash
# Check PipeWire status
systemctl --user status pipewire pipewire-pulse

# List available audio sources
pactl list sources short

# Test audio capture
pw-record --target=<sink-id> test.wav
```

---

## Acceptance Criteria

- [ ] System audio captures on all three platforms
- [ ] macOS permission flow is user-friendly
- [ ] Linux works with both PipeWire and PulseAudio
- [ ] Unified API hides platform differences
- [ ] Build succeeds on all platforms in CI
- [ ] Audio quality matches Windows baseline
- [ ] No memory leaks during long captures
- [ ] Graceful fallback when capture unavailable

---

## Next Steps

After completing cross-platform audio:

1. **Document 11: Deployment** - Build optimization, installers, distribution
2. **Testing** - Platform-specific test matrices
3. **Documentation** - User guides for platform-specific setup

---

## References

### macOS
- [ScreenCaptureKit WWDC 2022](https://developer.apple.com/videos/play/wwdc2022/10156/)
- [cidre crate examples](https://github.com/aspect-rs/cidre/tree/main/examples)
- [screenpipe macOS](https://github.com/mediar-ai/screenpipe)

### Linux
- [PipeWire Wiki](https://gitlab.freedesktop.org/pipewire/pipewire/-/wikis/home)
- [pipewire-rs examples](https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/tree/main/pipewire/examples)
- [PulseAudio Documentation](https://www.freedesktop.org/wiki/Software/PulseAudio/Documentation/)

### Cross-Platform
- [Tauri Platform-Specific Code](https://tauri.app/v2/guides/building/cross-platform/)
- [Rust Conditional Compilation](https://doc.rust-lang.org/reference/conditional-compilation.html)
