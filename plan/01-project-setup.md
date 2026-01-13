# 01 - Project Setup

> **Goal:** Scaffold the Tauri v2 + React + TypeScript project with all dependencies configured  
> **Prerequisites:** None (first step)  
> **Estimated Time:** 3-4 days  
> **Outcome:** Running dev environment with basic UI shell

---

## Table of Contents
1. [Prerequisites Check](#prerequisites-check)
2. [Create Tauri Project](#create-tauri-project)
3. [Configure Rust Dependencies](#configure-rust-dependencies)
4. [Configure Frontend Dependencies](#configure-frontend-dependencies)
5. [Project Configuration](#project-configuration)
6. [Basic UI Shell](#basic-ui-shell)
7. [Verification Checklist](#verification-checklist)

---

## Prerequisites Check

### Required Software

| Software | Version | Check Command | Installation |
|----------|---------|---------------|--------------|
| **Rust** | 1.75+ | `rustc --version` | [rustup.rs](https://rustup.rs/) |
| **Node.js** | 20 LTS | `node --version` | [nodejs.org](https://nodejs.org/) |
| **pnpm** (recommended) | 8+ | `pnpm --version` | `npm install -g pnpm` |
| **VS Code** | Latest | - | [code.visualstudio.com](https://code.visualstudio.com/) |

### Platform-Specific Requirements

#### Windows
```powershell
# Install Visual Studio Build Tools
winget install Microsoft.VisualStudio.2022.BuildTools

# Install WebView2 (usually pre-installed on Windows 10/11)
# https://developer.microsoft.com/en-us/microsoft-edge/webview2/
```

**Reference:** [Tauri Windows Prerequisites](https://tauri.app/v2/guide/prerequisites/#windows)

#### macOS
```bash
# Install Xcode Command Line Tools
xcode-select --install

# Install Homebrew (if not installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

**Reference:** [Tauri macOS Prerequisites](https://tauri.app/v2/guide/prerequisites/#macos)

#### Linux (Ubuntu/Debian)
```bash
# Install system dependencies
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libasound2-dev \
  libpulse-dev
```

**Reference:** [Tauri Linux Prerequisites](https://tauri.app/v2/guide/prerequisites/#linux)

### Recommended VS Code Extensions

```json
// .vscode/extensions.json
{
  "recommendations": [
    "rust-lang.rust-analyzer",
    "tauri-apps.tauri-vscode",
    "bradlc.vscode-tailwindcss",
    "dbaeumer.vscode-eslint",
    "esbenp.prettier-vscode"
  ]
}
```

---

## Create Tauri Project

### Step 1: Initialize Project

```bash
# Create new Tauri project with React + TypeScript template
pnpm create tauri-app meeting-scribe --template react-ts

# Navigate to project
cd meeting-scribe

# Install dependencies
pnpm install
```

**Reference:** [Tauri Create App](https://tauri.app/v2/guide/create/)

### Step 2: Verify Initial Setup

```bash
# Run development server
pnpm tauri dev

# Should open a window with "Welcome to Tauri!" message
```

### Step 3: Initialize Git Repository

```bash
git init
git add .
git commit -m "Initial Tauri + React project setup"
```

Create `.gitignore`:
```gitignore
# Dependencies
node_modules/
target/

# Build outputs
dist/
src-tauri/target/

# IDE
.vscode/*
!.vscode/extensions.json
!.vscode/settings.json
.idea/

# OS
.DS_Store
Thumbs.db

# Environment
.env
.env.local

# Logs
*.log
npm-debug.log*

# Models (downloaded separately)
src-tauri/models/
```

---

## Configure Rust Dependencies

### Step 4: Update Cargo.toml

Replace `src-tauri/Cargo.toml` with:

```toml
[package]
name = "meeting-scribe"
version = "0.1.0"
description = "Local-first meeting transcription and RAG"
authors = ["Your Name <your@email.com>"]
edition = "2021"
rust-version = "1.75"

[lib]
name = "meeting_scribe_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
# ============================================
# TAURI CORE
# ============================================
tauri = { version = "2", features = ["macos-private-api"] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"

# ============================================
# ASYNC RUNTIME
# ============================================
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# ============================================
# SERIALIZATION
# ============================================
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# ============================================
# AUDIO CAPTURE & PROCESSING
# ============================================
cpal = "0.15"                    # Cross-platform audio I/O
hound = "3.5"                    # WAV file encoding/decoding
ringbuf = "0.4"                  # Lock-free ring buffers

# ============================================
# AUDIO PREPROCESSING
# ============================================
rubato = "0.15"                  # High-quality resampling
nnnoiseless = "0.5"              # RNNoise denoising (pure Rust)

# ============================================
# VOICE ACTIVITY DETECTION
# ============================================
voice_activity_detector = "0.2"  # Silero VAD v5

# ============================================
# TRANSCRIPTION (added in step 04)
# ============================================
# transcribe-rs = { git = "https://github.com/cjpais/transcribe-rs", features = ["parakeet"] }

# ============================================
# EMBEDDINGS (added in step 06)
# ============================================
# ort = { version = "2", features = ["load-dynamic"] }
# tokenizers = "0.19"

# ============================================
# LLM (added in step 07)
# ============================================
# llama-cpp-2 = "0.1"

# ============================================
# STORAGE
# ============================================
rusqlite = { version = "0.32", features = ["bundled", "backup"] }
# lancedb = "0.10"               # Added in step 05
# arrow-array = "53"             # Added in step 05

# ============================================
# UTILITIES
# ============================================
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
parking_lot = "0.12"
dirs = "5"
reqwest = { version = "0.12", features = ["stream", "json"] }

# ============================================
# PLATFORM-SPECIFIC
# ============================================
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Media_Audio",
    "Win32_System_Com",
    "Win32_Foundation"
]}

[target.'cfg(target_os = "macos")'.dependencies]
# cidre = "0.4"                  # ScreenCaptureKit (added in step 10)

# ============================================
# BUILD CONFIGURATION
# ============================================
[features]
default = []
# GPU acceleration features (enable as needed)
cuda = []
metal = []
directml = []

[profile.dev]
opt-level = 1                    # Some optimization in dev for audio

[profile.release]
lto = true
codegen-units = 1
strip = true
```

**Key Documentation:**
- [cpal docs](https://docs.rs/cpal/latest/cpal/)
- [Tauri v2 plugins](https://tauri.app/v2/plugin/)
- [rusqlite docs](https://docs.rs/rusqlite/latest/rusqlite/)

### Step 5: Create Rust Module Structure

```bash
# Create directory structure
mkdir -p src-tauri/src/{audio,inference,storage,models,commands}

# Create module files
touch src-tauri/src/audio/mod.rs
touch src-tauri/src/audio/capture.rs
touch src-tauri/src/audio/buffer.rs
touch src-tauri/src/audio/vad.rs
touch src-tauri/src/audio/denoise.rs
touch src-tauri/src/inference/mod.rs
touch src-tauri/src/storage/mod.rs
touch src-tauri/src/models/mod.rs
touch src-tauri/src/commands/mod.rs
```

Create `src-tauri/src/lib.rs`:

```rust
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
```

Create `src-tauri/src/main.rs`:

```rust
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use meeting_scribe_lib::{commands, AppConfig};
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meeting_scribe=debug,tauri=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Meeting Scribe...");

    // Initialize app config
    let config = AppConfig::new().expect("Failed to create app config");
    config.ensure_dirs().expect("Failed to create directories");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(config)
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_app_info,
        ])
        .setup(|app| {
            info!("Application setup complete");
            
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Create `src-tauri/src/commands/mod.rs`:

```rust
//! Tauri commands - These functions are callable from the frontend via IPC

use serde::{Deserialize, Serialize};

/// Basic greeting command for testing IPC
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Meeting Scribe.", name)
}

/// Application info response
#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub platform: String,
}

/// Get application information
#[tauri::command]
pub fn get_app_info(config: tauri::State<crate::AppConfig>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: config.data_dir.display().to_string(),
        platform: std::env::consts::OS.to_string(),
    }
}
```

Create stub module files:

```rust
// src-tauri/src/audio/mod.rs
//! Audio capture and processing module
//! 
//! Implemented in step 02-audio-capture.md

pub mod buffer;
pub mod capture;
pub mod denoise;
pub mod vad;
```

```rust
// src-tauri/src/audio/capture.rs
//! Audio capture implementation - See 02-audio-capture.md
```

```rust
// src-tauri/src/audio/buffer.rs
//! Ring buffer management - See 02-audio-capture.md
```

```rust
// src-tauri/src/audio/vad.rs
//! Voice Activity Detection - See 03-audio-preprocessing.md
```

```rust
// src-tauri/src/audio/denoise.rs
//! Audio denoising - See 03-audio-preprocessing.md
```

```rust
// src-tauri/src/inference/mod.rs
//! ML inference module - See steps 04, 06, 07
```

```rust
// src-tauri/src/storage/mod.rs
//! Data persistence - See 05-storage-layer.md
```

```rust
// src-tauri/src/models/mod.rs
//! Model management - See 04-transcription-engine.md
```

---

## Configure Frontend Dependencies

### Step 6: Update package.json

```json
{
  "name": "meeting-scribe",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "lint": "eslint . --ext ts,tsx --report-unused-disable-directives --max-warnings 0",
    "format": "prettier --write \"src/**/*.{ts,tsx,css}\""
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-shell": "^2.0.0",
    "@tauri-apps/plugin-dialog": "^2.0.0",
    "@tauri-apps/plugin-fs": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-router-dom": "^6.26.0",
    "zustand": "^4.5.4",
    "clsx": "^2.1.1",
    "lucide-react": "^0.424.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@tauri-apps/cli": "^2.0.0",
    "@vitejs/plugin-react": "^4.3.1",
    "autoprefixer": "^10.4.19",
    "eslint": "^8.57.0",
    "eslint-plugin-react-hooks": "^4.6.2",
    "eslint-plugin-react-refresh": "^0.4.7",
    "postcss": "^8.4.40",
    "prettier": "^3.3.3",
    "tailwindcss": "^3.4.7",
    "typescript": "^5.5.3",
    "vite": "^5.3.4"
  }
}
```

### Step 7: Configure Tailwind CSS

Create `tailwind.config.js`:

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Custom color palette for Meeting Scribe
        primary: {
          50: '#f0f9ff',
          100: '#e0f2fe',
          200: '#bae6fd',
          300: '#7dd3fc',
          400: '#38bdf8',
          500: '#0ea5e9',
          600: '#0284c7',
          700: '#0369a1',
          800: '#075985',
          900: '#0c4a6e',
        },
        // Dark mode background colors
        surface: {
          50: '#f8fafc',
          100: '#f1f5f9',
          200: '#e2e8f0',
          800: '#1e293b',
          900: '#0f172a',
          950: '#020617',
        },
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
      },
    },
  },
  plugins: [],
  darkMode: 'class',
}
```

Create `postcss.config.js`:

```javascript
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
}
```

Update `src/styles.css` (or create `src/index.css`):

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

/* Base styles */
@layer base {
  body {
    @apply bg-surface-50 text-gray-900 dark:bg-surface-950 dark:text-gray-100;
  }
  
  /* Custom scrollbar */
  ::-webkit-scrollbar {
    @apply w-2;
  }
  
  ::-webkit-scrollbar-track {
    @apply bg-transparent;
  }
  
  ::-webkit-scrollbar-thumb {
    @apply bg-gray-300 dark:bg-gray-700 rounded-full;
  }
}

/* Utility classes */
@layer components {
  .btn {
    @apply px-4 py-2 rounded-lg font-medium transition-colors duration-200;
  }
  
  .btn-primary {
    @apply bg-primary-600 text-white hover:bg-primary-700;
  }
  
  .btn-secondary {
    @apply bg-gray-200 text-gray-800 hover:bg-gray-300 dark:bg-gray-700 dark:text-gray-200 dark:hover:bg-gray-600;
  }
  
  .card {
    @apply bg-white dark:bg-surface-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700;
  }
}
```

**Reference:** [Tailwind CSS Documentation](https://tailwindcss.com/docs)

### Step 8: Install Dependencies

```bash
pnpm install
```

---

## Project Configuration

### Step 9: Update Tauri Configuration

Update `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Meeting Scribe",
  "identifier": "com.meetingscribe.app",
  "version": "0.1.0",
  "build": {
    "beforeBuildCommand": "pnpm build",
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Meeting Scribe",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false,
        "center": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "resources": [],
    "windows": {
      "webviewInstallMode": {
        "type": "downloadBootstrapper"
      }
    }
  },
  "plugins": {
    "shell": {
      "open": true
    }
  }
}
```

**Reference:** [Tauri Configuration](https://tauri.app/v2/reference/config/)

### Step 10: Create VS Code Settings

Create `.vscode/settings.json`:

```json
{
  "editor.formatOnSave": true,
  "editor.defaultFormatter": "esbenp.prettier-vscode",
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.check.command": "clippy",
  "typescript.preferences.importModuleSpecifier": "relative",
  "tailwindCSS.experimental.classRegex": [
    ["clsx\\(([^)]*)\\)", "(?:'|\"|`)([^']*)(?:'|\"|`)"]
  ]
}
```

---

## Basic UI Shell

### Step 11: Create Basic App Structure

Create `src/App.tsx`:

```tsx
import { useState, useEffect } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { Layout } from './components/Layout';
import { RecordingView } from './components/Recording/RecordingView';
import { LibraryView } from './components/Library/LibraryView';
import { ChatView } from './components/Chat/ChatView';
import { SettingsView } from './components/Settings/SettingsView';

interface AppInfo {
  version: string;
  data_dir: string;
  platform: string;
}

function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    // Test IPC connection on mount
    invoke<AppInfo>('get_app_info').then(setAppInfo);
  }, []);

  return (
    <BrowserRouter>
      <Layout appInfo={appInfo}>
        <Routes>
          <Route path="/" element={<RecordingView />} />
          <Route path="/library" element={<LibraryView />} />
          <Route path="/chat" element={<ChatView />} />
          <Route path="/settings" element={<SettingsView />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

export default App;
```

Create `src/components/Layout.tsx`:

```tsx
import { ReactNode } from 'react';
import { NavLink } from 'react-router-dom';
import { Mic, Library, MessageSquare, Settings } from 'lucide-react';
import clsx from 'clsx';

interface LayoutProps {
  children: ReactNode;
  appInfo: { version: string; data_dir: string; platform: string } | null;
}

export function Layout({ children, appInfo }: LayoutProps) {
  return (
    <div className="flex flex-col h-screen bg-surface-50 dark:bg-surface-950">
      {/* Main content */}
      <main className="flex-1 overflow-auto p-6">
        {children}
      </main>

      {/* Bottom navigation */}
      <nav className="border-t border-gray-200 dark:border-gray-800 bg-white dark:bg-surface-900">
        <div className="flex justify-around py-2">
          <NavItem to="/" icon={<Mic size={24} />} label="Record" />
          <NavItem to="/library" icon={<Library size={24} />} label="Library" />
          <NavItem to="/chat" icon={<MessageSquare size={24} />} label="Chat" />
          <NavItem to="/settings" icon={<Settings size={24} />} label="Settings" />
        </div>
        
        {/* Version info (dev only) */}
        {appInfo && (
          <div className="text-center text-xs text-gray-400 pb-2">
            v{appInfo.version} | {appInfo.platform}
          </div>
        )}
      </nav>
    </div>
  );
}

interface NavItemProps {
  to: string;
  icon: ReactNode;
  label: string;
}

function NavItem({ to, icon, label }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        clsx(
          'flex flex-col items-center gap-1 px-4 py-2 rounded-lg transition-colors',
          isActive
            ? 'text-primary-600 dark:text-primary-400'
            : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
        )
      }
    >
      {icon}
      <span className="text-xs font-medium">{label}</span>
    </NavLink>
  );
}
```

Create placeholder view components:

```bash
mkdir -p src/components/{Recording,Library,Chat,Settings,MeetingDetail}
```

Create `src/components/Recording/RecordingView.tsx`:

```tsx
export function RecordingView() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Recording</h1>
      
      <div className="card p-8 text-center">
        <div className="mb-6">
          <div className="w-32 h-32 mx-auto rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
            <span className="text-4xl">🎙️</span>
          </div>
        </div>
        
        <p className="text-gray-500 dark:text-gray-400 mb-6">
          Ready to record your meeting
        </p>
        
        <button className="btn btn-primary text-lg px-8 py-3">
          Start Recording
        </button>
      </div>
      
      <p className="text-sm text-gray-500 text-center">
        Audio capture will be implemented in <code>02-audio-capture.md</code>
      </p>
    </div>
  );
}
```

Create `src/components/Library/LibraryView.tsx`:

```tsx
export function LibraryView() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Meeting Library</h1>
      
      <div className="card p-8 text-center">
        <p className="text-gray-500 dark:text-gray-400">
          No meetings yet. Start recording to see them here.
        </p>
      </div>
    </div>
  );
}
```

Create `src/components/Chat/ChatView.tsx`:

```tsx
export function ChatView() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Chat with Meetings</h1>
      
      <div className="card p-8 text-center">
        <p className="text-gray-500 dark:text-gray-400">
          RAG chat will be implemented in <code>09-rag-implementation.md</code>
        </p>
      </div>
    </div>
  );
}
```

Create `src/components/Settings/SettingsView.tsx`:

```tsx
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AppInfo {
  version: string;
  data_dir: string;
  platform: string;
}

