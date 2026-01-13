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
} from '../types';
import * as api from '../lib/tauri';

interface SettingsStore {
  // Preferences
  theme: 'light' | 'dark' | 'system';
  transcriptionBackend: TranscriptionBackend;
  llmModel: LlmModel;
  autoProcessMeetings: boolean;
  autoEmbedTranscripts: boolean;

  // Model status (not persisted)
  llmStatus: LlmStatus | null;
  llmModels: LlmModelInfo[];
  embeddingInfo: EmbeddingInfo | null;
  transcriptionDownloaded: boolean;
  transcriptionReady: boolean;
  embeddingReady: boolean;
  llmReady: boolean;

  // Loading states
  isLoadingModels: boolean;
  isDownloading: boolean;
  isLoadingTranscription: boolean;
  isLoadingLlm: boolean;
  downloadProgress: number;
  downloadingModel: string | null;
  error: string | null;
  errorModel: string | null; // Which model had the error

  // Actions - Preferences
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setTranscriptionBackend: (backend: TranscriptionBackend) => void;
  setLlmModel: (model: LlmModel) => void;
  setAutoProcessMeetings: (enabled: boolean) => void;
  setAutoEmbedTranscripts: (enabled: boolean) => void;

  // Actions - Model management
  refreshModelStatus: () => Promise<void>;
  initializeTranscription: () => Promise<boolean>;
  initializeEmbedding: () => Promise<boolean>;
  initializeLlm: (model?: LlmModel) => Promise<boolean>;
  downloadTranscriptionModel: (backend: TranscriptionBackend) => Promise<void>;
  downloadLlmModel: (model: LlmModel) => Promise<void>;
  deleteTranscriptionModel: (backend: TranscriptionBackend) => Promise<void>;
  deleteLlmModel: (model: LlmModel) => Promise<void>;
  setDownloadProgress: (progress: number, modelId?: string) => void;
  clearError: () => void;
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

      // Model status
      llmStatus: null,
      llmModels: [],
      embeddingInfo: null,
      transcriptionDownloaded: false,
      transcriptionReady: false,
      embeddingReady: false,
      llmReady: false,

      // Loading states
      isLoadingModels: false,
      isDownloading: false,
      isLoadingTranscription: false,
      isLoadingLlm: false,
      downloadProgress: 0,
      downloadingModel: null,
      error: null,
      errorModel: null,

      // Preference setters
      setTheme: (theme) => set({ theme }),
      setTranscriptionBackend: (backend) =>
        set({ transcriptionBackend: backend }),
      setLlmModel: (model) => set({ llmModel: model }),
      setAutoProcessMeetings: (enabled) =>
        set({ autoProcessMeetings: enabled }),
      setAutoEmbedTranscripts: (enabled) =>
        set({ autoEmbedTranscripts: enabled }),

      // Model management
      refreshModelStatus: async () => {
        set({ isLoadingModels: true, error: null });
        try {
          const { transcriptionBackend } = get();
          const [
            transcriptionDownloaded,
            transcriptionReady,
            embeddingReady,
            llmStatus,
            llmModels,
            embeddingInfo,
          ] = await Promise.all([
            api.isModelDownloaded(transcriptionBackend),
            api.isTranscriptionReady(),
            api.isEmbeddingReady(),
            api.getLlmStatus(),
            api.listLlmModels(),
            api.getEmbeddingInfo(),
          ]);

          set({
            transcriptionDownloaded,
            transcriptionReady,
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
        try {
          await api.initializeEmbedding();
          set({ embeddingReady: true });
          return true;
        } catch (e) {
          set({ error: e instanceof Error ? e.message : String(e) });
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
          error: null,
          errorModel: null,
        });
        try {
          await api.downloadTranscriptionModel(backend);
          set({
            isDownloading: false,
            downloadProgress: 100,
            downloadingModel: null,
          });
          // Refresh model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: backend,
            isDownloading: false,
            downloadingModel: null,
          });
        }
      },

      downloadLlmModel: async (model) => {
        set({
          isDownloading: true,
          downloadProgress: 0,
          downloadingModel: model,
          error: null,
          errorModel: null,
        });
        try {
          await api.downloadLlm(model);
          set({
            isDownloading: false,
            downloadProgress: 100,
            downloadingModel: null,
          });
          // Refresh all model status to update UI
          await get().refreshModelStatus();
        } catch (e) {
          set({
            error: e instanceof Error ? e.message : String(e),
            errorModel: model,
            isDownloading: false,
            downloadingModel: null,
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

      setDownloadProgress: (progress, modelId) => {
        set({
          downloadProgress: progress,
          downloadingModel: modelId ?? get().downloadingModel,
        });
      },

      clearError: () => set({ error: null, errorModel: null }),
    }),
    {
      name: 'meeting-scribe-settings',
      partialize: (state) => ({
        theme: state.theme,
        transcriptionBackend: state.transcriptionBackend,
        llmModel: state.llmModel,
        autoProcessMeetings: state.autoProcessMeetings,
        autoEmbedTranscripts: state.autoEmbedTranscripts,
      }),
    }
  )
);
