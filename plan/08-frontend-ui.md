# 08 - Frontend UI: React Components and State Management

## Goal
Build a polished, responsive React frontend with Tailwind CSS for the Meeting Scribe desktop application. Implement all core views (Recording, Library, Meeting Detail, Chat, Settings) with proper state management, Tauri IPC integration, and real-time updates.

**Estimated Time:** 6-7 days

## Prerequisites
- Document 01 (Project Setup) completed - Tauri + React scaffolded
- Document 02 (Audio Capture) completed - Recording commands available
- Document 05 (Storage Layer) completed - Data persistence working
- Document 07 (LLM Engine) completed - Summarization available

## Technology Overview

### Stack

| Technology | Purpose | Documentation |
|------------|---------|---------------|
| **React 18** | UI framework | [react.dev](https://react.dev/) |
| **TypeScript** | Type safety | [typescriptlang.org](https://www.typescriptlang.org/) |
| **Tailwind CSS** | Styling | [tailwindcss.com](https://tailwindcss.com/) |
| **Zustand** | State management | [zustand](https://github.com/pmndrs/zustand) |
| **TanStack Query** | Server state | [tanstack.com/query](https://tanstack.com/query/latest) |
| **Tauri API** | IPC communication | [tauri.app/v2/reference/js](https://tauri.app/v2/reference/js/) |
| **Lucide React** | Icons | [lucide.dev](https://lucide.dev/) |
| **Framer Motion** | Animations | [framer.com/motion](https://www.framer.com/motion/) |

### Why This Stack?

1. **Zustand over Redux**: Simpler API, less boilerplate, perfect for desktop apps
2. **TanStack Query**: Handles caching, background refetching, optimistic updates
3. **Tailwind CSS**: Rapid prototyping, consistent design system, small bundle
4. **Framer Motion**: Smooth animations for state transitions

## Project Structure

```
src/
├── App.tsx                    # Main app with routing
├── main.tsx                   # Entry point
├── index.css                  # Tailwind imports
│
├── components/
│   ├── ui/                    # Reusable UI primitives
│   │   ├── Button.tsx
│   │   ├── Card.tsx
│   │   ├── Input.tsx
│   │   ├── Modal.tsx
│   │   ├── Progress.tsx
│   │   ├── Tabs.tsx
│   │   ├── Toast.tsx
│   │   └── index.ts
│   │
│   ├── layout/
│   │   ├── AppShell.tsx       # Main layout wrapper
│   │   ├── Navigation.tsx     # Bottom navigation bar
│   │   └── TitleBar.tsx       # Custom window title bar
│   │
│   ├── recording/
│   │   ├── RecordingView.tsx  # Main recording screen
│   │   ├── Waveform.tsx       # Audio waveform display
│   │   ├── RecordingControls.tsx
│   │   ├── RecordingTimer.tsx
│   │   └── AudioLevelMeter.tsx
│   │
│   ├── library/
│   │   ├── LibraryView.tsx    # Meeting list
│   │   ├── MeetingCard.tsx    # Individual meeting item
│   │   ├── MeetingSearch.tsx  # Search input
│   │   ├── MeetingFilters.tsx # Filter controls
│   │   └── TimelineGroup.tsx  # Date grouping
│   │
│   ├── meeting/
│   │   ├── MeetingDetailView.tsx
│   │   ├── TranscriptPanel.tsx
│   │   ├── SummaryPanel.tsx
│   │   ├── NotesPanel.tsx
│   │   ├── AudioPlayer.tsx
│   │   └── MeetingHeader.tsx
│   │
│   ├── chat/
│   │   ├── ChatView.tsx       # RAG chat interface
│   │   ├── ChatMessage.tsx    # Message bubble
│   │   ├── ChatInput.tsx      # Message input
│   │   ├── SourceCard.tsx     # Citation display
│   │   └── ChatSuggestions.tsx
│   │
│   └── settings/
│       ├── SettingsView.tsx
│       ├── ModelSettings.tsx  # Model management
│       ├── AudioSettings.tsx  # Device selection
│       ├── StorageSettings.tsx
│       └── ModelDownloadCard.tsx
│
├── hooks/
│   ├── useRecording.ts        # Recording state
│   ├── useMeetings.ts         # Meeting queries
│   ├── useSearch.ts           # Search functionality
│   ├── useChat.ts             # Chat state
│   ├── useModels.ts           # Model management
│   ├── useAudioDevices.ts     # Audio device list
│   └── useTauriEvent.ts       # Event subscription
│
├── stores/
│   ├── recordingStore.ts      # Recording UI state
│   ├── chatStore.ts           # Chat messages
│   ├── settingsStore.ts       # App preferences
│   └── toastStore.ts          # Notifications
│
├── lib/
│   ├── tauri.ts               # Tauri command wrappers
│   ├── formatters.ts          # Date, duration formatting
│   └── constants.ts           # App constants
│
└── types/
    ├── meeting.ts             # Meeting types
    ├── recording.ts           # Recording types
    ├── chat.ts                # Chat types
    └── models.ts              # Model types
```

## Implementation

### Step 1: Install Dependencies

```bash
cd meeting-scribe

# Core dependencies
pnpm add zustand @tanstack/react-query lucide-react framer-motion

# Development
pnpm add -D @types/node
```

### Step 2: Tailwind Configuration

**File: `tailwind.config.js`**

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
        // Custom color palette
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
        surface: {
          50: '#fafafa',
          100: '#f4f4f5',
          200: '#e4e4e7',
          300: '#d4d4d8',
          400: '#a1a1aa',
          500: '#71717a',
          600: '#52525b',
          700: '#3f3f46',
          800: '#27272a',
          900: '#18181b',
        },
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Menlo', 'monospace'],
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'waveform': 'waveform 1s ease-in-out infinite',
      },
      keyframes: {
        waveform: {
          '0%, 100%': { transform: 'scaleY(0.5)' },
          '50%': { transform: 'scaleY(1)' },
        },
      },
    },
  },
  plugins: [],
};
```

**File: `src/index.css`**

```css
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap');

@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  html {
    font-family: 'Inter', system-ui, sans-serif;
  }
  
  /* Custom scrollbar */
  ::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }
  
  ::-webkit-scrollbar-track {
    background: transparent;
  }
  
  ::-webkit-scrollbar-thumb {
    background: #d4d4d8;
    border-radius: 4px;
  }
  
  ::-webkit-scrollbar-thumb:hover {
    background: #a1a1aa;
  }
  
  /* Dark mode scrollbar */
  .dark ::-webkit-scrollbar-thumb {
    background: #3f3f46;
  }
  
  .dark ::-webkit-scrollbar-thumb:hover {
    background: #52525b;
  }
}

@layer components {
  .btn-primary {
    @apply bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded-lg 
           font-medium transition-colors duration-200 
           focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2;
  }
  
  .btn-secondary {
    @apply bg-surface-200 hover:bg-surface-300 text-surface-700 px-4 py-2 rounded-lg 
           font-medium transition-colors duration-200
           focus:outline-none focus:ring-2 focus:ring-surface-400 focus:ring-offset-2;
  }
  
  .btn-danger {
    @apply bg-red-600 hover:bg-red-700 text-white px-4 py-2 rounded-lg 
           font-medium transition-colors duration-200;
  }
  
  .card {
    @apply bg-white rounded-xl shadow-sm border border-surface-200 p-4;
  }
  
  .input {
    @apply w-full px-3 py-2 border border-surface-300 rounded-lg
           focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent
           placeholder:text-surface-400;
  }
}
```

### Step 3: TypeScript Types

**File: `src/types/meeting.ts`**

```typescript
export type MeetingStatus = 'recording' | 'processing' | 'ready' | 'error';
export type Speaker = 'you' | 'others' | 'unknown';
export type SummaryType = 'key_points' | 'action_items' | 'full';

export interface Meeting {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  duration_ms: number;
  status: MeetingStatus;
  audio_path_you?: string;
  audio_path_others?: string;
  tags: string[];
}

export interface TranscriptSegment {
  id: number;
  meeting_id: string;
  speaker: Speaker;
  text: string;
  start_ms: number;
  end_ms: number;
  confidence: number;
}

export interface Note {
  id: number;
  meeting_id: string;
  content: string;
  created_at: number;
  updated_at: number;
}

export interface Summary {
  id: number;
  meeting_id: string;
  summary_type: SummaryType;
  content: string;
  model_used?: string;
  created_at: number;
}

export interface MeetingWithDetails extends Meeting {
  transcript: TranscriptSegment[];
  notes: Note[];
  summaries: Summary[];
}

// Grouped by date for timeline view
export interface MeetingGroup {
  date: string; // ISO date string (YYYY-MM-DD)
  label: string; // "Today", "Yesterday", "January 12, 2026"
  meetings: Meeting[];
}
```

**File: `src/types/recording.ts`**

```typescript
export type RecordingState = 'idle' | 'recording' | 'paused' | 'stopping';

export interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
  device_type: 'input' | 'output';
}

export interface WaveformData {
  you: {
    rms: number;
    peak: number;
    samples: number[];
  };
  others: {
    rms: number;
    peak: number;
    samples: number[];
  };
  timestamp: number;
}

export interface RecordingProgress {
  duration_ms: number;
  speech_detected_you: boolean;
  speech_detected_others: boolean;
}
```

**File: `src/types/chat.ts`**

```typescript
export type MessageRole = 'user' | 'assistant';

export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: number;
  sources?: ChatSource[];
  isStreaming?: boolean;
}