export function SettingsView() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>('get_app_info').then(setAppInfo);
  }, []);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Settings</h1>
      
      {appInfo && (
        <div className="card p-6 space-y-4">
          <h2 className="font-semibold">Application Info</h2>
          
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="text-gray-500">Version</div>
            <div>{appInfo.version}</div>
            
            <div className="text-gray-500">Platform</div>
            <div>{appInfo.platform}</div>
            
            <div className="text-gray-500">Data Directory</div>
            <div className="font-mono text-xs break-all">{appInfo.data_dir}</div>
          </div>
        </div>
      )}
      
      <div className="card p-6">
        <h2 className="font-semibold mb-4">Models</h2>
        <p className="text-gray-500 text-sm">
          Model management will be added in <code>04-transcription-engine.md</code>
        </p>
      </div>
    </div>
  );
}
```

Update `src/main.tsx`:

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

---

## Verification Checklist

### ✅ Acceptance Criteria

Run through each item to verify setup is complete:

- [ ] **Project builds without errors**
  ```bash
  pnpm tauri build --debug
  ```

- [ ] **Development server starts**
  ```bash
  pnpm tauri dev
  # Window should open with UI shell
  ```

- [ ] **IPC works** - Settings page shows app info (version, platform, data dir)

- [ ] **Navigation works** - All four tabs (Record, Library, Chat, Settings) are accessible

- [ ] **Tailwind CSS applied** - UI has proper styling (rounded cards, colors)

- [ ] **Data directories created** - Check that `~/.meeting-scribe/` (or platform equivalent) exists

- [ ] **Rust analyzer working** - Open a `.rs` file in VS Code, check for autocomplete

### 🧪 Test Commands

```bash
# Run Rust tests
cd src-tauri && cargo test

