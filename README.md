# Meeting Scribe

<p align="left">
  <img src="src/assets/branding/meeting-scribe-logo.svg" alt="Meeting Scribe logo" width="420" />
</p>

Meeting Scribe is a **privacy-first desktop app** for recording, transcribing, summarizing, and searching meeting content. All inference runs locally using open-source models.

**Stack:** Rust + Tauri v2 + React + TypeScript

## Quick Start

```bash
pnpm install
pnpm tauri dev
```

## Features

- Record microphone + system audio (Windows WASAPI, macOS CoreAudio Process Tap with loopback fallback, Linux PipeWire/Pulse monitor input)
- Local transcription (default: Parakeet via `transcribe-rs`)
- Local summaries + action items (llama.cpp via `llama-cpp-2`, generated in background tasks)
- Semantic search + RAG chat (EmbeddingGemma + LanceDB)
- SQLite storage for meetings, transcripts, notes, summaries

## Development

### Prerequisites

- Node.js + `pnpm`
- Rust toolchain (stable)
- Platform build tools
  - Windows: MSVC build tools
  - macOS: Xcode Command Line Tools (full Xcode required for some bundle/signing flows)

Some Rust deps may require additional tooling depending on your environment:

- `protoc` (protobuf compiler)
- `cmake` + `ninja` (for crates that compile native code)

### Core Commands

```bash
pnpm tauri dev
pnpm tauri build
pnpm tauri build --debug
```

`pnpm tauri ...` now stages the same pinned ONNX Runtime binaries used by release builds into `src-tauri/resources/runtime` before invoking Tauri. The first run on a machine may download these files.
On macOS, staged ONNX dylibs are re-signed with the active app signing identity (default ad-hoc `-`) so bundled apps can load them under hardened runtime.
On macOS, local builds also enforce `11.0` deployment target for Cargo/CMake-native dependencies (including `llama-cpp-sys`) to match bundle support policy.
On macOS `pnpm tauri build ...` runs preflight cleanup for stale `llama-cpp-sys` CMake caches and stale project-owned interstitial DMG mounts (`rw.*.dmg` artifacts/attachments) left by failed packaging runs.
macOS bundle signing uses `src-tauri/entitlements.plist`, which explicitly disables strict library validation so ONNX runtime dylibs bundled in app resources can be loaded reliably.

### macOS Notes
- Grant Microphone permission when prompted.
- Grant System Audio Capture permission when prompted (macOS Process Tap).
- On macOS 14.2+, app tries native CoreAudio Process Tap first.
- Dev-mode caveat: when launched from some terminal apps, macOS may attribute audio-capture permission to the terminal bundle. If that bundle does not declare `NSAudioCaptureUsageDescription`, Process Tap will be denied and output silence.
- If this happens, run a built `.app` bundle (`pnpm tauri build --debug`, then open the app from Finder) and grant System Audio Recording to `meeting-scribe`.
- If Process Tap is unavailable, install a loopback input device (BlackHole, Loopback, Soundflower, or Background Music) as fallback.
- Optionally set `MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE` to force a specific input device match.
- Optional backend override: `MEETING_SCRIBE_MACOS_SYSTEM_AUDIO_BACKEND=process_tap|loopback|auto`.
- Optional echo backend override for transcription cleanup: `MEETING_SCRIBE_ECHO_BACKEND=webrtc_aec3|speex`.
- Optional real-time cleanup backend override (recording-time AEC): `MEETING_SCRIBE_REALTIME_ECHO_BACKEND=webrtc_aec3|speex` (macOS defaults to WebRTC if unset).
- Optional LLM GPU override: `MEETING_SCRIBE_LLM_GPU_LAYERS=<n>` (`0` forces CPU-only; unset tries GPU first and falls back as needed).

Windows convenience script (sets a few env vars before `pnpm tauri dev`):

```bat
dev.bat
```

### Lint / Typecheck / Tests

```bash
pnpm lint
npx tsc --noEmit

cd src-tauri
cargo test
cargo clippy
```

### Dependency Hygiene

Use these when doing maintenance/refactors:

```bash
# Frontend dependency scan (note: can report false positives for PostCSS/Tailwind config usage)
pnpm dlx depcheck

# Rust dependency scan
cd src-tauri
cargo machete --with-metadata --skip-target-dir
```

## CI/CD And GitHub Releases

This repo ships with GitHub Actions workflows in `.github/workflows/`:

- `ci.yml`: runs lint, frontend build, and Rust tests on pushes/PRs.
- `release.yml`: builds precompiled desktop bundles for macOS (Intel + Apple Silicon), Windows x64, and Linux x64, then uploads them to a GitHub Release.

### Create a precompiled GitHub release

1. Commit and push your changes to `main`.
2. Create and push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

3. GitHub Actions will run the `Release` workflow and attach built artifacts to the `v0.1.0` release.

## Reliability Notes

- `process_meeting` now fails fast when no audio paths are provided, when referenced files are missing, or when both channels produce no transcribable segments. This avoids silent "success" states with empty transcripts.
- FTS queries are sanitized and normalized. Empty/symbol-only inputs now return an empty result set instead of surfacing SQLite FTS syntax errors.
- AEC now aligns system-audio reference timing to mic audio before cancellation, which improves feedback suppression when capture streams start with offset.
- Echo cancellation now supports `WebRTC AEC3` (default) with automatic `SpeexDSP` fallback for robustness across platforms/environments.
- Mic feedback suppression runs in two stages after AEC: signal-level residual echo attenuation, then transcript-level dedupe of likely echoed mic segments that closely match overlapping system speech.
- When system audio is available, `process_meeting` writes cleaned mic audio (`*_clean.wav`) and updates meeting playback to that file. Raw capture is retained for future reprocessing quality.

## System Audio Backends

- Windows system audio capture uses WASAPI loopback.
- macOS capture uses CoreAudio Process Tap (native) with loopback-input fallback.
- Linux capture uses PipeWire/Pulse monitor input devices (for example `Monitor of ...` sources).
- Optional device override: `MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE=<device name>`

## Data & Storage

On first run the backend creates a local data directory (see `src-tauri/src/lib.rs` `AppConfig`).

Layout (approx):

```
<data_dir>/
  data/
    meetings.db      # SQLite
    vectors/         # LanceDB
  audio/<meeting_id>/
    you.wav
    you_clean.wav   # optional: generated after AEC processing
    others.wav
  models/
    transcription/
    embedding/
    llm/
```

Deleting a meeting removes:

- The meeting row (and cascaded transcripts/notes/summaries) from SQLite
- The meeting’s embeddings from LanceDB
- The meeting’s audio directory from disk (best-effort)

## Troubleshooting

- **Transcription/embeddings fail to load in local dev builds**:
  - This repo uses the `ort` crate with `load-dynamic`.
  - Cargo config pins `ORT_LIB_LOCATION` to `src-tauri/resources/runtime` and disables `libonnxruntime` pkg-config probing to prevent accidental hard-linking to Homebrew dylibs.
  - Release bundles now include ONNX Runtime and the app auto-detects bundled runtime paths on startup.
  - For ad-hoc/dev runs, if runtime discovery fails, set `ORT_DYLIB_PATH` explicitly.
- **Linux system-audio capture fails to start**:
  - Ensure your audio stack exposes a monitor input source (PipeWire/PulseAudio monitor device).
  - Use `MEETING_SCRIBE_SYSTEM_AUDIO_DEVICE` to force a specific monitor/loopback input device name if auto-detect picks the wrong one.
- **Models show “not downloaded”**: download them in Settings first, then click “Load”.