export interface ChatSource {
  meeting_id: string;
  meeting_title: string;
  chunk_type: 'transcript' | 'note' | 'summary';
  text: string;
  start_ms?: number;
  similarity: number;
}

export interface ChatSession {
  id: string;
  title: string;
  messages: ChatMessage[];
  created_at: number;
}
```

**File: `src/types/models.ts`**

```typescript
export type ModelType = 'transcription' | 'embedding' | 'llm' | 'vad';
export type ModelStatus = 'not_downloaded' | 'downloading' | 'ready' | 'error';

export interface ModelInfo {
  id: string;
  name: string;
  type: ModelType;
  size_bytes: number;
  status: ModelStatus;
  download_progress?: number;
  error_message?: string;
  description: string;
}

export interface TranscriptionEngine {
  id: 'parakeet' | 'whisper' | 'moonshine';
  name: string;
  model: ModelInfo;
  languages: string[];
  speed: string; // "4x realtime"
}
```

### Step 4: Tauri API Wrapper

**File: `src/lib/tauri.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { 
  Meeting, 
  MeetingWithDetails, 
  TranscriptSegment,
  Summary 
} from '../types/meeting';
import type { 
  AudioDevice, 
  WaveformData, 
  RecordingProgress 
} from '../types/recording';
import type { ChatSource } from '../types/chat';
import type { ModelInfo } from '../types/models';

// ==================== Recording Commands ====================

export async function startRecording(title?: string): Promise<string> {
  return invoke('start_recording', { title });
}

export async function stopRecording(): Promise<Meeting> {
  return invoke('stop_recording');
}

export async function pauseRecording(): Promise<void> {
  return invoke('pause_recording');
}

export async function resumeRecording(): Promise<void> {
  return invoke('resume_recording');
}

export async function getRecordingState(): Promise<{
  is_recording: boolean;
  is_paused: boolean;
  meeting_id?: string;
  duration_ms: number;
}> {
  return invoke('get_recording_state');
}

export async function listAudioDevices(): Promise<AudioDevice[]> {
  return invoke('list_audio_devices');
}

export async function setAudioDevice(
  deviceId: string, 
  deviceType: 'input' | 'output'
): Promise<void> {
  return invoke('set_audio_device', { deviceId, deviceType });
}

// ==================== Meeting Commands ====================

export async function listMeetings(options?: {
  limit?: number;
  offset?: number;
  status?: string;
}): Promise<Meeting[]> {
  return invoke('list_meetings', { options });
}

export async function getMeeting(id: string): Promise<MeetingWithDetails> {
  return invoke('get_meeting', { id });
}

export async function updateMeeting(
  id: string, 
  updates: Partial<Pick<Meeting, 'title' | 'tags'>>
): Promise<Meeting> {
  return invoke('update_meeting', { id, updates });
}

export async function deleteMeeting(id: string): Promise<void> {
  return invoke('delete_meeting', { id });
}

export async function getTranscript(meetingId: string): Promise<TranscriptSegment[]> {
  return invoke('get_transcript', { meetingId });
}

// ==================== Processing Commands ====================

export async function transcribeMeeting(meetingId: string): Promise<void> {
  return invoke('transcribe_meeting', { meetingId });
}

export async function generateSummary(
  meetingId: string, 
  summaryType: 'key_points' | 'action_items' | 'full'
): Promise<Summary> {
  return invoke('generate_summary', { meetingId, summaryType });
}

export async function embedMeeting(meetingId: string): Promise<void> {
  return invoke('embed_meeting_transcript', { meetingId });
}

// ==================== Search Commands ====================

export async function searchMeetings(query: string): Promise<{
  meeting_id: string;
  title: string;
  snippet: string;
  rank: number;
}[]> {
  return invoke('search_transcripts', { query });
}

export async function vectorSearch(
  query: string, 
  options?: { limit?: number; meeting_id?: string }
): Promise<ChatSource[]> {
  return invoke('vector_search', { query, ...options });
}

// ==================== Chat Commands ====================

export async function chatWithMeetings(
  message: string,
  history: { role: 'user' | 'assistant'; content: string }[]
): Promise<{ response: string; sources: ChatSource[] }> {
  return invoke('chat_with_meetings', { message, history });
}

export async function streamChatResponse(
  message: string,
  history: { role: 'user' | 'assistant'; content: string }[]
): Promise<string> {
  // Returns stream ID for event listening
  return invoke('stream_chat_response', { message, history });
}

// ==================== Model Commands ====================

export async function listModels(): Promise<ModelInfo[]> {
  return invoke('list_models');
}

export async function downloadModel(modelId: string): Promise<void> {
  return invoke('download_model', { modelId });
}

export async function cancelDownload(modelId: string): Promise<void> {
  return invoke('cancel_download', { modelId });
}

export async function deleteModel(modelId: string): Promise<void> {
  return invoke('delete_model', { modelId });
}

export async function setActiveModel(
  modelType: 'transcription' | 'llm',
  modelId: string
): Promise<void> {
  return invoke('set_active_model', { modelType, modelId });
}

// ==================== Storage Commands ====================

export async function getDatabaseStats(): Promise<{
  meeting_count: number;
  total_duration_ms: number;
  storage_bytes: number;
  audio_storage_bytes: number;
}> {
  return invoke('get_database_stats');
}

export async function exportMeeting(
  meetingId: string, 
  format: 'markdown' | 'json'
): Promise<string> {
  return invoke('export_meeting', { meetingId, format });
}

// ==================== Event Listeners ====================

export function onWaveformUpdate(
  callback: (data: WaveformData) => void
): Promise<UnlistenFn> {
  return listen<WaveformData>('waveform-update', (event) => {
    callback(event.payload);
  });
}

export function onRecordingProgress(
  callback: (data: RecordingProgress) => void
): Promise<UnlistenFn> {
  return listen<RecordingProgress>('recording-progress', (event) => {
    callback(event.payload);
  });
}

export function onTranscriptionProgress(
  callback: (data: { meeting_id: string; progress: number; stage: string }) => void
): Promise<UnlistenFn> {
  return listen('transcription-progress', (event) => {
    callback(event.payload as any);
  });
}

export function onModelDownloadProgress(
  callback: (data: { model_id: string; progress: number }) => void
): Promise<UnlistenFn> {
  return listen('model-download-progress', (event) => {
    callback(event.payload as any);
  });
}

export function onChatToken(
  streamId: string,
  callback: (token: string) => void
): Promise<UnlistenFn> {
  return listen<string>(`chat-token-${streamId}`, (event) => {
    callback(event.payload);
  });
}

export function onChatComplete(
  streamId: string,
  callback: (sources: ChatSource[]) => void
): Promise<UnlistenFn> {
  return listen(`chat-complete-${streamId}`, (event) => {
    callback(event.payload as ChatSource[]);
  });
}
```

### Step 5: Zustand Stores

**File: `src/stores/recordingStore.ts`**

```typescript
import { create } from 'zustand';
import type { RecordingState, WaveformData } from '../types/recording';

interface RecordingStore {
  // State
  state: RecordingState;
  meetingId: string | null;
  durationMs: number;
  waveformData: WaveformData | null;
  speechDetectedYou: boolean;
  speechDetectedOthers: boolean;
  
  // Actions
  setRecordingState: (state: RecordingState) => void;
  setMeetingId: (id: string | null) => void;
  setDuration: (ms: number) => void;
  updateWaveform: (data: WaveformData) => void;
  setSpeechDetected: (you: boolean, others: boolean) => void;
  reset: () => void;
}

const initialState = {
  state: 'idle' as RecordingState,
  meetingId: null,
  durationMs: 0,
  waveformData: null,
  speechDetectedYou: false,
  speechDetectedOthers: false,
};

export const useRecordingStore = create<RecordingStore>((set) => ({
  ...initialState,
  
  setRecordingState: (state) => set({ state }),
  
  setMeetingId: (meetingId) => set({ meetingId }),
  
  setDuration: (durationMs) => set({ durationMs }),
  
  updateWaveform: (waveformData) => set({ waveformData }),
  
  setSpeechDetected: (speechDetectedYou, speechDetectedOthers) => 
    set({ speechDetectedYou, speechDetectedOthers }),
  
  reset: () => set(initialState),
}));
```

**File: `src/stores/chatStore.ts`**

```typescript
import { create } from 'zustand';
import { v4 as uuid } from 'uuid';
import type { ChatMessage, ChatSource } from '../types/chat';

