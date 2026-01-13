# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Meeting Scribe is a **privacy-first desktop application** for recording, transcribing, summarizing, and searching meeting content. All processing happens locally using open-source models.

**Stack:** Rust + Tauri v2 + React + TypeScript
**Target Platforms:** Windows (primary), macOS, Linux
**Status:** Core functionality complete (Phases 1-7), UI and RAG in progress

## Build Commands

```bash
# Install frontend dependencies
pnpm install

# Development mode
pnpm tauri dev

# Production build (current platform)
pnpm tauri build

# Debug build
pnpm tauri build --debug

# Rust tests
cd src-tauri && cargo test

# Rust linting
cd src-tauri && cargo clippy

# Frontend linting
pnpm lint

# Type check
npx tsc --noEmit

# Format code
pnpm format
cd src-tauri && cargo fmt
```

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                       TAURI SHELL                            │
├──────────────────────────────────────────────────────────────┤
│  REACT FRONTEND (src/)                                       │
│  ├── components/                                             │
│  │   ├── Chat/        ChatView, ChatInput, ChatMessage       │
│  │   ├── Library/     LibraryView, MeetingCard, ModelSelector│
│  │   ├── Meeting/     MeetingView, TranscriptPanel, etc.     │
│  │   ├── Settings/    SettingsView, ModelSettings, Storage   │
│  │   └── ui/          Button, Card, Modal, Badge, etc.       │
│  ├── hooks/           useMeetings, useModels, useRecording   │
│  ├── stores/          Zustand stores (settings, toast)       │
│  ├── lib/tauri.ts     Typed IPC command wrappers             │
│  └── types/           TypeScript type definitions            │
│                        │                                     │
│               Tauri Commands (IPC)                           │
│                        │                                     │
│  RUST BACKEND (src-tauri/src/)                              │
│  ├── audio/           Audio capture & preprocessing          │
│  │   ├── capture.rs   cpal integration                       │
│  │   ├── buffer.rs    Ring buffer management                 │
│  │   ├── vad.rs       Silero VAD v5                         │
│  │   ├── denoise.rs   nnnoiseless (RNNoise)                 │
│  │   └── platform/    WASAPI/ScreenCaptureKit/PipeWire      │
│  ├── inference/       ML model inference                     │
│  │   ├── transcription.rs  transcribe-rs (Parakeet/Whisper) │
│  │   ├── embedding.rs      ONNX + EmbeddingGemma            │
│  │   └── llm.rs            llama-cpp-2                      │
│  ├── storage/         Data persistence                       │
│  │   ├── sqlite.rs    rusqlite (meetings, transcripts)      │
│  │   └── vectors.rs   LanceDB (embeddings)                  │
│  ├── models/          Model download management              │
│  └── commands/        Tauri IPC handlers                     │
└──────────────────────────────────────────────────────────────┘
```

## Key Technology Choices

| Component | Library | Notes |
|-----------|---------|-------|
| Transcription | `transcribe-rs` | Parakeet (4x faster, default) or Whisper |
| VAD | `voice_activity_detector` | Silero VAD v5 via ONNX |
| Denoising | `nnnoiseless` | Pure Rust RNNoise, requires 48kHz |
| Embeddings | `ort` | ONNX Runtime + EmbeddingGemma 300M |
| LLM | `llama-cpp-2` | Summarization and RAG chat |
| SQL DB | `rusqlite` | Meetings, transcripts, settings |
| Vector DB | `lancedb` | 768-dim embeddings for RAG |
| Audio Capture | `cpal` | Cross-platform, plus platform-specific loopback |
| State Management | Zustand | React frontend state |

## Audio Pipeline Flow

```
Mic Input (cpal) ─────┐
                      ├──► Ring Buffers ──► Resample (rubato)
System Audio ─────────┘    (ringbuf)        to 48kHz
(platform-specific)                             │
                                                ▼
                                         Denoise (nnnoiseless)
                                                │
                                                ▼
                                         Resample to 16kHz
                                                │
                                                ▼
                                         VAD (Silero v5)
                                                │
                                                ▼
                                         Speech Chunks
                                                │
                                                ▼
                                         Transcription
