/**
 * Recording hook
 * Provides recording controls and event subscriptions
 */

import { useCallback, useEffect } from 'react';
import { useRecordingStore } from '../stores';
import { useTauriEvent } from './useTauriEvent';
import type { WaveformUpdate } from '../types';

export function useRecording() {
  const store = useRecordingStore();
  const { setWaveform, refreshState } = store;

  // Subscribe to waveform updates
  useTauriEvent<WaveformUpdate>('waveform-update', (waveform) => {
    setWaveform(waveform);
  });

  // Refresh state on mount
  useEffect(() => {
    refreshState();
  }, [refreshState]);

  const startRecording = useCallback(async () => {
    await store.startRecording();
  }, [store]);

  const stopRecording = useCallback(async () => {
    return await store.stopRecording();
  }, [store]);

  return {
    // State
    state: store.state,
    meetingId: store.meetingId,
    durationMs: store.durationMs,
    waveform: store.waveform,
    isRecording: store.state === 'Recording',
    isLoading: store.isLoading,
    error: store.error,

    // Actions
    startRecording,
    stopRecording,
    refreshState: store.refreshState,
    clearError: () => store.setError(null),
  };
}