interface ChatStore {
  // State
  messages: ChatMessage[];
  isLoading: boolean;
  streamingMessageId: string | null;
  
  // Actions
  addUserMessage: (content: string) => string;
  startAssistantMessage: () => string;
  appendToMessage: (id: string, token: string) => void;
  completeMessage: (id: string, sources: ChatSource[]) => void;
  setLoading: (loading: boolean) => void;
  clearMessages: () => void;
}

export const useChatStore = create<ChatStore>((set, get) => ({
  messages: [],
  isLoading: false,
  streamingMessageId: null,
  
  addUserMessage: (content) => {
    const id = uuid();
    set((state) => ({
      messages: [...state.messages, {
        id,
        role: 'user',
        content,
        timestamp: Date.now(),
      }],
    }));
    return id;
  },
  
  startAssistantMessage: () => {
    const id = uuid();
    set((state) => ({
      messages: [...state.messages, {
        id,
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
        isStreaming: true,
      }],
      streamingMessageId: id,
      isLoading: true,
    }));
    return id;
  },
  
  appendToMessage: (id, token) => {
    set((state) => ({
      messages: state.messages.map((msg) =>
        msg.id === id
          ? { ...msg, content: msg.content + token }
          : msg
      ),
    }));
  },
  
  completeMessage: (id, sources) => {
    set((state) => ({
      messages: state.messages.map((msg) =>
        msg.id === id
          ? { ...msg, isStreaming: false, sources }
          : msg
      ),
      streamingMessageId: null,
      isLoading: false,
    }));
  },
  
  setLoading: (isLoading) => set({ isLoading }),
  
  clearMessages: () => set({ messages: [], streamingMessageId: null }),
}));
```

**File: `src/stores/toastStore.ts`**

```typescript
import { create } from 'zustand';

type ToastType = 'success' | 'error' | 'info' | 'warning';

interface Toast {
  id: string;
  type: ToastType;
  title: string;
  message?: string;
  duration?: number;
}

interface ToastStore {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, 'id'>) => void;
  removeToast: (id: string) => void;
}

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  
  addToast: (toast) => {
    const id = crypto.randomUUID();
    const duration = toast.duration ?? 5000;
    
    set((state) => ({
      toasts: [...state.toasts, { ...toast, id }],
    }));
    
    // Auto-remove after duration
    if (duration > 0) {
      setTimeout(() => {
        set((state) => ({
          toasts: state.toasts.filter((t) => t.id !== id),
        }));
      }, duration);
    }
  },
  
  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },
}));

// Convenience functions
export const toast = {
  success: (title: string, message?: string) => 
    useToastStore.getState().addToast({ type: 'success', title, message }),
  error: (title: string, message?: string) => 
    useToastStore.getState().addToast({ type: 'error', title, message }),
  info: (title: string, message?: string) => 
    useToastStore.getState().addToast({ type: 'info', title, message }),
  warning: (title: string, message?: string) => 
    useToastStore.getState().addToast({ type: 'warning', title, message }),
};
```

**File: `src/stores/settingsStore.ts`**

```typescript
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SettingsStore {
  // Audio settings
  inputDeviceId: string | null;
  outputDeviceId: string | null;
  enableDenoising: boolean;
  
  // Model settings
  activeTranscriptionEngine: 'parakeet' | 'whisper' | 'moonshine';
  activeLLM: string;
  
  // UI settings
  theme: 'light' | 'dark' | 'system';
  compactView: boolean;
  showTimestamps: boolean;
  
  // Actions
  setInputDevice: (id: string | null) => void;
  setOutputDevice: (id: string | null) => void;
  setDenoising: (enabled: boolean) => void;
  setTranscriptionEngine: (engine: 'parakeet' | 'whisper' | 'moonshine') => void;
  setActiveLLM: (model: string) => void;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setCompactView: (compact: boolean) => void;
  setShowTimestamps: (show: boolean) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      // Default values
      inputDeviceId: null,
      outputDeviceId: null,
      enableDenoising: true,
      activeTranscriptionEngine: 'parakeet',
      activeLLM: 'llama-3.2-3b-q4',
      theme: 'system',
      compactView: false,
      showTimestamps: true,
      
      // Actions
      setInputDevice: (inputDeviceId) => set({ inputDeviceId }),
      setOutputDevice: (outputDeviceId) => set({ outputDeviceId }),
      setDenoising: (enableDenoising) => set({ enableDenoising }),
      setTranscriptionEngine: (activeTranscriptionEngine) => 
        set({ activeTranscriptionEngine }),
      setActiveLLM: (activeLLM) => set({ activeLLM }),
      setTheme: (theme) => set({ theme }),
      setCompactView: (compactView) => set({ compactView }),
      setShowTimestamps: (showTimestamps) => set({ showTimestamps }),
    }),
    {
      name: 'meeting-scribe-settings',
    }
  )
);
```

### Step 6: Custom Hooks

**File: `src/hooks/useTauriEvent.ts`**

```typescript
import { useEffect, useRef } from 'react';
import { UnlistenFn } from '@tauri-apps/api/event';

/**
 * Hook to subscribe to Tauri events with automatic cleanup
 */
export function useTauriEvent<T>(
  subscribe: (callback: (data: T) => void) => Promise<UnlistenFn>,
  callback: (data: T) => void,
  deps: React.DependencyList = []
) {
  const unlistenRef = useRef<UnlistenFn | null>(null);
  
  useEffect(() => {
    let mounted = true;
    
    subscribe((data) => {
      if (mounted) {
        callback(data);
      }
    }).then((unlisten) => {
      if (mounted) {
        unlistenRef.current = unlisten;
      } else {
        unlisten();
      }
    });
    
    return () => {
      mounted = false;
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, deps);
}
```

**File: `src/hooks/useRecording.ts`**

```typescript
import { useCallback, useEffect } from 'react';
import { useRecordingStore } from '../stores/recordingStore';
import { useTauriEvent } from './useTauriEvent';
import { toast } from '../stores/toastStore';
import * as api from '../lib/tauri';
import type { WaveformData, RecordingProgress } from '../types/recording';

export function useRecording() {
  const store = useRecordingStore();
  
  // Subscribe to waveform updates
  useTauriEvent<WaveformData>(
    api.onWaveformUpdate,
    (data) => store.updateWaveform(data),
    []
  );
  
  // Subscribe to recording progress
  useTauriEvent<RecordingProgress>(
    api.onRecordingProgress,
    (data) => {
      store.setDuration(data.duration_ms);
      store.setSpeechDetected(
        data.speech_detected_you, 
        data.speech_detected_others
      );
    },
    []
  );
  
  // Sync initial state on mount
  useEffect(() => {
    api.getRecordingState().then((state) => {
      if (state.is_recording) {
        store.setRecordingState(state.is_paused ? 'paused' : 'recording');
        store.setMeetingId(state.meeting_id ?? null);
        store.setDuration(state.duration_ms);
      }
    });
  }, []);
  
  const start = useCallback(async (title?: string) => {
    try {
      store.setRecordingState('recording');
      const meetingId = await api.startRecording(title);
      store.setMeetingId(meetingId);
      toast.success('Recording started');
    } catch (error) {
      store.reset();
      toast.error('Failed to start recording', String(error));
      throw error;
    }
  }, []);
  
  const stop = useCallback(async () => {
    try {
      store.setRecordingState('stopping');
      const meeting = await api.stopRecording();
      store.reset();
      toast.success('Recording saved', `Duration: ${formatDuration(meeting.duration_ms)}`);
      return meeting;
    } catch (error) {
      store.setRecordingState('recording');
      toast.error('Failed to stop recording', String(error));
      throw error;
    }
  }, []);
  
  const pause = useCallback(async () => {
    try {
      await api.pauseRecording();
      store.setRecordingState('paused');
    } catch (error) {
      toast.error('Failed to pause recording', String(error));
      throw error;
    }
  }, []);
  
  const resume = useCallback(async () => {
    try {
      await api.resumeRecording();
      store.setRecordingState('recording');
    } catch (error) {
      toast.error('Failed to resume recording', String(error));
      throw error;
    }
  }, []);
  
  return {
    ...store,
    start,
    stop,
    pause,
    resume,
  };
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  
  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  return `${minutes}m ${seconds % 60}s`;
}
```

**File: `src/hooks/useMeetings.ts`**

```typescript
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import * as api from '../lib/tauri';
import { toast } from '../stores/toastStore';
import type { Meeting, MeetingWithDetails, MeetingGroup } from '../types/meeting';

// Query keys
export const meetingKeys = {
  all: ['meetings'] as const,
  list: () => [...meetingKeys.all, 'list'] as const,
  detail: (id: string) => [...meetingKeys.all, 'detail', id] as const,
  search: (query: string) => [...meetingKeys.all, 'search', query] as const,
};

// List meetings with grouping by date
export function useMeetings() {
  return useQuery({
    queryKey: meetingKeys.list(),
    queryFn: api.listMeetings,
    select: (meetings) => groupMeetingsByDate(meetings),
    staleTime: 30 * 1000, // 30 seconds
  });
}

// Single meeting with details
export function useMeeting(id: string) {
  return useQuery({
    queryKey: meetingKeys.detail(id),
    queryFn: () => api.getMeeting(id),
    enabled: !!id,
  });
}

// Update meeting
export function useUpdateMeeting() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: ({ id, updates }: { 
      id: string; 
      updates: Partial<Pick<Meeting, 'title' | 'tags'>> 
    }) => api.updateMeeting(id, updates),
    
    onSuccess: (meeting) => {
      queryClient.setQueryData(meetingKeys.detail(meeting.id), (old: MeetingWithDetails | undefined) => 
        old ? { ...old, ...meeting } : undefined
      );
      queryClient.invalidateQueries({ queryKey: meetingKeys.list() });
      toast.success('Meeting updated');
    },
    
    onError: (error) => {
      toast.error('Failed to update meeting', String(error));
    },
  });
}

