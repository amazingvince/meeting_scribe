# System Audio Backends

## Current State

- Windows: `WASAPI loopback` implemented in `src-tauri/src/audio/platform/windows.rs`.
- macOS: `CoreAudio Process Tap` primary backend in `src-tauri/src/audio/platform/macos.rs`.
  - Native speaker-output capture on macOS 14.2+ via a Swift helper (`macos_process_tap_helper.swift`).
  - Automatic fallback to virtual loopback input devices (BlackHole/Loopback/Soundflower/Background Music).
  - Optional device override via `MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE`.
  - Optional microphone override via `MEETING_SCRIBE_MIC_DEVICE` when default input is a virtual loopback device.
  - Optional backend override via `MEETING_SCRIBE_MACOS_SYSTEM_AUDIO_BACKEND=process_tap|loopback|auto`.
- Linux: `PipeWire/Pulse monitor input` backend implemented in `src-tauri/src/audio/platform/linux.rs`.
  - Auto-selects monitor/loopback input devices by name heuristics (`Monitor of ...`, `.monitor`, `loopback`, etc.).
  - Optional device override via `MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE`.

## Backend Contract (must stay stable)

Each platform backend should keep the same `SystemAudioCapture` behavior:

1. Output mono `f32` samples at `16kHz`.
2. Use non-blocking internal buffering (`AudioBuffer`).
3. Support `new()`, `start()`, `stop()`, `is_running()`, `buffer()`.
4. Never panic on missing permissions/devices; return actionable errors.

## Cross-Platform Plan

1. Add capability probing
- Report backend name and support status at runtime.
- Surface dependency/permission requirements to logs/UI.

2. Implement macOS backend
- Completed with CoreAudio Process Tap + loopback fallback strategy.
- Future enhancement: optional ScreenCaptureKit backend for older macOS compatibility paths.

3. Harden Linux backend compatibility
- Expand monitor-device heuristics as we see distro/driver variants in the field.
- Add optional native PipeWire graph capture path if CPAL monitor devices are missing.

4. Add backend validation tests
- Start/stop lifecycle tests (best-effort integration tests).
- Resample/format correctness tests.
- Error-path tests (missing permission, missing backend dependency).

## Quality Gates

- AEC must remain optional and never block recording.
- System capture failure must still allow mic-only recording.
- Backend-specific failures should include:
- backend name
- missing permission/dependency hint
- suggested remediation

## Echo Cancellation Backends

- Primary: `WebRTC AEC3` via `aec3` crate (pure Rust, cross-platform).
- Fallback: `SpeexDSP` via `aec-rs`.
- Selection:
  - Settings UI: `Audio Capture And Echo Control -> Echo Cancellation Backend`
  - Env override: `MEETING_SCRIBE_ECHO_BACKEND=webrtc_aec3|speex`
