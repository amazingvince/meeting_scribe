/**
 * Models hook
 * Provides model status and download progress
 */

import { useEffect } from 'react';
import { useSettingsStore } from '../stores';
import { useTauriEvent } from './useTauriEvent';
import type {
  TranscriptionBackend,
  LlmDownloadProgress,
  EmbeddingDownloadProgress,
} from '../types';
import type { DownloadProgressEvent, BatchEmbedProgress } from '../lib/tauri';

const transcriptionModelIdMap: Record<string, TranscriptionBackend> = {
  'parakeet-tdt-0.6b-v3-int8': 'Parakeet',
  'whisper-medium-q4_1': 'Whisper',
  'moonshine-tiny': 'Moonshine',
};

function asTranscriptionBackend(
  value: string | null | undefined
): TranscriptionBackend | undefined {
  if (value === 'Parakeet' || value === 'Whisper' || value === 'Moonshine') {
    return value;
  }
  return undefined;
}

export function useModels() {
  const store = useSettingsStore();
  const {
    setDownloadProgress,
    refreshModelStatus,
    setBatchEmbedProgress,
    refreshUnembeddedCount,
  } = store;

  // Subscribe to download progress events
  useTauriEvent<DownloadProgressEvent>('model-download-progress', (data) => {
    const activeDownload = useSettingsStore.getState().downloadingModel;
    const modelKey =
      transcriptionModelIdMap[data.model_id] ??
      asTranscriptionBackend(activeDownload);
    setDownloadProgress({
      progress: data.percent,
      modelKey,
      sourceModelId: data.model_id,
      stage: data.stage,
      message: data.message,
      downloadedBytes: data.downloaded_bytes ?? null,
      totalBytes: data.total_bytes ?? null,
      speedBps: data.speed_bps ?? null,
    });
  });

  useTauriEvent<LlmDownloadProgress>('llm-download-progress', (data) => {
    setDownloadProgress({
      progress: data.percent,
      modelKey: data.model,
      sourceModelId: data.model,
      stage: 'Downloading',
      message: `Downloading ${data.model}...`,
      downloadedBytes: data.downloaded_bytes,
      totalBytes: data.total_bytes,
      speedBps: data.speed_bps ?? null,
    });
  });

  useTauriEvent<EmbeddingDownloadProgress>(
    'embedding-download-progress',
    (data) => {
      setDownloadProgress({
        progress: data.percent,
        modelKey: 'embedding',
        sourceModelId: data.model_id,
        stage: data.status,
        message: `Downloading ${data.file}...`,
        downloadedBytes: data.downloaded,
        totalBytes: data.total,
        speedBps: null,
        file: data.file,
      });
    }
  );

  // Listen for batch embed progress
  useTauriEvent<BatchEmbedProgress>('batch-embed-progress', (data) => {
    if (data.status === 'complete') {
      setBatchEmbedProgress(null);
      refreshUnembeddedCount();
    } else {
      setBatchEmbedProgress({
        current: data.current,
        total: data.total,
        currentMeeting: data.current_meeting,
      });
    }
  });

  // Refresh model status and unembedded count on mount
  useEffect(() => {
    refreshModelStatus();
    refreshUnembeddedCount();
  }, [refreshModelStatus, refreshUnembeddedCount]);

  return {
    // Model status
    transcriptionDownloaded: store.transcriptionDownloaded,
    transcriptionReady: store.transcriptionReady,
    embeddingDownloaded: store.embeddingDownloaded,
    embeddingReady: store.embeddingReady,
    llmReady: store.llmReady,
    llmStatus: store.llmStatus,
    llmModels: store.llmModels,
    embeddingInfo: store.embeddingInfo,

    // Loading states
    isLoadingModels: store.isLoadingModels,
    isDownloading: store.isDownloading,
    isLoadingTranscription: store.isLoadingTranscription,
    isLoadingEmbedding: store.isLoadingEmbedding,
    isLoadingLlm: store.isLoadingLlm,
    downloadProgress: store.downloadProgress,
    downloadingModel: store.downloadingModel,
    downloadStage: store.downloadStage,
    downloadMessage: store.downloadMessage,
    downloadedBytes: store.downloadedBytes,
    downloadTotalBytes: store.downloadTotalBytes,
    downloadSpeedBps: store.downloadSpeedBps,
    downloadFile: store.downloadFile,
    downloadSourceModelId: store.downloadSourceModelId,
    error: store.error,
    errorModel: store.errorModel,

    // Actions
    refreshModelStatus: store.refreshModelStatus,
    initializeTranscription: store.initializeTranscription,
    initializeEmbedding: store.initializeEmbedding,
    initializeLlm: store.initializeLlm,
    downloadTranscriptionModel: store.downloadTranscriptionModel,
    downloadEmbeddingModel: store.downloadEmbeddingModel,
    downloadLlmModel: store.downloadLlmModel,
    deleteTranscriptionModel: store.deleteTranscriptionModel,
    deleteEmbeddingModel: store.deleteEmbeddingModel,
    deleteLlmModel: store.deleteLlmModel,
    clearError: store.clearError,

    // Batch embedding
    unembeddedCount: store.unembeddedCount,
    isBatchEmbedding: store.isBatchEmbedding,
    batchEmbedProgress: store.batchEmbedProgress,
    batchEmbedMeetings: store.batchEmbedMeetings,
    refreshUnembeddedCount: store.refreshUnembeddedCount,
  };
}