// Delete meeting
export function useDeleteMeeting() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: api.deleteMeeting,
    
    onSuccess: (_, id) => {
      queryClient.removeQueries({ queryKey: meetingKeys.detail(id) });
      queryClient.invalidateQueries({ queryKey: meetingKeys.list() });
      toast.success('Meeting deleted');
    },
    
    onError: (error) => {
      toast.error('Failed to delete meeting', String(error));
    },
  });
}

// Transcription
export function useTranscribeMeeting() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: api.transcribeMeeting,
    
    onSuccess: (_, meetingId) => {
      queryClient.invalidateQueries({ queryKey: meetingKeys.detail(meetingId) });
      toast.success('Transcription complete');
    },
    
    onError: (error) => {
      toast.error('Transcription failed', String(error));
    },
  });
}

// Summary generation
export function useGenerateSummary() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: ({ meetingId, summaryType }: { 
      meetingId: string; 
      summaryType: 'key_points' | 'action_items' | 'full' 
    }) => api.generateSummary(meetingId, summaryType),
    
    onSuccess: (summary, { meetingId }) => {
      queryClient.invalidateQueries({ queryKey: meetingKeys.detail(meetingId) });
      toast.success('Summary generated');
    },
    
    onError: (error) => {
      toast.error('Failed to generate summary', String(error));
    },
  });
}

// Helper: Group meetings by date
function groupMeetingsByDate(meetings: Meeting[]): MeetingGroup[] {
  const groups = new Map<string, Meeting[]>();
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  
  for (const meeting of meetings) {
    const date = new Date(meeting.created_at);
    date.setHours(0, 0, 0, 0);
    const key = date.toISOString().split('T')[0];
    
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key)!.push(meeting);
  }
  
  return Array.from(groups.entries())
    .map(([dateStr, meetings]) => {
      const date = new Date(dateStr);
      let label: string;
      
      if (date.getTime() === today.getTime()) {
        label = 'Today';
      } else if (date.getTime() === yesterday.getTime()) {
        label = 'Yesterday';
      } else {
        label = date.toLocaleDateString('en-US', {
          weekday: 'long',
          month: 'long',
          day: 'numeric',
          year: date.getFullYear() !== today.getFullYear() ? 'numeric' : undefined,
        });
      }
      
      return { date: dateStr, label, meetings };
    })
    .sort((a, b) => b.date.localeCompare(a.date));
}
```

**File: `src/hooks/useSearch.ts`**

```typescript
import { useQuery } from '@tanstack/react-query';
import { useDebouncedValue } from './useDebouncedValue';
import * as api from '../lib/tauri';
import { meetingKeys } from './useMeetings';

export function useSearch(query: string) {
  const debouncedQuery = useDebouncedValue(query, 300);
  
  return useQuery({
    queryKey: meetingKeys.search(debouncedQuery),
    queryFn: () => api.searchMeetings(debouncedQuery),
    enabled: debouncedQuery.length >= 2,
    staleTime: 60 * 1000,
  });
}

export function useVectorSearch(query: string, options?: { 
  limit?: number; 
  meetingId?: string 
}) {
  const debouncedQuery = useDebouncedValue(query, 500);
  
  return useQuery({
    queryKey: ['vector-search', debouncedQuery, options],
    queryFn: () => api.vectorSearch(debouncedQuery, options),
    enabled: debouncedQuery.length >= 3,
    staleTime: 60 * 1000,
  });
}
```

**File: `src/hooks/useDebouncedValue.ts`**

```typescript
import { useState, useEffect } from 'react';

export function useDebouncedValue<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState(value);
  
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedValue(value);
    }, delay);
    
    return () => clearTimeout(timer);
  }, [value, delay]);
  
  return debouncedValue;
}
```

**File: `src/hooks/useChat.ts`**

```typescript
import { useCallback, useRef } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useTauriEvent } from './useTauriEvent';
import { toast } from '../stores/toastStore';
import * as api from '../lib/tauri';
import type { ChatSource } from '../types/chat';

export function useChat() {
  const store = useChatStore();
  const streamIdRef = useRef<string | null>(null);
  
  const sendMessage = useCallback(async (content: string) => {
    if (!content.trim() || store.isLoading) return;
    
    // Add user message
    store.addUserMessage(content);
    
    // Prepare history
    const history = store.messages.map((msg) => ({
      role: msg.role,
      content: msg.content,
    }));
    
    try {
      // Start streaming response
      const streamId = await api.streamChatResponse(content, history);
      streamIdRef.current = streamId;
      
      // Start assistant message
      const messageId = store.startAssistantMessage();
      
      // Listen for tokens
      const unlistenToken = await api.onChatToken(streamId, (token) => {
        store.appendToMessage(messageId, token);
      });
      
      // Listen for completion
      const unlistenComplete = await api.onChatComplete(streamId, (sources) => {
        store.completeMessage(messageId, sources);
        unlistenToken();
        unlistenComplete();
        streamIdRef.current = null;
      });
    } catch (error) {
      store.setLoading(false);
      toast.error('Failed to send message', String(error));
    }
  }, [store.messages, store.isLoading]);
  
  const clearChat = useCallback(() => {
    store.clearMessages();
  }, []);
  
  return {
    messages: store.messages,
    isLoading: store.isLoading,
    sendMessage,
    clearChat,
  };
}
```

### Step 7: UI Components

**File: `src/components/ui/Button.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { Loader2 } from 'lucide-react';
import { cn } from '../../lib/utils';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'danger' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  isLoading?: boolean;
  leftIcon?: React.ReactNode;
  rightIcon?: React.ReactNode;
}

export function Button({
  children,
  variant = 'primary',
  size = 'md',
  isLoading = false,
  leftIcon,
  rightIcon,
  className,
  disabled,
  ...props
}: ButtonProps) {
  const variants = {
    primary: 'bg-primary-600 hover:bg-primary-700 text-white',
    secondary: 'bg-surface-200 hover:bg-surface-300 text-surface-700',
    danger: 'bg-red-600 hover:bg-red-700 text-white',
    ghost: 'bg-transparent hover:bg-surface-100 text-surface-700',
  };
  
  const sizes = {
    sm: 'px-3 py-1.5 text-sm',
    md: 'px-4 py-2',
    lg: 'px-6 py-3 text-lg',
  };
  
  return (
    <motion.button
      whileTap={{ scale: 0.98 }}
      className={cn(
        'inline-flex items-center justify-center gap-2 rounded-lg font-medium',
        'transition-colors duration-200',
        'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        variants[variant],
        sizes[size],
        className
      )}
      disabled={disabled || isLoading}
      {...props}
    >
      {isLoading ? (
        <Loader2 className="w-4 h-4 animate-spin" />
      ) : leftIcon}
      {children}
      {!isLoading && rightIcon}
    </motion.button>
  );
}
```

**File: `src/components/ui/Card.tsx`**

```typescript
import React from 'react';
import { cn } from '../../lib/utils';

interface CardProps {
  children: React.ReactNode;
  className?: string;
  onClick?: () => void;
  hoverable?: boolean;
}

export function Card({ children, className, onClick, hoverable }: CardProps) {
  return (
    <div
      className={cn(
        'bg-white rounded-xl shadow-sm border border-surface-200 p-4',
        hoverable && 'hover:shadow-md hover:border-surface-300 transition-all cursor-pointer',
        className
      )}
      onClick={onClick}
    >
      {children}
    </div>
  );
}

export function CardHeader({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn('mb-3', className)}>
      {children}
    </div>
  );
}

export function CardTitle({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <h3 className={cn('text-lg font-semibold text-surface-900', className)}>
      {children}
    </h3>
  );
}

export function CardContent({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn('text-surface-600', className)}>
      {children}
    </div>
  );
}
```

**File: `src/components/ui/Progress.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { cn } from '../../lib/utils';

