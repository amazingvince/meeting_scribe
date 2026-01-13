/**
 * Recording state store
 * Manages recording state, waveform data, and duration
 */

import { create } from 'zustand';
import type {
  RecordingState,
  WaveformUpdate,
  RecordingResult,
} from '../types';
import * as api from '../lib/tauri';

interface RecordingStore {
  // State
  state: RecordingState;
  meetingId: string | null;
  durationMs: number;
  waveform: WaveformUpdate | null;
  error: string | null;
  isLoading: boolean;

  // Actions
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<RecordingResult | null>;
  refreshState: () => Promise<void>;
  setWaveform: (waveform: WaveformUpdate) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const initialState = {
  state: 'Idle' as RecordingState,
  meetingId: null,
  durationMs: 0,
  waveform: null,
  error: null,
  isLoading: false,
};

export const useRecordingStore = create<RecordingStore>((set, get) => ({
  ...initialState,

  startRecording: async () => {
    set({ isLoading: true, error: null });
    try {
      const meetingId = await api.startRecording();
      set({
        state: 'Recording',
        meetingId,
        durationMs: 0,
        isLoading: false,
      });
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
    }
  },

  stopRecording: async () => {
    const { state } = get();
    if (state !== 'Recording') return null;

    set({ isLoading: true, error: null });
    try {
      const result = await api.stopRecording();
      set({
        state: 'Idle',
        meetingId: null,
        durationMs: 0,
        waveform: null,
        isLoading: false,
      });
      return result;
    } catch (e) {
      set({
        error: e instanceof Error ? e.message : String(e),
        isLoading: false,
      });
      return null;
    }
  },

  refreshState: async () => {
    try {
      const response = await api.getRecordingState();
      set({
        state: response.state,
        meetingId: response.meeting_id,
        durationMs: response.duration_ms,
      });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  setWaveform: (waveform) => {
    set({ waveform, durationMs: waveform.duration_ms });
  },

  setError: (error) => {
    set({ error });
  },

  reset: () => {
    set(initialState);
  },
}));