```

## Audio Format Constants

```rust
// Whisper/transcription requires 16kHz mono
WHISPER_SAMPLE_RATE: 16000
// nnnoiseless requires 48kHz
DENOISE_SAMPLE_RATE: 48000
// Silero VAD chunk sizes
VAD_CHUNK_SIZE_16K: 512  // Only valid size for 16kHz
```

## Data Storage

```
~/.meeting-scribe/
├── data/
│   ├── meetings.db           # SQLite
│   └── vectors/              # LanceDB
├── audio/{meeting_id}/
│   ├── you.wav / you.opus
│   └── others.wav / others.opus
├── models/
│   ├── parakeet/
│   ├── whisper/
│   ├── embedding/
│   └── llm/
└── cache/
```

## Platform-Specific Audio

| Platform | Mic | System Audio Loopback |
|----------|-----|----------------------|
| Windows | cpal | WASAPI loopback |
| macOS | cpal | ScreenCaptureKit (cidre crate) |
| Linux | cpal | PipeWire/PulseAudio monitor |

## Tauri v2 IPC Conventions

**IMPORTANT:** Tauri v2 uses camelCase for command parameters in TypeScript, which auto-converts to snake_case in Rust.

```typescript
// Frontend (src/lib/tauri.ts)
export async function processMeeting(
  meetingId: string,    // camelCase
  micPath?: string
): Promise<ProcessingResult> {
  return invoke<ProcessingResult>('process_meeting', {
    meetingId,          // camelCase keys
    micPath,
  });
}
```

```rust
// Backend (src-tauri/src/commands/*.rs)
#[tauri::command]
pub async fn process_meeting(
    meeting_id: String,  // snake_case (auto-converted)
    mic_path: Option<String>,
) -> Result<ProcessingResult, String> { ... }
```

## Frontend Patterns

### Zustand Stores (`src/stores/`)

```typescript
// Settings store with persistence
export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set, get) => ({
      theme: 'system',
      llmModel: 'Qwen3_4B',

      // Actions
      setTheme: (theme) => set({ theme }),

      // Async actions with error handling
      initializeLlm: async (model) => {
        set({ isLoadingLlm: true, error: null });
        try {
          await api.initializeLlm(model);
          set({ llmReady: true, isLoadingLlm: false });
        } catch (e) {
          set({ error: e.message, isLoadingLlm: false });
        }
      },
    }),
    {
      name: 'meeting-scribe-settings',
      partialize: (state) => ({ theme: state.theme, llmModel: state.llmModel }),
    }
  )
);
```

### Custom Hooks (`src/hooks/`)

Hooks wrap stores and add side effects (event listeners, auto-refresh):

```typescript
export function useModels() {
  const store = useSettingsStore();

  // Subscribe to Tauri events
  useTauriEvent<DownloadProgressEvent>('model-download-progress', (data) => {
    store.setDownloadProgress(data.percent, data.model_id);
  });

  // Auto-refresh on mount
  useEffect(() => {
    store.refreshModelStatus();
  }, []);

  return { ...store };
}
```

### Component Structure

- `src/components/ui/` - Reusable primitives (Button, Card, Modal, Badge)
- `src/components/{Feature}/` - Feature-specific components with index.ts exports
- Each feature folder has: View component, sub-components, index.ts

## Development Phases

The `plan/` directory contains sequential implementation guides:
1. `01-project-setup.md` - Scaffolding ✅
2. `02-audio-capture.md` - cpal + platform loopback ✅
3. `03-audio-preprocessing.md` - VAD, denoising, resampling ✅
4. `04-transcription-engine.md` - transcribe-rs integration ✅
5. `05-storage-layer.md` - SQLite + LanceDB ✅
6. `06-embedding-engine.md` - ONNX embeddings ✅
7. `07-llm-engine.md` - llama-cpp-2 ✅
8. `08-frontend-ui.md` - React components (in progress)
9. `09-rag-implementation.md` - Vector search + chat
10. `10-cross-platform.md` - macOS/Linux support
11. `11-deployment.md` - Build optimization, installers

## Common Issues

| Issue | Solution |
|-------|----------|
| "No model loaded" | Call `initializeLlm()` before LLM operations |
| Model shows not downloaded | Check `isModelDownloaded()` not `isReady()` |
| IPC param mismatch | Use camelCase in TS, snake_case in Rust |
| ONNX version conflict | Ensure single `ort` version across crates |