interface ProgressProps {
  value: number; // 0-100
  size?: 'sm' | 'md' | 'lg';
  showLabel?: boolean;
  className?: string;
}

export function Progress({ value, size = 'md', showLabel = false, className }: ProgressProps) {
  const heights = {
    sm: 'h-1',
    md: 'h-2',
    lg: 'h-3',
  };
  
  return (
    <div className={cn('w-full', className)}>
      <div className={cn('w-full bg-surface-200 rounded-full overflow-hidden', heights[size])}>
        <motion.div
          className="h-full bg-primary-600 rounded-full"
          initial={{ width: 0 }}
          animate={{ width: `${Math.min(100, Math.max(0, value))}%` }}
          transition={{ duration: 0.3, ease: 'easeOut' }}
        />
      </div>
      {showLabel && (
        <span className="text-sm text-surface-500 mt-1">
          {Math.round(value)}%
        </span>
      )}
    </div>
  );
}
```

**File: `src/components/ui/Modal.tsx`**

```typescript
import React, { useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X } from 'lucide-react';
import { cn } from '../../lib/utils';

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl';
}

export function Modal({ isOpen, onClose, title, children, size = 'md' }: ModalProps) {
  // Close on escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    
    if (isOpen) {
      document.addEventListener('keydown', handleEscape);
      document.body.style.overflow = 'hidden';
    }
    
    return () => {
      document.removeEventListener('keydown', handleEscape);
      document.body.style.overflow = '';
    };
  }, [isOpen, onClose]);
  
  const sizes = {
    sm: 'max-w-sm',
    md: 'max-w-md',
    lg: 'max-w-lg',
    xl: 'max-w-xl',
  };
  
  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            className="fixed inset-0 bg-black/50 z-40"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
          />
          
          {/* Modal */}
          <motion.div
            className={cn(
              'fixed left-1/2 top-1/2 z-50 w-full p-6 bg-white rounded-xl shadow-xl',
              sizes[size]
            )}
            initial={{ opacity: 0, scale: 0.95, x: '-50%', y: '-50%' }}
            animate={{ opacity: 1, scale: 1, x: '-50%', y: '-50%' }}
            exit={{ opacity: 0, scale: 0.95, x: '-50%', y: '-50%' }}
          >
            {/* Header */}
            {title && (
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-xl font-semibold text-surface-900">{title}</h2>
                <button
                  onClick={onClose}
                  className="p-1 rounded-lg hover:bg-surface-100 text-surface-500"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>
            )}
            
            {/* Content */}
            {children}
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
```

**File: `src/components/ui/Tabs.tsx`**

```typescript
import React, { createContext, useContext, useState } from 'react';
import { motion } from 'framer-motion';
import { cn } from '../../lib/utils';

interface TabsContextValue {
  activeTab: string;
  setActiveTab: (tab: string) => void;
}

const TabsContext = createContext<TabsContextValue | null>(null);

interface TabsProps {
  defaultValue: string;
  children: React.ReactNode;
  className?: string;
}

export function Tabs({ defaultValue, children, className }: TabsProps) {
  const [activeTab, setActiveTab] = useState(defaultValue);
  
  return (
    <TabsContext.Provider value={{ activeTab, setActiveTab }}>
      <div className={className}>{children}</div>
    </TabsContext.Provider>
  );
}

export function TabsList({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn('flex gap-1 p-1 bg-surface-100 rounded-lg', className)}>
      {children}
    </div>
  );
}

export function TabsTrigger({ 
  value, 
  children,
  className 
}: { 
  value: string; 
  children: React.ReactNode;
  className?: string;
}) {
  const context = useContext(TabsContext);
  if (!context) throw new Error('TabsTrigger must be used within Tabs');
  
  const isActive = context.activeTab === value;
  
  return (
    <button
      onClick={() => context.setActiveTab(value)}
      className={cn(
        'relative px-4 py-2 text-sm font-medium rounded-md transition-colors',
        isActive ? 'text-surface-900' : 'text-surface-500 hover:text-surface-700',
        className
      )}
    >
      {isActive && (
        <motion.div
          layoutId="activeTab"
          className="absolute inset-0 bg-white rounded-md shadow-sm"
          transition={{ type: 'spring', bounce: 0.2, duration: 0.4 }}
        />
      )}
      <span className="relative z-10">{children}</span>
    </button>
  );
}

export function TabsContent({ 
  value, 
  children,
  className 
}: { 
  value: string; 
  children: React.ReactNode;
  className?: string;
}) {
  const context = useContext(TabsContext);
  if (!context) throw new Error('TabsContent must be used within Tabs');
  
  if (context.activeTab !== value) return null;
  
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -10 }}
      className={className}
    >
      {children}
    </motion.div>
  );
}
```

**File: `src/components/ui/Toast.tsx`**

```typescript
import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { CheckCircle, XCircle, Info, AlertTriangle, X } from 'lucide-react';
import { useToastStore } from '../../stores/toastStore';

const icons = {
  success: CheckCircle,
  error: XCircle,
  info: Info,
  warning: AlertTriangle,
};

const colors = {
  success: 'bg-green-50 border-green-200 text-green-800',
  error: 'bg-red-50 border-red-200 text-red-800',
  info: 'bg-blue-50 border-blue-200 text-blue-800',
  warning: 'bg-yellow-50 border-yellow-200 text-yellow-800',
};

const iconColors = {
  success: 'text-green-500',
  error: 'text-red-500',
  info: 'text-blue-500',
  warning: 'text-yellow-500',
};

export function ToastContainer() {
  const { toasts, removeToast } = useToastStore();
  
  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      <AnimatePresence>
        {toasts.map((toast) => {
          const Icon = icons[toast.type];
          
          return (
            <motion.div
              key={toast.id}
              initial={{ opacity: 0, x: 50 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 50 }}
              className={`flex items-start gap-3 p-4 rounded-lg border shadow-lg max-w-sm ${colors[toast.type]}`}
            >
              <Icon className={`w-5 h-5 flex-shrink-0 ${iconColors[toast.type]}`} />
              
              <div className="flex-1 min-w-0">
                <p className="font-medium">{toast.title}</p>
                {toast.message && (
                  <p className="text-sm opacity-80 mt-0.5">{toast.message}</p>
                )}
              </div>
              
              <button
                onClick={() => removeToast(toast.id)}
                className="flex-shrink-0 opacity-50 hover:opacity-100"
              >
                <X className="w-4 h-4" />
              </button>
            </motion.div>
          );
        })}
      </AnimatePresence>
    </div>
  );
}
```

**File: `src/lib/utils.ts`**

```typescript
import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

Add `clsx` and `tailwind-merge`:
```bash
pnpm add clsx tailwind-merge
```

### Step 8: Layout Components

**File: `src/components/layout/AppShell.tsx`**

```typescript
import React from 'react';
import { Outlet } from 'react-router-dom';
import { Navigation } from './Navigation';
import { TitleBar } from './TitleBar';
import { ToastContainer } from '../ui/Toast';

export function AppShell() {
  return (
    <div className="h-screen flex flex-col bg-surface-50">
      {/* Custom title bar for frameless window */}
      <TitleBar />
      
      {/* Main content area */}
      <main className="flex-1 overflow-hidden">
        <Outlet />
      </main>
      
      {/* Bottom navigation */}
      <Navigation />
      
      {/* Toast notifications */}
      <ToastContainer />
    </div>
  );
}
```

**File: `src/components/layout/TitleBar.tsx`**

```typescript
import React from 'react';
import { useLocation } from 'react-router-dom';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, X } from 'lucide-react';

const titles: Record<string, string> = {
  '/': 'Recording',
  '/library': 'Library',
  '/chat': 'Chat',
  '/settings': 'Settings',
};

export function TitleBar() {
  const location = useLocation();
  const appWindow = getCurrentWindow();
  
  const handleMinimize = () => appWindow.minimize();
  const handleMaximize = () => appWindow.toggleMaximize();
  const handleClose = () => appWindow.close();
  
  // Get title based on current route
  const title = titles[location.pathname] ?? 'Meeting Scribe';
  
  return (
    <div 
      className="h-8 flex items-center justify-between bg-white border-b border-surface-200 select-none"
      data-tauri-drag-region
    >
      {/* App title */}
      <div className="px-4 flex items-center gap-2" data-tauri-drag-region>
        <span className="text-sm font-medium text-surface-700" data-tauri-drag-region>
          Meeting Scribe
        </span>
        <span className="text-surface-400">—</span>
        <span className="text-sm text-surface-500" data-tauri-drag-region>
          {title}
        </span>
      </div>
      
      {/* Window controls */}
      <div className="flex">
        <button
          onClick={handleMinimize}
          className="w-12 h-8 flex items-center justify-center hover:bg-surface-100 transition-colors"
        >
          <Minus className="w-4 h-4 text-surface-600" />
        </button>
        <button
          onClick={handleMaximize}
          className="w-12 h-8 flex items-center justify-center hover:bg-surface-100 transition-colors"
        >
          <Square className="w-3 h-3 text-surface-600" />
        </button>
        <button
          onClick={handleClose}
          className="w-12 h-8 flex items-center justify-center hover:bg-red-500 hover:text-white transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
```