# Run Rust linter
cd src-tauri && cargo clippy

# Run frontend linter
pnpm lint

# Format code
pnpm format
cd src-tauri && cargo fmt
```

### 📝 Commit Checkpoint

```bash
git add .
git commit -m "Complete project setup with Tauri v2 + React + TypeScript"
```

---

## Troubleshooting

### Common Issues

#### "WebView2 not found" (Windows)
```powershell
# Install WebView2 runtime
winget install Microsoft.EdgeWebView2Runtime
```

#### "Command not found: pnpm"
```bash
npm install -g pnpm
```

#### Rust compilation errors about missing C compiler
```bash
# Windows: Install Visual Studio Build Tools
# macOS: xcode-select --install
# Linux: sudo apt install build-essential
```

#### "Cannot find module '@tauri-apps/api'"
```bash
pnpm install
```

---

## Next Steps

With the project scaffolded, proceed to:

→ **[02-audio-capture.md](./02-audio-capture.md)** - Implement microphone and system audio capture

---

## References

- [Tauri v2 Documentation](https://tauri.app/v2/guide/)
- [Tauri v2 Prerequisites](https://tauri.app/v2/guide/prerequisites/)
- [React Router Documentation](https://reactrouter.com/)
- [Zustand Documentation](https://github.com/pmndrs/zustand)
- [Tailwind CSS Documentation](https://tailwindcss.com/docs)
- [rust-analyzer User Manual](https://rust-analyzer.github.io/manual.html)
