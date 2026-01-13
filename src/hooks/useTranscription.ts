/**
 * Transcription hook
 * Handles transcription progress and processing
 */

import { useState, useCallback } from 'react';
import { useTauriEvent } from './useTauriEvent';
import { useSettingsStore, useToastStore } from '../stores';
import * as api from '../lib/tauri';

interface ProcessingProgress {
  meeting_id: string;
  stage: string;
  percent: number;
  message: string;
}

export function useTranscription() {
  const settings = useSettingsStore();
  const toast = useToastStore();

  const [isProcessing, setIsProcessing] = useState(false);
  const [progress, setProgress] = useState<ProcessingProgress | null>(null);

  // Subscribe to processing progress
  useTauriEvent<ProcessingProgress>('meeting-processing-progress', (data) => {
    setProgress(data);
  });

  const processMeeting = useCallback(
    async (meetingId: string, micPath?: string, systemPath?: string) => {
      if (!settings.transcriptionReady) {
        toast.error(
          'Transcription not ready',
          'Please download and initialize a transcription model first.'
        );
        return null;
      }

      setIsProcessing(true);
      setProgress(null);

      try {
        const result = await api.processMeeting(meetingId, micPath, systemPath);
        toast.success(
          'Transcription complete',
          `Processed ${result.mic_segment_count + result.system_segment_count} segments`
        );
        return result;
      } catch (e) {
        toast.error(
          'Transcription failed',
          e instanceof Error ? e.message : String(e)
        );
        return null;
      } finally {
        setIsProcessing(false);
        setProgress(null);
      }
    },
    [settings.transcriptionReady, toast]
  );

  const transcribeFile = useCallback(
    async (audioPath: string) => {
      if (!settings.transcriptionReady) {
        toast.error(
          'Transcription not ready',
          'Please download and initialize a transcription model first.'
        );
        return null;
      }

      try {
        const segments = await api.transcribeFile(audioPath);
        return segments;
      } catch (e) {
        toast.error(
          'Transcription failed',
          e instanceof Error ? e.message : String(e)
        );
        return null;
      }
    },
    [settings.transcriptionReady, toast]
  );

  return {
    isReady: settings.transcriptionReady,
    isProcessing,
    progress,
    processMeeting,
    transcribeFile,
  };
}