**File: `src/components/layout/Navigation.tsx`**

```typescript
import React from 'react';
import { NavLink } from 'react-router-dom';
import { motion } from 'framer-motion';
import { Mic, Library, MessageSquare, Settings } from 'lucide-react';
import { cn } from '../../lib/utils';
import { useRecordingStore } from '../../stores/recordingStore';

const navItems = [
  { path: '/', icon: Mic, label: 'Record' },
  { path: '/library', icon: Library, label: 'Library' },
  { path: '/chat', icon: MessageSquare, label: 'Chat' },
  { path: '/settings', icon: Settings, label: 'Settings' },
];

export function Navigation() {
  const recordingState = useRecordingStore((s) => s.state);
  const isRecording = recordingState === 'recording' || recordingState === 'paused';
  
  return (
    <nav className="h-16 bg-white border-t border-surface-200 flex items-center justify-around px-4">
      {navItems.map(({ path, icon: Icon, label }) => (
        <NavLink
          key={path}
          to={path}
          className={({ isActive }) => cn(
            'relative flex flex-col items-center gap-1 px-6 py-2 rounded-lg transition-colors',
            isActive 
              ? 'text-primary-600' 
              : 'text-surface-500 hover:text-surface-700 hover:bg-surface-50'
          )}
        >
          {({ isActive }) => (
            <>
              <div className="relative">
                <Icon className="w-5 h-5" />
                
                {/* Recording indicator on Record tab */}
                {path === '/' && isRecording && (
                  <motion.div
                    className="absolute -top-0.5 -right-0.5 w-2 h-2 bg-red-500 rounded-full"
                    animate={{ scale: [1, 1.2, 1] }}
                    transition={{ repeat: Infinity, duration: 1.5 }}
                  />
                )}
              </div>
              
              <span className="text-xs font-medium">{label}</span>
              
              {/* Active indicator */}
              {isActive && (
                <motion.div
                  layoutId="navIndicator"
                  className="absolute -bottom-1 left-1/2 -translate-x-1/2 w-1 h-1 bg-primary-600 rounded-full"
                />
              )}
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
}
```

### Step 9: Recording View

**File: `src/components/recording/RecordingView.tsx`**

```typescript
import React, { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { Waveform } from './Waveform';
import { RecordingControls } from './RecordingControls';
import { RecordingTimer } from './RecordingTimer';
import { AudioLevelMeter } from './AudioLevelMeter';
import { useRecording } from '../../hooks/useRecording';

export function RecordingView() {
  const recording = useRecording();
  const navigate = useNavigate();
  
  const handleStop = useCallback(async () => {
    const meeting = await recording.stop();
    // Navigate to meeting detail for processing
    navigate(`/meeting/${meeting.id}`);
  }, [recording, navigate]);
  
  const isIdle = recording.state === 'idle';
  const isRecording = recording.state === 'recording';
  const isPaused = recording.state === 'paused';
  const isStopping = recording.state === 'stopping';
  
  return (
    <div className="h-full flex flex-col items-center justify-center p-8">
      {/* Recording status */}
      <AnimatePresence mode="wait">
        {isIdle ? (
          <motion.div
            key="idle"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="text-center mb-8"
          >
            <h1 className="text-2xl font-semibold text-surface-900 mb-2">
              Ready to Record
            </h1>
            <p className="text-surface-500">
              Click the button below to start recording your meeting
            </p>
          </motion.div>
        ) : (
          <motion.div
            key="recording"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            className="w-full max-w-2xl"
          >
            {/* Timer */}
            <div className="text-center mb-6">
              <RecordingTimer 
                durationMs={recording.durationMs} 
                isPaused={isPaused}
              />
              <div className="flex items-center justify-center gap-2 mt-2">
                {isRecording && (
                  <motion.div
                    className="w-2 h-2 bg-red-500 rounded-full"
                    animate={{ opacity: [1, 0.5, 1] }}
                    transition={{ repeat: Infinity, duration: 1 }}
                  />
                )}
                <span className="text-sm text-surface-500 uppercase tracking-wide">
                  {isPaused ? 'Paused' : isStopping ? 'Stopping...' : 'Recording'}
                </span>
              </div>
            </div>
            
            {/* Waveforms */}
            <div className="bg-white rounded-xl border border-surface-200 p-6 mb-6">
              <div className="space-y-4">
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-medium text-surface-700">You</span>
                    <AudioLevelMeter 
                      level={recording.waveformData?.you.rms ?? 0}
                      isActive={recording.speechDetectedYou}
                    />
                  </div>
                  <Waveform 
                    samples={recording.waveformData?.you.samples ?? []}
                    color="primary"
                  />
                </div>
                
                <div className="border-t border-surface-100" />
                
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-medium text-surface-700">Others</span>
                    <AudioLevelMeter 
                      level={recording.waveformData?.others.rms ?? 0}
                      isActive={recording.speechDetectedOthers}
                    />
                  </div>
                  <Waveform 
                    samples={recording.waveformData?.others.samples ?? []}
                    color="secondary"
                  />
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      
      {/* Controls */}
      <RecordingControls
        state={recording.state}
        onStart={() => recording.start()}
        onStop={handleStop}
        onPause={() => recording.pause()}
        onResume={() => recording.resume()}
      />
    </div>
  );
}
```

**File: `src/components/recording/Waveform.tsx`**

```typescript
import React, { useMemo } from 'react';
import { motion } from 'framer-motion';
import { cn } from '../../lib/utils';

interface WaveformProps {
  samples: number[];
  color?: 'primary' | 'secondary';
  height?: number;
  barCount?: number;
}

export function Waveform({ 
  samples, 
  color = 'primary',
  height = 64,
  barCount = 64
}: WaveformProps) {
  // Downsample or interpolate to barCount bars
  const bars = useMemo(() => {
    if (samples.length === 0) {
      return new Array(barCount).fill(0.1);
    }
    
    const result: number[] = [];
    const step = samples.length / barCount;
    
    for (let i = 0; i < barCount; i++) {
      const start = Math.floor(i * step);
      const end = Math.floor((i + 1) * step);
      const slice = samples.slice(start, end);
      
      if (slice.length > 0) {
        // Use max value in slice for more dynamic display
        const value = Math.max(...slice.map(Math.abs));
        result.push(Math.max(0.1, Math.min(1, value)));
      } else {
        result.push(0.1);
      }
    }
    
    return result;
  }, [samples, barCount]);
  
  const colorClass = color === 'primary' 
    ? 'bg-primary-500' 
    : 'bg-surface-400';
  
  return (
    <div 
      className="flex items-center justify-center gap-0.5"
      style={{ height }}
    >
      {bars.map((value, index) => (
        <motion.div
          key={index}
          className={cn('w-1 rounded-full', colorClass)}
          initial={{ height: '10%' }}
          animate={{ height: `${value * 100}%` }}
          transition={{ 
            type: 'spring', 
            stiffness: 300, 
            damping: 20,
            mass: 0.5 
          }}
        />
      ))}
    </div>
  );
}
```

**File: `src/components/recording/RecordingControls.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { Mic, Square, Pause, Play } from 'lucide-react';
import { Button } from '../ui/Button';
import type { RecordingState } from '../../types/recording';

interface RecordingControlsProps {
  state: RecordingState;
  onStart: () => void;
  onStop: () => void;
  onPause: () => void;
  onResume: () => void;
}

export function RecordingControls({
  state,
  onStart,
  onStop,
  onPause,
  onResume,
}: RecordingControlsProps) {
  const isIdle = state === 'idle';
  const isRecording = state === 'recording';
  const isPaused = state === 'paused';
  const isStopping = state === 'stopping';
  
  if (isIdle) {
    return (
      <motion.div
        initial={{ scale: 0.8, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ delay: 0.2 }}
      >
        <button
          onClick={onStart}
          className="w-20 h-20 bg-red-500 hover:bg-red-600 rounded-full flex items-center justify-center shadow-lg hover:shadow-xl transition-all"
        >
          <Mic className="w-8 h-8 text-white" />
        </button>
      </motion.div>
    );
  }
  
  return (
    <motion.div
      initial={{ scale: 0.8, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      className="flex items-center gap-4"
    >
      {/* Stop button */}
      <button
        onClick={onStop}
        disabled={isStopping}
        className="w-16 h-16 bg-red-500 hover:bg-red-600 disabled:bg-red-300 rounded-full flex items-center justify-center shadow-lg transition-all"
      >
        <Square className="w-6 h-6 text-white" fill="white" />
      </button>
      
      {/* Pause/Resume button */}
      <button
        onClick={isPaused ? onResume : onPause}
        disabled={isStopping}
        className="w-12 h-12 bg-surface-200 hover:bg-surface-300 disabled:bg-surface-100 rounded-full flex items-center justify-center transition-all"
      >
        {isPaused ? (
          <Play className="w-5 h-5 text-surface-700 ml-0.5" fill="currentColor" />
        ) : (
          <Pause className="w-5 h-5 text-surface-700" fill="currentColor" />
        )}
      </button>
    </motion.div>
  );
}
```

