# Meeting Scribe - Developer Planning Guide

> **Project:** Local-first meeting transcription, summarization, and RAG app  
> **Stack:** Rust + Tauri v2 + React + TypeScript  
> **Target Platforms:** Windows (primary), macOS, Linux  
> **Estimated Timeline:** 12 weeks to MVP

---

## 📋 Document Index

This planning guide is broken into sequential development phases. Follow these documents in order for a structured build approach.

| # | Document | Description | Est. Time |
|---|----------|-------------|-----------|
| 01 | [Project Setup](./01-project-setup.md) | Tauri + React scaffolding, tooling, initial structure | 3-4 days |
| 02 | [Audio Capture](./02-audio-capture.md) | Microphone and system audio capture with `cpal` | 5-6 days |
| 03 | [Audio Preprocessing](./03-audio-preprocessing.md) | VAD (Silero), denoising (nnnoiseless), resampling | 4-5 days |
| 04 | [Transcription Engine](./04-transcription-engine.md) | `transcribe-rs` integration with Parakeet/Whisper | 5-6 days |
| 05 | [Storage Layer](./05-storage-layer.md) | SQLite schema, migrations, LanceDB vectors | 4-5 days |
| 06 | [Embedding Engine](./06-embedding-engine.md) | ONNX Runtime + EmbeddingGemma for vectors | 4-5 days |
| 07 | [LLM Engine](./07-llm-engine.md) | `llama-cpp-2` for summarization and chat | 4-5 days |
| 08 | [Frontend UI](./08-frontend-ui.md) | React components, state management, Tauri IPC | 8-10 days |
| 09 | [RAG Implementation](./09-rag-implementation.md) | Vector search, retrieval, chat interface | 5-6 days |
| 10 | [Cross-Platform](./10-cross-platform.md) | macOS (ScreenCaptureKit), Linux (PipeWire) | 6-8 days |
| 11 | [Deployment](./11-deployment.md) | Build optimization, installers, CI/CD | 4-5 days |

---

## 🎯 Project Goals

### Privacy-First Design
- **All processing happens locally** - no cloud dependencies
- Audio, transcripts, and embeddings never leave the user's machine
- Models are downloaded once and run offline

### Core Features (MVP)
- ✅ Dual audio capture (microphone + system audio)
- ✅ Voice Activity Detection for efficient processing
- ✅ Post-meeting transcription with speaker labels ("you" vs "others")
- ✅ LLM-powered summarization (key points, action items)
- ✅ Vector search (RAG) across all meeting history
- ✅ Meeting library with timeline view

### V2 Features (Future)
- ⏳ Live transcription during recording
- ⏳ Speaker diarization (multiple speaker identification)
- ⏳ Export to PDF/Markdown
- ⏳ Calendar integration

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         TAURI SHELL                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    REACT FRONTEND                           │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │ │
│  │  │Recording │ │ Library  │ │ Detail   │ │   Chat   │      │ │
│  │  │  View    │ │(Timeline)│ │  Editor  │ │Interface │      │ │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘      │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              │                                   │
│                     Tauri Commands (IPC)                         │
│                              │                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                     RUST BACKEND                            │ │
│  │                                                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │ │
│  │  │Audio Capture│  │ Inference   │  │  Storage    │        │ │
│  │  │  Pipeline   │  │ Subsystem   │  │ Subsystem   │        │ │
│  │  │             │  │             │  │             │        │ │
│  │  │ cpal + VAD  │  │transcribe-rs│  │  SQLite +   │        │ │
│  │  │ nnnoiseless │  │ort + llama  │  │  LanceDB    │        │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘        │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🔧 Technology Stack

### Backend (Rust)

