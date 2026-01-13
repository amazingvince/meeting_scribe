/**
 * Models hook
 * Provides model status and download progress
 */

import { useEffect, useRef } from 'react';
import { useSettingsStore, useToastStore } from '../stores';
import { useTauriEvent } from './useTauriEvent';
import type { LlmDownloadProgress, EmbeddingDownloadProgress } from '../types';
import type { DownloadProgressEvent } from '../lib/tauri';

export function useModels() {
  const store = useSettingsStore();
  const toast = useToastStore();
  const { setDownloadProgress, refreshModelStatus } = store;

  // Track if we've shown the completion toast to avoid duplicates
  const completedModelsRef = useRef<Set<string>>(new Set());

  // Subscribe to download progress events
  useTauriEvent<DownloadProgressEvent>('model-download-progress', (data) => {
    setDownloadProgress(data.percent, data.model_id);
    // Show completion toast when reaching 100%
    if (data.percent >= 100 && !completedModelsRef.current.has(data.model_id)) {
      completedModelsRef.current.add(data.model_id);
      toast.success(`${data.model_id} downloaded successfully`);
      // Clear after a delay to allow re-download
      setTimeout(() => completedModelsRef.current.delete(data.model_id), 5000);
    }
  });

  useTauriEvent<LlmDownloadProgress>('llm-download-progress', (data) => {
    setDownloadProgress(data.percent, data.model);
    // Show completion toast when reaching 100%
    if (data.percent >= 100 && !completedModelsRef.current.has(data.model)) {
      completedModelsRef.current.add(data.model);
      toast.success(`${data.model} downloaded successfully`);
      setTimeout(() => completedModelsRef.current.delete(data.model), 5000);
    }
  });

  useTauriEvent<EmbeddingDownloadProgress>(
    'embedding-download-progress',
    (data) => {
      setDownloadProgress(data.percent, data.model_id);
      if (data.percent >= 100 && !completedModelsRef.current.has(data.model_id)) {
        completedModelsRef.current.add(data.model_id);
        toast.success('Embedding model downloaded successfully');
        setTimeout(() => completedModelsRef.current.delete(data.model_id), 5000);
      }
    }
  );

  // Refresh model status on mount
  useEffect(() => {
    refreshModelStatus();
  }, [refreshModelStatus]);

  return {
    // Model status
    transcriptionDownloaded: store.transcriptionDownloaded,
    transcriptionReady: store.transcriptionReady,
    embeddingReady: store.embeddingReady,
    llmReady: store.llmReady,
    llmStatus: store.llmStatus,
    llmModels: store.llmModels,
    embeddingInfo: store.embeddingInfo,

    // Loading states
    isLoadingModels: store.isLoadingModels,
    isDownloading: store.isDownloading,
    isLoadingTranscription: store.isLoadingTranscription,
    isLoadingLlm: store.isLoadingLlm,
    downloadProgress: store.downloadProgress,
    downloadingModel: store.downloadingModel,
    error: store.error,
    errorModel: store.errorModel,

    // Actions
    refreshModelStatus: store.refreshModelStatus,
    initializeTranscription: store.initializeTranscription,
    initializeEmbedding: store.initializeEmbedding,
    initializeLlm: store.initializeLlm,
    downloadTranscriptionModel: store.downloadTranscriptionModel,
    downloadLlmModel: store.downloadLlmModel,
    deleteTranscriptionModel: store.deleteTranscriptionModel,
    deleteLlmModel: store.deleteLlmModel,
    clearError: store.clearError,
  };
}