**File: `src/components/recording/RecordingTimer.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';

interface RecordingTimerProps {
  durationMs: number;
  isPaused?: boolean;
}

export function RecordingTimer({ durationMs, isPaused }: RecordingTimerProps) {
  const hours = Math.floor(durationMs / 3600000);
  const minutes = Math.floor((durationMs % 3600000) / 60000);
  const seconds = Math.floor((durationMs % 60000) / 1000);
  
  const format = (n: number) => n.toString().padStart(2, '0');
  
  return (
    <motion.div
      className="font-mono text-5xl font-semibold text-surface-900 tabular-nums"
      animate={isPaused ? { opacity: [1, 0.5, 1] } : { opacity: 1 }}
      transition={isPaused ? { repeat: Infinity, duration: 1 } : {}}
    >
      {hours > 0 && <span>{format(hours)}:</span>}
      <span>{format(minutes)}</span>
      <span className="text-surface-400">:</span>
      <span>{format(seconds)}</span>
    </motion.div>
  );
}
```

**File: `src/components/recording/AudioLevelMeter.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { cn } from '../../lib/utils';

interface AudioLevelMeterProps {
  level: number; // 0-1
  isActive?: boolean;
}

export function AudioLevelMeter({ level, isActive }: AudioLevelMeterProps) {
  // Convert to dB-like scale for more natural display
  const displayLevel = Math.pow(level, 0.5) * 100;
  
  return (
    <div className="flex items-center gap-2">
      {/* Speech indicator */}
      <div 
        className={cn(
          'w-2 h-2 rounded-full transition-colors',
          isActive ? 'bg-green-500' : 'bg-surface-300'
        )}
      />
      
      {/* Level bar */}
      <div className="w-20 h-2 bg-surface-200 rounded-full overflow-hidden">
        <motion.div
          className={cn(
            'h-full rounded-full',
            displayLevel > 80 ? 'bg-red-500' : displayLevel > 50 ? 'bg-yellow-500' : 'bg-green-500'
          )}
          animate={{ width: `${displayLevel}%` }}
          transition={{ type: 'spring', stiffness: 300, damping: 30 }}
        />
      </div>
    </div>
  );
}
```

### Step 10: Library View

**File: `src/components/library/LibraryView.tsx`**

```typescript
import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { Search, Filter, Loader2 } from 'lucide-react';
import { MeetingSearch } from './MeetingSearch';
import { MeetingFilters } from './MeetingFilters';
import { TimelineGroup } from './TimelineGroup';
import { MeetingCard } from './MeetingCard';
import { useMeetings, useSearch } from '../../hooks/useMeetings';

export function LibraryView() {
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [filterStatus, setFilterStatus] = useState<string | null>(null);
  
  const { data: meetingGroups, isLoading, error } = useMeetings();
  const { data: searchResults, isLoading: isSearching } = useSearch(searchQuery);
  
  const isSearchMode = searchQuery.length >= 2;
  
  const handleMeetingClick = (id: string) => {
    navigate(`/meeting/${id}`);
  };
  
  return (
    <div className="h-full flex flex-col">
      {/* Search header */}
      <div className="p-4 bg-white border-b border-surface-200">
        <div className="flex gap-2">
          <MeetingSearch 
            value={searchQuery}
            onChange={setSearchQuery}
            isLoading={isSearching}
          />
          <MeetingFilters
            status={filterStatus}
            onStatusChange={setFilterStatus}
          />
        </div>
      </div>
      
      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {isLoading ? (
          <div className="flex items-center justify-center h-64">
            <Loader2 className="w-8 h-8 text-primary-500 animate-spin" />
          </div>
        ) : error ? (
          <div className="text-center text-red-500 py-8">
            Failed to load meetings
          </div>
        ) : isSearchMode ? (
          // Search results
          <div className="space-y-2">
            <h2 className="text-sm font-medium text-surface-500 mb-3">
              {searchResults?.length ?? 0} results for "{searchQuery}"
            </h2>
            <AnimatePresence>
              {searchResults?.map((result, index) => (
                <motion.div
                  key={result.meeting_id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  transition={{ delay: index * 0.05 }}
                >
                  <MeetingCard
                    id={result.meeting_id}
                    title={result.title}
                    snippet={result.snippet}
                    onClick={() => handleMeetingClick(result.meeting_id)}
                    isSearchResult
                  />
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        ) : (
          // Timeline view
          <div className="space-y-6">
            {meetingGroups?.map((group) => (
              <TimelineGroup
                key={group.date}
                label={group.label}
                meetings={group.meetings}
                onMeetingClick={handleMeetingClick}
              />
            ))}
            
            {meetingGroups?.length === 0 && (
              <div className="text-center py-16">
                <p className="text-surface-500 mb-2">No meetings yet</p>
                <p className="text-sm text-surface-400">
                  Start recording to see your meetings here
                </p>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
```

**File: `src/components/library/MeetingCard.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { Clock, ChevronRight, Tag } from 'lucide-react';
import { Card } from '../ui/Card';
import type { Meeting } from '../../types/meeting';
import { formatDuration, formatTime } from '../../lib/formatters';

interface MeetingCardProps {
  meeting?: Meeting;
  id?: string;
  title?: string;
  snippet?: string;
  onClick: () => void;
  isSearchResult?: boolean;
}

export function MeetingCard({ 
  meeting, 
  id,
  title,
  snippet,
  onClick,
  isSearchResult 
}: MeetingCardProps) {
  const displayTitle = meeting?.title ?? title ?? 'Untitled Meeting';
  const displayDate = meeting ? new Date(meeting.created_at) : null;
  
  return (
    <Card hoverable onClick={onClick} className="group">
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          {/* Date and time */}
          {displayDate && (
            <div className="flex items-center gap-2 text-sm text-surface-500 mb-1">
              <span>{formatTime(displayDate)}</span>
              {meeting && (
                <>
                  <span className="text-surface-300">•</span>
                  <span className="flex items-center gap-1">
                    <Clock className="w-3 h-3" />
                    {formatDuration(meeting.duration_ms)}
                  </span>
                </>
              )}
            </div>
          )}
          
          {/* Title */}
          <h3 className="font-medium text-surface-900 truncate">
            {displayTitle}
          </h3>
          
          {/* Snippet or summary */}
          {(snippet || meeting?.status === 'ready') && (
            <p className="text-sm text-surface-500 mt-1 line-clamp-2">
              {isSearchResult && snippet ? (
                <span dangerouslySetInnerHTML={{ __html: snippet }} />
              ) : (
                snippet ?? 'Meeting recorded successfully'
              )}
            </p>
          )}
          
          {/* Status badges */}
          {meeting?.status === 'processing' && (
            <span className="inline-flex items-center px-2 py-0.5 mt-2 text-xs font-medium bg-yellow-100 text-yellow-800 rounded-full">
              Processing...
            </span>
          )}
          
          {/* Tags */}
          {meeting?.tags && meeting.tags.length > 0 && (
            <div className="flex items-center gap-1 mt-2">
              <Tag className="w-3 h-3 text-surface-400" />
              <div className="flex gap-1">
                {meeting.tags.map((tag) => (
                  <span
                    key={tag}
                    className="text-xs text-primary-600 bg-primary-50 px-1.5 py-0.5 rounded"
                  >
                    #{tag}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
        
        {/* Arrow */}
        <ChevronRight className="w-5 h-5 text-surface-400 group-hover:text-surface-600 transition-colors flex-shrink-0 ml-2" />
      </div>
    </Card>
  );
}
```

**File: `src/components/library/MeetingSearch.tsx`**

```typescript
import React from 'react';
import { Search, X, Loader2 } from 'lucide-react';

interface MeetingSearchProps {
  value: string;
  onChange: (value: string) => void;
  isLoading?: boolean;
}

export function MeetingSearch({ value, onChange, isLoading }: MeetingSearchProps) {
  return (
    <div className="relative flex-1">
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-surface-400" />
      
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Search meetings..."
        className="w-full pl-10 pr-10 py-2 border border-surface-300 rounded-lg
                   focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent
                   placeholder:text-surface-400"
      />
      
      {/* Loading/Clear */}
      <div className="absolute right-3 top-1/2 -translate-y-1/2">
        {isLoading ? (
          <Loader2 className="w-4 h-4 text-surface-400 animate-spin" />
        ) : value && (
          <button
            onClick={() => onChange('')}
            className="text-surface-400 hover:text-surface-600"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  );
}
```