| Component | Library | Purpose | Docs |
|-----------|---------|---------|------|
| Framework | Tauri v2 | Desktop app shell | [tauri.app/v2](https://tauri.app/v2/) |
| Audio Capture | cpal | Cross-platform audio | [docs.rs/cpal](https://docs.rs/cpal/latest/cpal/) |
| VAD | voice_activity_detector | Silero VAD v5 | [crates.io](https://crates.io/crates/voice_activity_detector) |
| Denoising | nnnoiseless | RNNoise port | [docs.rs/nnnoiseless](https://docs.rs/nnnoiseless/latest/nnnoiseless/) |
| Resampling | rubato | Audio resampling | [docs.rs/rubato](https://docs.rs/rubato/latest/rubato/) |
| Transcription | transcribe-rs | Parakeet/Whisper wrapper | [GitHub](https://github.com/cjpais/transcribe-rs) |
| Embeddings | ort | ONNX Runtime | [docs.rs/ort](https://docs.rs/ort/latest/ort/) |
| LLM | llama-cpp-2 | llama.cpp bindings | [docs.rs/llama-cpp-2](https://docs.rs/llama-cpp-2/latest/llama_cpp_2/) |
| SQL Database | rusqlite | SQLite bindings | [docs.rs/rusqlite](https://docs.rs/rusqlite/latest/rusqlite/) |
| Vector DB | lancedb | Embedded vectors | [lancedb.github.io](https://lancedb.github.io/lancedb/) |
| Ring Buffers | ringbuf | Lock-free buffers | [docs.rs/ringbuf](https://docs.rs/ringbuf/latest/ringbuf/) |

### Frontend (TypeScript/React)

| Component | Library | Purpose | Docs |
|-----------|---------|---------|------|
| Framework | React 18 | UI framework | [react.dev](https://react.dev/) |
| Build Tool | Vite | Fast bundler | [vitejs.dev](https://vitejs.dev/) |
| State | Zustand | State management | [zustand](https://github.com/pmndrs/zustand) |
| Styling | Tailwind CSS | Utility CSS | [tailwindcss.com](https://tailwindcss.com/) |
| Tauri API | @tauri-apps/api | IPC bindings | [tauri.app/v2/reference/js](https://tauri.app/v2/reference/js/) |

---

## 📁 Project Structure

```
meeting-scribe/
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── audio/               # Audio capture & processing
│   │   │   ├── mod.rs
│   │   │   ├── capture.rs       # cpal integration
│   │   │   ├── buffer.rs        # Ring buffer management
│   │   │   ├── vad.rs           # Voice activity detection
│   │   │   ├── denoise.rs       # nnnoiseless integration
│   │   │   └── platform/        # Platform-specific code
│   │   │       ├── windows.rs   # WASAPI loopback
│   │   │       ├── macos.rs     # ScreenCaptureKit
│   │   │       └── linux.rs     # PipeWire
│   │   ├── inference/           # ML model inference
│   │   │   ├── mod.rs
│   │   │   ├── transcription.rs # transcribe-rs wrapper
│   │   │   ├── embedding.rs     # ONNX embeddings
│   │   │   └── llm.rs           # llama-cpp-2 wrapper
│   │   ├── storage/             # Data persistence
│   │   │   ├── mod.rs
│   │   │   ├── sqlite.rs        # SQLite operations
│   │   │   └── vectors.rs       # LanceDB operations
│   │   ├── models/              # Model management
│   │   │   ├── mod.rs
│   │   │   └── downloader.rs    # Download from HuggingFace
│   │   └── commands/            # Tauri commands (IPC)
│   │       ├── mod.rs
│   │       ├── recording.rs
│   │       ├── meetings.rs
│   │       └── chat.rs
├── src/                          # React frontend
│   ├── App.tsx
│   ├── main.tsx
│   ├── components/
│   │   ├── Recording/           # Recording view components
│   │   ├── Library/             # Meeting library components
│   │   ├── MeetingDetail/       # Detail/editor components
│   │   ├── Chat/                # RAG chat components
│   │   └── Settings/            # Settings components
│   ├── hooks/                   # Custom React hooks
│   ├── stores/                  # Zustand stores
│   ├── types/                   # TypeScript types
│   └── styles/                  # Global styles
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── README.md
```

---

## 🚀 Quick Start (After completing 01-project-setup.md)

```bash
# Clone the repo
git clone https://github.com/your-org/meeting-scribe.git
cd meeting-scribe

# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

---

## 📚 Key Reference Projects

These open-source projects were studied for architecture decisions:

| Project | Stars | What We Learned |
|---------|-------|-----------------|
| [screenpipe](https://github.com/mediar-ai/screenpipe) | 16.1k | Audio capture patterns, VAD integration, FFmpeg encoding |
| [Meetily](https://github.com/Zackriya-Solutions/meeting-minutes) | 8.2k | Tauri + Rust stack, Whisper integration patterns |
| [transcribe-rs](https://github.com/cjpais/transcribe-rs) | - | Unified transcription API (our choice) |
| [voice_activity_detector](https://github.com/nkeenan38/voice_activity_detector) | - | Silero VAD v5 Rust wrapper |
| [nnnoiseless](https://github.com/jneem/nnnoiseless) | - | Pure Rust RNNoise port |

---

## ⚠️ Known Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| macOS ScreenCaptureKit complexity | Medium | High | Start with Windows, use cidre crate |
| Linux audio capture (PipeWire) | Medium | Medium | Provide PulseAudio fallback |
| transcribe-rs GPU issues | Medium | High | Multiple engine fallbacks |
| Model download sizes | Low | Low | Progress UI, resume support |
| Memory usage spikes | Medium | Medium | Streaming pipeline, profiling |

---

## 📖 How to Use This Guide

1. **Read sequentially** - Each document builds on the previous
2. **Check prerequisites** - Each doc lists what must be complete first
3. **Follow the acceptance criteria** - Each section has testable criteria
4. **Use the code samples** - Examples are production-ready starting points
5. **Reference the links** - External docs are linked for deep dives

---

## 🔗 Essential External Documentation

### Tauri v2
- [Getting Started](https://tauri.app/v2/guide/)
- [Commands (IPC)](https://tauri.app/v2/guide/command/)
- [Events](https://tauri.app/v2/guide/event/)
- [State Management](https://tauri.app/v2/guide/state-management/)

### Audio Processing
- [cpal Examples](https://github.com/RustAudio/cpal/tree/master/examples)
- [WASAPI Loopback Guide](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
- [Silero VAD Docs](https://github.com/snakers4/silero-vad)

### ML/Inference
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- [llama.cpp](https://github.com/ggerganov/llama.cpp)
- [ONNX Runtime](https://onnxruntime.ai/docs/)
- [EmbeddingGemma on HuggingFace](https://huggingface.co/google/embeddinggemma-300m)

---

*Continue to [01-project-setup.md](./01-project-setup.md) →*
