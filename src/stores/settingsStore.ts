/**
 * Settings state store
 * Manages app preferences, theme, and model settings
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type {
  TranscriptionBackend,
  LlmModel,
  LlmModelInfo,
  LlmStatus,
  EmbeddingInfo,
  MacSystemAudioBackend,
  EchoCancellationBackend,
} from '../types';
import * as api from '../lib/tauri';

interface DownloadProgressUpdate {
  progress: number;
  modelKey?: string;
  sourceModelId?: string | null;
  stage?: string | null;
  message?: string | null;
  downloadedBytes?: number | null;
  totalBytes?: number | null;
  speedBps?: number | null;
  file?: string | null;
}

interface SettingsStore {
  // Preferences
  theme: 'light' | 'dark' | 'system';
  transcriptionBackend: TranscriptionBackend;
  llmModel: LlmModel;
  autoProcessMeetings: boolean;
  autoEmbedTranscripts: boolean;
  liveTranscriptionEnabled: boolean;
  liveTranscriptionIntervalSec: number;
  echoCancellationBackend: EchoCancellationBackend;
  macSystemAudioBackend: MacSystemAudioBackend;
  macSystemAudioDevice: string;

  // Model status (not persisted)
  llmStatus: LlmStatus | null;
  llmModels: LlmModelInfo[];
  embeddingInfo: EmbeddingInfo | null;
  transcriptionDownloaded: boolean;
  transcriptionReady: boolean;
  embeddingDownloaded: boolean;
  embeddingReady: boolean;
  llmReady: boolean;

  // Loading states
  isLoadingModels: boolean;
  isDownloading: boolean;
  isLoadingTranscription: boolean;
  isLoadingEmbedding: boolean;
  isLoadingLlm: boolean;
  downloadProgress: number;
  downloadingModel: string | null;
  downloadStage: string | null;
  downloadMessage: string | null;
  downloadedBytes: number | null;
  downloadTotalBytes: number | null;
  downloadSpeedBps: number | null;
  downloadFile: string | null;
  downloadSourceModelId: string | null;
  error: string | null;
  errorModel: string | null; // Which model had the error

  // Batch embedding state
  unembeddedCount: number;
  isBatchEmbedding: boolean;
  batchEmbedProgress: { current: number; total: number; currentMeeting: string } | null;

  // Actions - Preferences
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setTranscriptionBackend: (backend: TranscriptionBackend) => void;
  setLlmModel: (model: LlmModel) => void;
  setAutoProcessMeetings: (enabled: boolean) => void;
  setAutoEmbedTranscripts: (enabled: boolean) => void;
  setLiveTranscriptionEnabled: (enabled: boolean) => void;
  setLiveTranscriptionIntervalSec: (seconds: number) => void;
  setEchoCancellationBackend: (backend: EchoCancellationBackend) => void;
  setMacSystemAudioBackend: (backend: MacSystemAudioBackend) => void;
  setMacSystemAudioDevice: (device: string) => void;

  // Actions - Model management
  refreshModelStatus: () => Promise<void>;
  initializeTranscription: () => Promise<boolean>;
  initializeEmbedding: () => Promise<boolean>;
  initializeLlm: (model?: LlmModel) => Promise<boolean>;
  downloadTranscriptionModel: (backend: TranscriptionBackend) => Promise<void>;
  downloadEmbeddingModel: () => Promise<void>;
  downloadLlmModel: (model: LlmModel) => Promise<void>;
  deleteTranscriptionModel: (backend: TranscriptionBackend) => Promise<void>;
  deleteEmbeddingModel: () => Promise<void>;
  deleteLlmModel: (model: LlmModel) => Promise<void>;
  setDownloadProgress: (progress: DownloadProgressUpdate) => void;
  clearError: () => void;

  // Actions - Batch embedding
  refreshUnembeddedCount: () => Promise<void>;
  batchEmbedMeetings: () => Promise<void>;
  setBatchEmbedProgress: (progress: { current: number; total: number; currentMeeting: string } | null) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set, get) => ({
      // Default preferences
      theme: 'system',
      transcriptionBackend: 'Parakeet',
      llmModel: 'Qwen3_4B',
      autoProcessMeetings: true,
      autoEmbedTranscripts: true,
      liveTranscriptionEnabled: false,
      liveTranscriptionIntervalSec: 6,
      echoCancellationBackend: 'webrtc_aec3',
      macSystemAudioBackend: 'auto',
      macSystemAudioDevice: '',

      // Model status
      llmStatus: null,
      llmModels: [],
      embeddingInfo: null,
      transcriptionDownloaded: false,
      transcriptionReady: false,
      embeddingDownloaded: false,
      embeddingReady: false,
      llmReady: false,

      // Loading states
      isLoadingModels: false,
      isDownloading: false,
      isLoadingTranscription: false,
      isLoadingEmbedding: false,
      isLoadingLlm: false,
      downloadProgress: 0,
      downloadingModel: null,
      downloadStage: null,
      downloadMessage: null,
      downloadedBytes: null,
      downloadTotalBytes: null,
      downloadSpeedBps: null,
      downloadFile: null,
      downloadSourceModelId: null,
      error: null,
      errorModel: null,

      // Batch embedding state
      unembeddedCount: 0,
      isBatchEmbedding: false,
      batchEmbedProgress: null,

      // Preference setters
      setTheme: (theme) => set({ theme }),
      setTranscriptionBackend: (backend) =>
        set({ transcriptionBackend: backend }),
      setLlmModel: (model) => set({ llmModel: model }),
      setAutoProcessMeetings: (enabled) =>
        set({ autoProcessMeetings: enabled }),
      setAutoEmbedTranscripts: (enabled) =>
        set({ autoEmbedTranscripts: enabled }),
      setLiveTranscriptionEnabled: (enabled) =>
        set({ liveTranscriptionEnabled: enabled }),
      setLiveTranscriptionIntervalSec: (seconds) =>
        set({ liveTranscriptionIntervalSec: Math.min(Math.max(Math.round(seconds), 2), 15) }),
      setEchoCancellationBackend: (backend) =>
        set({ echoCancellationBackend: backend }),
      setMacSystemAudioBackend: (backend) =>
        set({ macSystemAudioBackend: backend }),
      setMacSystemAudioDevice: (device) =>
        set({ macSystemAudioDevice: device }),

      // Model management
      refreshModelStatus: async () => {
        set({ isLoadingModels: true, error: null });
        try {
          const { transcriptionBackend } = get();
          const [
            transcriptionDownloaded,
            transcriptionReady,
            embeddingDownloaded,
            embeddingReady,
            llmStatus,
            llmModels,
            embeddingInfo,
          ] = await Promise.all([
            api.isModelDownloaded(transcriptionBackend),
            api.isTranscriptionReady(),
            api.isEmbeddingDownloaded(),
            api.isEmbeddingReady(),
            api.getLlmStatus(),
            api.listLlmModels(),
            api.getEmbeddingInfo(),
          ]);

          set({
            transcriptionDownloaded,
            transcriptionReady,
            embeddingDownloaded,
            embeddingReady,
            llmReady: llmStatus.loaded,
            llmStatus,
            llmModels,
            embeddingInfo,
            isLoadingModels: false,
          });
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            isLoadingModels: false,
          });
        }
      },

      initializeTranscription: async () => {
        const { transcriptionBackend } = get();
        set({ isLoadingTranscription: true, error: null });
        try {
          await api.initTranscription(transcriptionBackend);
          set({ transcriptionReady: true, isLoadingTranscription: false });
          return true;
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            isLoadingTranscription: false,
          });
          return false;
        }
      },

      initializeEmbedding: async () => {
        set({ isLoadingEmbedding: true, error: null });
        try {
          await api.initializeEmbedding();
          set({ embeddingReady: true, embeddingDownloaded: true, isLoadingEmbedding: false });
          return true;
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: 'embedding',
            isLoadingEmbedding: false,
          });
          return false;
        }
      },

      initializeLlm: async (model) => {
        const modelToLoad = model ?? get().llmModel;
        set({ isLoadingLlm: true, error: null });
        try {
          await api.initializeLlm(modelToLoad);
          const status = await api.getLlmStatus();
          set({
            llmReady: status.loaded,
            llmStatus: status,
            llmModel: modelToLoad,
            isLoadingLlm: false
          });
          return status.loaded;
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            isLoadingLlm: false
          });
          return false;
        }
      },

      downloadTranscriptionModel: async (backend) => {
        set({
          isDownloading: true,
          downloadProgress: 0,
          downloadingModel: backend,
          downloadStage: 'Preparing',
          downloadMessage: 'Preparing transcription model download...',
          downloadedBytes: null,
          downloadTotalBytes: null,
          downloadSpeedBps: null,
          downloadFile: null,
          downloadSourceModelId: null,
          error: null,
          errorModel: null,
        });
        try {
          await api.downloadTranscriptionModel(backend);
          set({
            isDownloading: false,
            downloadProgress: 100,
            downloadingModel: null,
            downloadStage: 'Complete',
            downloadMessage: 'Transcription model downloaded successfully.',
            downloadSpeedBps: null,
            downloadFile: null,
          });
          // Refresh model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          const message = e instanceof Error ? e.message : String(e);
          set({
            error: message,
            errorModel: backend,
            isDownloading: false,
            downloadingModel: null,
            downloadStage: 'Failed',
            downloadMessage: message,
            downloadSpeedBps: null,
          });
        }
      },

      downloadEmbeddingModel: async () => {
        set({
          isDownloading: true,
          downloadProgress: 0,
          downloadingModel: 'embedding',
          downloadStage: 'Preparing',
          downloadMessage: 'Preparing embedding model download...',
          downloadedBytes: null,
          downloadTotalBytes: null,
          downloadSpeedBps: null,
          downloadFile: null,
          downloadSourceModelId: null,
          error: null,
          errorModel: null,
        });
        try {
          // initializeEmbedding downloads the model if needed
          await api.initializeEmbedding();
          set({
            isDownloading: false,
            downloadProgress: 100,
            downloadingModel: null,
            downloadStage: 'Complete',
            downloadMessage: 'Embedding model downloaded successfully.',
            embeddingDownloaded: true,
            embeddingReady: true,
          });
          // Refresh model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          const message = e instanceof Error ? e.message : String(e);
          set({
            error: message,
            errorModel: 'embedding',
            isDownloading: false,
            downloadingModel: null,
            downloadStage: 'Failed',
            downloadMessage: message,
            downloadSpeedBps: null,
          });
        }
      },

      downloadLlmModel: async (model) => {
        set({
          isDownloading: true,
          downloadProgress: 0,
          downloadingModel: model,
          downloadStage: 'Preparing',
          downloadMessage: 'Preparing language model download...',
          downloadedBytes: null,
          downloadTotalBytes: null,
          downloadSpeedBps: null,
          downloadFile: null,
          downloadSourceModelId: null,
          error: null,
          errorModel: null,
        });
        try {
          await api.downloadLlm(model);
          set({
            isDownloading: false,
            downloadProgress: 100,
            downloadingModel: null,
            downloadStage: 'Complete',
            downloadMessage: 'Language model downloaded successfully.',
            downloadSpeedBps: null,
            downloadFile: null,
          });
          // Refresh all model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          const message = e instanceof Error ? e.message : String(e);
          set({
            error: message,
            errorModel: model,
            isDownloading: false,
            downloadingModel: null,
            downloadStage: 'Failed',
            downloadMessage: message,
            downloadSpeedBps: null,
          });
        }
      },

      deleteTranscriptionModel: async (backend) => {
        set({ error: null, errorModel: null });
        try {
          await api.deleteTranscriptionModel(backend);
          // Refresh model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: backend,
          });
        }
      },

      deleteEmbeddingModel: async () => {
        set({ error: null, errorModel: null });
        try {
          await api.deleteEmbedding();
          set({ embeddingDownloaded: false, embeddingReady: false });
          // Refresh model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: 'embedding',
          });
        }
      },

      deleteLlmModel: async (model) => {
        set({ error: null, errorModel: null });
        try {
          await api.deleteLlmModel(model);
          // Refresh model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: model,
          });
        }
      },

      setDownloadProgress: (update) => {
        set((state) => ({
          downloadProgress: Math.min(Math.max(update.progress, 0), 100),
          downloadingModel: update.modelKey ?? state.downloadingModel,
          downloadStage:
            update.stage !== undefined ? update.stage : state.downloadStage,
          downloadMessage:
            update.message !== undefined ? update.message : state.downloadMessage,
          downloadedBytes:
            update.downloadedBytes !== undefined
              ? update.downloadedBytes
              : state.downloadedBytes,
          downloadTotalBytes:
            update.totalBytes !== undefined
              ? update.totalBytes
              : state.downloadTotalBytes,
          downloadSpeedBps:
            update.speedBps !== undefined
              ? update.speedBps
              : state.downloadSpeedBps,
          downloadFile:
            update.file !== undefined ? update.file : state.downloadFile,
          downloadSourceModelId:
            update.sourceModelId !== undefined
              ? update.sourceModelId
              : state.downloadSourceModelId,
        }));
      },

      clearError: () => set({ error: null, errorModel: null }),

      // Batch embedding actions
      refreshUnembeddedCount: async () => {
        try {
          const unembedded = await api.getUnembeddedMeetings();
          set({ unembeddedCount: unembedded.length });
        } catch (e) {
          console.error('Failed to get unembedded meetings:', e);
        }
      },

      batchEmbedMeetings: async () => {
        set({ isBatchEmbedding: true, error: null });
        try {
          const result = await api.batchEmbedMeetings();
          set({
            isBatchEmbedding: false,
            batchEmbedProgress: null,
            unembeddedCount: 0,
          });
          if (result.failed.length > 0) {
            set({
              error: `${result.failed.length} meeting(s) failed to embed`,
              errorModel: 'embedding',
            });
          }
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: 'embedding',
            isBatchEmbedding: false,
            batchEmbedProgress: null,
          });
        }
      },

      setBatchEmbedProgress: (progress) => {
        set({ batchEmbedProgress: progress });
      },
    }),
    {
      name: 'meeting-scribe-settings',
      partialize: (state) => ({
        theme: state.theme,
        transcriptionBackend: state.transcriptionBackend,
        llmModel: state.llmModel,
        autoProcessMeetings: state.autoProcessMeetings,
        autoEmbedTranscripts: state.autoEmbedTranscripts,
        liveTranscriptionEnabled: state.liveTranscriptionEnabled,
        liveTranscriptionIntervalSec: state.liveTranscriptionIntervalSec,
        echoCancellationBackend: state.echoCancellationBackend,
        macSystemAudioBackend: state.macSystemAudioBackend,
        macSystemAudioDevice: state.macSystemAudioDevice,
      }),
    }
  )
);