**File: `src/components/library/MeetingFilters.tsx`**

```typescript
import React, { useState } from 'react';
import { Filter, ChevronDown } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

interface MeetingFiltersProps {
  status: string | null;
  onStatusChange: (status: string | null) => void;
}

export function MeetingFilters({ status, onStatusChange }: MeetingFiltersProps) {
  const [isOpen, setIsOpen] = useState(false);
  
  const options = [
    { value: null, label: 'All' },
    { value: 'ready', label: 'Ready' },
    { value: 'processing', label: 'Processing' },
    { value: 'error', label: 'Failed' },
  ];
  
  const currentLabel = options.find((o) => o.value === status)?.label ?? 'All';
  
  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-3 py-2 border border-surface-300 rounded-lg hover:bg-surface-50 transition-colors"
      >
        <Filter className="w-4 h-4 text-surface-500" />
        <span className="text-sm text-surface-700">{currentLabel}</span>
        <ChevronDown className="w-4 h-4 text-surface-400" />
      </button>
      
      <AnimatePresence>
        {isOpen && (
          <>
            {/* Backdrop */}
            <div 
              className="fixed inset-0 z-10"
              onClick={() => setIsOpen(false)}
            />
            
            {/* Dropdown */}
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              className="absolute right-0 top-full mt-1 w-40 bg-white border border-surface-200 rounded-lg shadow-lg z-20 py-1"
            >
              {options.map((option) => (
                <button
                  key={option.value ?? 'all'}
                  onClick={() => {
                    onStatusChange(option.value);
                    setIsOpen(false);
                  }}
                  className={`w-full px-3 py-2 text-left text-sm hover:bg-surface-50 transition-colors ${
                    status === option.value ? 'text-primary-600 bg-primary-50' : 'text-surface-700'
                  }`}
                >
                  {option.label}
                </button>
              ))}
            </motion.div>
          </>
        )}
      </AnimatePresence>
    </div>
  );
}
```

**File: `src/components/library/TimelineGroup.tsx`**

```typescript
import React from 'react';
import { MeetingCard } from './MeetingCard';
import type { Meeting } from '../../types/meeting';

interface TimelineGroupProps {
  label: string;
  meetings: Meeting[];
  onMeetingClick: (id: string) => void;
}

export function TimelineGroup({ label, meetings, onMeetingClick }: TimelineGroupProps) {
  return (
    <div>
      {/* Date label */}
      <div className="flex items-center gap-3 mb-3">
        <div className="h-px flex-1 bg-surface-200" />
        <span className="text-sm font-medium text-surface-500">{label}</span>
        <div className="h-px flex-1 bg-surface-200" />
      </div>
      
      {/* Meetings */}
      <div className="space-y-2">
        {meetings.map((meeting) => (
          <MeetingCard
            key={meeting.id}
            meeting={meeting}
            onClick={() => onMeetingClick(meeting.id)}
          />
        ))}
      </div>
    </div>
  );
}
```

### Step 11: Formatters

**File: `src/lib/formatters.ts`**

```typescript
export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  
  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  }
  if (minutes > 0) {
    return `${minutes}m`;
  }
  return `${seconds}s`;
}

export function formatTime(date: Date): string {
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  });
}

export function formatDate(date: Date): string {
  return date.toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
    year: 'numeric',
  });
}

export function formatTimestamp(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  
  const pad = (n: number) => n.toString().padStart(2, '0');
  
  if (hours > 0) {
    return `${pad(hours)}:${pad(minutes % 60)}:${pad(seconds % 60)}`;
  }
  return `${pad(minutes)}:${pad(seconds % 60)}`;
}

export function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIndex = 0;
  
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}

export function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSeconds = Math.floor(diffMs / 1000);
  const diffMinutes = Math.floor(diffSeconds / 60);
  const diffHours = Math.floor(diffMinutes / 60);
  const diffDays = Math.floor(diffHours / 24);
  
  if (diffSeconds < 60) {
    return 'just now';
  }
  if (diffMinutes < 60) {
    return `${diffMinutes}m ago`;
  }
  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }
  if (diffDays < 7) {
    return `${diffDays}d ago`;
  }
  
  return formatDate(date);
}
```

### Step 12: App Entry Point

**File: `src/App.tsx`**

```typescript
import React from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppShell } from './components/layout/AppShell';
import { RecordingView } from './components/recording/RecordingView';
import { LibraryView } from './components/library/LibraryView';
import { MeetingDetailView } from './components/meeting/MeetingDetailView';
import { ChatView } from './components/chat/ChatView';
import { SettingsView } from './components/settings/SettingsView';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30 * 1000,
      retry: 1,
    },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<RecordingView />} />
            <Route path="/library" element={<LibraryView />} />
            <Route path="/meeting/:id" element={<MeetingDetailView />} />
            <Route path="/chat" element={<ChatView />} />
            <Route path="/settings" element={<SettingsView />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
```

Add React Router:
```bash
pnpm add react-router-dom
```

**File: `src/main.tsx`**

```typescript
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

## Remaining Components

Due to space constraints, the following components should be implemented following the same patterns:

### Meeting Detail View
- `src/components/meeting/MeetingDetailView.tsx` - Main meeting view with tabs
- `src/components/meeting/TranscriptPanel.tsx` - Scrollable transcript with timestamps
- `src/components/meeting/SummaryPanel.tsx` - Summary display with generation button
- `src/components/meeting/NotesPanel.tsx` - Editable notes with markdown support
- `src/components/meeting/AudioPlayer.tsx` - Playback with seek to timestamp
- `src/components/meeting/MeetingHeader.tsx` - Title, date, tags editing

### Chat View
- `src/components/chat/ChatView.tsx` - Main chat interface
- `src/components/chat/ChatMessage.tsx` - Message bubbles with streaming support
- `src/components/chat/ChatInput.tsx` - Message input with send button
- `src/components/chat/SourceCard.tsx` - Clickable citations
- `src/components/chat/ChatSuggestions.tsx` - Suggested questions

### Settings View
- `src/components/settings/SettingsView.tsx` - Settings navigation
- `src/components/settings/ModelSettings.tsx` - Model download/selection
- `src/components/settings/AudioSettings.tsx` - Device configuration
- `src/components/settings/StorageSettings.tsx` - Storage stats, cleanup
- `src/components/settings/ModelDownloadCard.tsx` - Download progress UI

## Tauri Window Configuration

**File: `src-tauri/tauri.conf.json` (relevant sections)**

```json
{
  "app": {
    "windows": [
      {
        "title": "Meeting Scribe",
        "width": 1024,
        "height": 768,
        "minWidth": 800,
        "minHeight": 600,
        "decorations": false,
        "transparent": false,
        "resizable": true,
        "fullscreen": false
      }
    ]
  }
}
```

## Testing

### Component Testing with Vitest

```bash
pnpm add -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

**File: `vitest.config.ts`**

```typescript
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    globals: true,
  },
});
```

**Example test: `src/components/ui/Button.test.tsx`**

```typescript
import { render, screen, fireEvent } from '@testing-library/react';
import { Button } from './Button';

describe('Button', () => {
  it('renders children', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByText('Click me')).toBeInTheDocument();
  });
  
  it('calls onClick when clicked', () => {
    const handleClick = vi.fn();
    render(<Button onClick={handleClick}>Click me</Button>);
    fireEvent.click(screen.getByText('Click me'));
    expect(handleClick).toHaveBeenCalled();
  });
  
  it('shows loading state', () => {
    render(<Button isLoading>Loading</Button>);
    expect(screen.getByRole('button')).toBeDisabled();
  });
});
```

## Acceptance Criteria

- [ ] All four main views implemented (Recording, Library, Chat, Settings)
- [ ] Recording view shows dual waveforms and controls
- [ ] Library shows meetings grouped by date with search
- [ ] Meeting detail shows transcript, summary, notes tabs
- [ ] Chat interface supports streaming responses with citations
- [ ] Settings allows model and device configuration
- [ ] Custom frameless window with working controls
- [ ] Toast notifications for user feedback
- [ ] Responsive layout for various window sizes
- [ ] Smooth animations and transitions
- [ ] Accessible keyboard navigation

## References

### UI Libraries
- [Tailwind CSS Documentation](https://tailwindcss.com/docs)
- [Framer Motion](https://www.framer.com/motion/)
- [Lucide Icons](https://lucide.dev/icons/)

### React Patterns
- [React Query (TanStack)](https://tanstack.com/query/latest/docs/react/overview)
- [Zustand](https://github.com/pmndrs/zustand)

### Tauri Integration
- [Tauri JS API](https://tauri.app/v2/reference/js/)
- [Tauri Events](https://tauri.app/v2/reference/js/event/)
- [Custom Window Decorations](https://tauri.app/v2/guides/window-customization/)

---

**Next:** [09 - RAG Implementation](./09-rag-implementation.md) - Vector search and chat with meetings
