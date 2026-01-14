# Meeting Scribe

Meeting Scribe is a **privacy-first desktop app** for recording, transcribing, summarizing, and searching meeting content. All inference runs locally using open-source models.

**Stack:** Rust + Tauri v2 + React + TypeScript

## Features

- Record microphone + (Windows) system audio
- Local transcription (default: Parakeet via `transcribe-rs`)
- Local summaries + action items (llama.cpp via `llama-cpp-2`)
- Semantic search + RAG chat (EmbeddingGemma + LanceDB)
- SQLite storage for meetings, transcripts, notes, summaries

## Development

### Prerequisites

- Node.js + `pnpm`
- Rust toolchain (stable)
- Platform build tools (MSVC on Windows)

Some Rust deps may require additional tooling depending on your environment:

- `protoc` (protobuf compiler)
- `cmake` + `ninja` (for crates that compile native code)

### Commands

```bash
pnpm install
pnpm tauri dev
```

Windows convenience script (sets a few env vars before `pnpm tauri dev`):

```bat
dev.bat
```

If you move this repo, update any hard-coded paths in `dev.bat` and `.cargo/config.toml`.

### Lint / Typecheck / Tests

```bash
pnpm lint
npx tsc --noEmit

cd src-tauri
cargo test
cargo clippy
```

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

- **Transcription/embeddings fail to load on Windows**: the ONNX Runtime DLL must be discoverable.
  - This repo uses the `ort` crate with `load-dynamic`.
  - `.cargo/config.toml` sets `ORT_DYLIB_PATH` for local builds; `dev.bat` also sets it for `tauri dev`.
- **Models show “not downloaded”**: download them in Settings first, then click “Load”.
