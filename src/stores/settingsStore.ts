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

interface InitializeOptions {
  background?: boolean;
}

const DEFAULT_TRANSCRIPTION_BACKEND: TranscriptionBackend = 'Parakeet';
const DEFAULT_LLM_MODEL: LlmModel = 'Qwen3_4B';
const SUPPORTED_TRANSCRIPTION_BACKENDS = new Set<TranscriptionBackend>(['Parakeet']);
const KNOWN_LLM_MODELS = new Set<LlmModel>([
  'Qwen3_4B',
  'Qwen3_1_7B',
  'Qwen3_8B',
]);

function normalizeTranscriptionBackend(value: unknown): TranscriptionBackend {
  if (
    typeof value === 'string' &&
    SUPPORTED_TRANSCRIPTION_BACKENDS.has(value as TranscriptionBackend)
  ) {
    return value as TranscriptionBackend;
  }
  return DEFAULT_TRANSCRIPTION_BACKEND;
}

function normalizeLlmModel(value: unknown): LlmModel {
  if (typeof value === 'string' && KNOWN_LLM_MODELS.has(value as LlmModel)) {
    return value as LlmModel;
  }
  return DEFAULT_LLM_MODEL;
}

function isConfiguredLlmLoaded(status: LlmStatus | null, model: LlmModel): boolean {
  return Boolean(status?.loaded && status.current_model === model);
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const TRANSCRIPTION_INIT_TIMEOUT_MS = 120_000;
const EMBEDDING_INIT_TIMEOUT_MS = 120_000;
const MODEL_STATUS_TIMEOUT_MS = 30_000;

async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  timeoutMessage: string
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  try {
    return await Promise.race([
      promise,
      new Promise<T>((_, reject) => {
        timer = setTimeout(() => reject(new Error(timeoutMessage)), timeoutMs);
      }),
    ]);
  } finally {
    if (timer !== null) {
      clearTimeout(timer);
    }
  }
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
  initializeTranscription: (options?: InitializeOptions) => Promise<boolean>;
  initializeEmbedding: (options?: InitializeOptions) => Promise<boolean>;
  initializeLlm: (model?: LlmModel, options?: InitializeOptions) => Promise<boolean>;
  ensureModelsLoadedInBackground: () => Promise<void>;
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
  setBatchEmbedProgress: (
    progress: { current: number; total: number; currentMeeting: string } | null
  ) => void;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set, get) => {
      let transcriptionInitPromise: Promise<boolean> | null = null;
      let embeddingInitPromise: Promise<boolean> | null = null;
      let llmInitPromise: Promise<boolean> | null = null;
      let llmInitTarget: LlmModel | null = null;
      let warmupPromise: Promise<void> | null = null;

      return {
        // Default preferences
        theme: 'system',
        transcriptionBackend: DEFAULT_TRANSCRIPTION_BACKEND,
        llmModel: DEFAULT_LLM_MODEL,
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
        setTranscriptionBackend: (backend) => {
          const normalized = normalizeTranscriptionBackend(backend);
          const previous = get().transcriptionBackend;
          set({
            transcriptionBackend: normalized,
            transcriptionReady:
              previous === normalized ? get().transcriptionReady : false,
            error: null,
            errorModel: null,
          });

          if (normalized !== previous) {
            void get().refreshModelStatus().then(() => {
              const state = get();
              if (state.transcriptionDownloaded) {
                void state.initializeTranscription({ background: true });
              }
            });
          }
        },
        setLlmModel: (model) => {
          const normalized = normalizeLlmModel(model);
          const previous = get().llmModel;
          set({
            llmModel: normalized,
            llmReady: isConfiguredLlmLoaded(get().llmStatus, normalized),
            error: null,
            errorModel: null,
          });

          if (normalized !== previous) {
            const isDownloaded = get().llmModels.some(
              (item) => item.model === normalized && item.downloaded
            );
            if (isDownloaded) {
              void get().initializeLlm(normalized, { background: true });
            }
          }
        },
        setAutoProcessMeetings: (enabled) =>
          set({ autoProcessMeetings: enabled }),
        setAutoEmbedTranscripts: (enabled) =>
          set({ autoEmbedTranscripts: enabled }),
        setLiveTranscriptionEnabled: (enabled) =>
          set({ liveTranscriptionEnabled: enabled }),
        setLiveTranscriptionIntervalSec: (seconds) =>
          set({
            liveTranscriptionIntervalSec: Math.min(
              Math.max(Math.round(seconds), 2),
              15
            ),
          }),
        setEchoCancellationBackend: (backend) =>
          set({ echoCancellationBackend: backend }),
        setMacSystemAudioBackend: (backend) =>
          set({ macSystemAudioBackend: backend }),
        setMacSystemAudioDevice: (device) =>
          set({ macSystemAudioDevice: device }),

        // Model management
        refreshModelStatus: async () => {
          const current = get();
          const transcriptionBackend = normalizeTranscriptionBackend(
            current.transcriptionBackend
          );
          const llmModel = normalizeLlmModel(current.llmModel);
          if (
            transcriptionBackend !== current.transcriptionBackend ||
            llmModel !== current.llmModel
          ) {
            set({ transcriptionBackend, llmModel });
          }

          set({ isLoadingModels: true, error: null });
          try {
            const [
              transcriptionDownloadedResult,
              transcriptionEngineReadyResult,
              transcriptionConfigResult,
              embeddingDownloadedResult,
              embeddingReadyResult,
              llmStatusResult,
              llmModelsResult,
              embeddingInfoResult,
            ] = await Promise.allSettled([
              withTimeout(
                api.isModelDownloaded(transcriptionBackend),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while checking transcription download status.'
              ),
              withTimeout(
                api.isTranscriptionReady(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while checking transcription readiness.'
              ),
              withTimeout(
                api.getTranscriptionConfig(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while checking transcription configuration.'
              ),
              withTimeout(
                api.isEmbeddingDownloaded(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while checking embedding download status.'
              ),
              withTimeout(
                api.isEmbeddingReady(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while checking embedding readiness.'
              ),
              withTimeout(
                api.getLlmStatus(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while checking language model status.'
              ),
              withTimeout(
                api.listLlmModels(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while listing language models.'
              ),
              withTimeout(
                api.getEmbeddingInfo(),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while reading embedding model info.'
              ),
            ]);

            const previous = get();

            const transcriptionDownloaded =
              transcriptionDownloadedResult.status === 'fulfilled'
                ? transcriptionDownloadedResult.value
                : previous.transcriptionDownloaded;
            const embeddingDownloaded =
              embeddingDownloadedResult.status === 'fulfilled'
                ? embeddingDownloadedResult.value
                : previous.embeddingDownloaded;
            const embeddingReady =
              embeddingReadyResult.status === 'fulfilled'
                ? embeddingReadyResult.value
                : previous.embeddingReady;
            const llmStatus =
              llmStatusResult.status === 'fulfilled'
                ? llmStatusResult.value
                : previous.llmStatus;
            const llmModels =
              llmModelsResult.status === 'fulfilled'
                ? llmModelsResult.value
                : previous.llmModels;
            const embeddingInfo =
              embeddingInfoResult.status === 'fulfilled'
                ? embeddingInfoResult.value
                : previous.embeddingInfo;

            let transcriptionReady = previous.transcriptionReady;
            if (
              transcriptionEngineReadyResult.status === 'fulfilled' &&
              transcriptionConfigResult.status === 'fulfilled'
            ) {
              transcriptionReady =
                transcriptionEngineReadyResult.value &&
                transcriptionConfigResult.value.backend === transcriptionBackend;
            } else if (
              transcriptionEngineReadyResult.status === 'fulfilled' &&
              !transcriptionEngineReadyResult.value
            ) {
              transcriptionReady = false;
            }

            const llmReady =
              llmStatusResult.status === 'fulfilled'
                ? isConfiguredLlmLoaded(llmStatusResult.value, llmModel)
                : previous.llmReady;

            if (transcriptionDownloadedResult.status === 'rejected') {
              console.warn(
                'Failed to refresh transcription download status:',
                transcriptionDownloadedResult.reason
              );
            }
            if (transcriptionEngineReadyResult.status === 'rejected') {
              console.warn(
                'Failed to refresh transcription readiness:',
                transcriptionEngineReadyResult.reason
              );
            }
            if (transcriptionConfigResult.status === 'rejected') {
              console.warn(
                'Failed to refresh transcription config:',
                transcriptionConfigResult.reason
              );
            }
            if (embeddingDownloadedResult.status === 'rejected') {
              console.warn(
                'Failed to refresh embedding download status:',
                embeddingDownloadedResult.reason
              );
            }
            if (embeddingReadyResult.status === 'rejected') {
              console.warn(
                'Failed to refresh embedding readiness:',
                embeddingReadyResult.reason
              );
            }
            if (llmStatusResult.status === 'rejected') {
              console.warn('Failed to refresh LLM status:', llmStatusResult.reason);
            }
            if (llmModelsResult.status === 'rejected') {
              console.warn(
                'Failed to refresh LLM model list:',
                llmModelsResult.reason
              );
            }
            if (embeddingInfoResult.status === 'rejected') {
              console.warn(
                'Failed to refresh embedding info:',
                embeddingInfoResult.reason
              );
            }

            set({
              transcriptionBackend,
              llmModel,
              transcriptionDownloaded,
              transcriptionReady,
              embeddingDownloaded,
              embeddingReady,
              llmReady,
              llmStatus,
              llmModels,
              embeddingInfo,
              isLoadingModels: false,
            });
          } catch (error) {
            set({
              error: getErrorMessage(error),
              isLoadingModels: false,
            });
          }
        },

        initializeTranscription: async (_options) => {
          if (transcriptionInitPromise) {
            return transcriptionInitPromise;
          }

          const requestedBackend = normalizeTranscriptionBackend(
            get().transcriptionBackend
          );

          transcriptionInitPromise = (async () => {
            set({
              isLoadingTranscription: true,
              error: null,
              errorModel: null,
              transcriptionBackend: requestedBackend,
            });
            try {
              await withTimeout(
                api.initTranscription(requestedBackend),
                TRANSCRIPTION_INIT_TIMEOUT_MS,
                'Timed out while loading transcription model.'
              );
              const [downloaded, engineReady, config] = await withTimeout(
                Promise.all([
                  api.isModelDownloaded(requestedBackend),
                  api.isTranscriptionReady(),
                  api.getTranscriptionConfig(),
                ]),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while verifying transcription model status.'
              );
              const ready = downloaded && engineReady && config.backend === requestedBackend;

              const mismatchError =
                config.backend !== requestedBackend
                  ? `Loaded backend (${config.backend}) does not match requested backend (${requestedBackend}).`
                  : 'Transcription engine did not report ready after initialization.';

              set({
                transcriptionReady: ready,
                transcriptionDownloaded: downloaded,
                isLoadingTranscription: false,
                error: ready ? null : mismatchError,
                errorModel: ready ? null : requestedBackend,
              });
              return ready;
            } catch (error) {
              let downloaded = get().transcriptionDownloaded;
              try {
                downloaded = await api.isModelDownloaded(requestedBackend);
              } catch (statusError) {
                console.warn(
                  'Failed to verify transcription download state after init error:',
                  statusError
                );
              }

              set({
                error: getErrorMessage(error),
                errorModel: requestedBackend,
                isLoadingTranscription: false,
                transcriptionReady: false,
                transcriptionDownloaded: downloaded,
              });
              return false;
            }
          })();

          try {
            return await transcriptionInitPromise;
          } finally {
            transcriptionInitPromise = null;
            void get().refreshModelStatus();
          }
        },

        initializeEmbedding: async (_options) => {
          if (embeddingInitPromise) {
            return embeddingInitPromise;
          }

          embeddingInitPromise = (async () => {
            set({ isLoadingEmbedding: true, error: null, errorModel: null });
            try {
              await withTimeout(
                api.initializeEmbedding(),
                EMBEDDING_INIT_TIMEOUT_MS,
                'Timed out while loading embedding model.'
              );
              const [embeddingInfo, embeddingDownloaded, embeddingReady] = await withTimeout(
                Promise.all([
                  api.getEmbeddingInfo(),
                  api.isEmbeddingDownloaded(),
                  api.isEmbeddingReady(),
                ]),
                MODEL_STATUS_TIMEOUT_MS,
                'Timed out while verifying embedding model status.'
              );
              const ready = embeddingDownloaded && embeddingReady;
              set({
                embeddingReady: ready,
                embeddingDownloaded,
                embeddingInfo,
                isLoadingEmbedding: false,
                error: ready
                  ? null
                  : 'Embedding engine did not report ready after initialization.',
                errorModel: ready ? null : 'embedding',
              });
              return ready;
            } catch (error) {
              set({
                error: getErrorMessage(error),
                errorModel: 'embedding',
                isLoadingEmbedding: false,
                embeddingReady: false,
              });
              return false;
            }
          })();

          try {
            return await embeddingInitPromise;
          } finally {
            embeddingInitPromise = null;
            void get().refreshModelStatus();
          }
        },

        initializeLlm: async (model, _options) => {
          const modelToLoad = normalizeLlmModel(model ?? get().llmModel);

          if (llmInitPromise) {
            if (llmInitTarget === modelToLoad) {
              return llmInitPromise;
            }
            await llmInitPromise;
          }

          llmInitTarget = modelToLoad;
          llmInitPromise = (async () => {
            set({
              isLoadingLlm: true,
              error: null,
              errorModel: null,
              llmModel: modelToLoad,
            });
            try {
              await api.initializeLlm(modelToLoad);
              const [status, llmModels] = await Promise.all([
                api.getLlmStatus(),
                api.listLlmModels(),
              ]);
              const loaded = isConfiguredLlmLoaded(status, modelToLoad);
              set({
                llmReady: loaded,
                llmStatus: status,
                llmModels,
                llmModel: modelToLoad,
                isLoadingLlm: false,
              });
              return loaded;
            } catch (error) {
              set({
                error: getErrorMessage(error),
                errorModel: modelToLoad,
                isLoadingLlm: false,
                llmReady: false,
              });
              return false;
            }
          })();

          try {
            return await llmInitPromise;
          } finally {
            llmInitPromise = null;
            llmInitTarget = null;
            void get().refreshModelStatus();
          }
        },

        ensureModelsLoadedInBackground: async () => {
          if (warmupPromise) {
            return warmupPromise;
          }

          warmupPromise = (async () => {
            await get().refreshModelStatus();
            const state = get();

            // Keep startup memory usage predictable across release targets by
            // only warming transcription when recording workflows need it.
            const shouldPrimeTranscription =
              state.transcriptionDownloaded &&
              !state.transcriptionReady &&
              (state.autoProcessMeetings || state.liveTranscriptionEnabled);

            if (shouldPrimeTranscription) {
              await state.initializeTranscription({ background: true });
            }

            await get().refreshModelStatus();
          })().finally(() => {
            warmupPromise = null;
          });

          return warmupPromise;
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
            await get().refreshModelStatus();
            const state = get();
            if (state.transcriptionBackend === backend) {
              void state.initializeTranscription({ background: true });
            }
          } catch (error) {
            const message = getErrorMessage(error);
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
            const alreadyDownloaded = await api.isEmbeddingDownloaded();
            if (alreadyDownloaded) {
              set({
                isDownloading: false,
                downloadingModel: null,
                downloadStage: null,
                downloadMessage: null,
              });

              const loaded = await get().initializeEmbedding({ background: true });
              if (!loaded) {
                throw new Error(
                  get().error ??
                    'Embedding model files are present but loading failed.'
                );
              }

              await get().refreshModelStatus();
              return;
            }

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
            await get().refreshModelStatus();
          } catch (error) {
            const message = getErrorMessage(error);
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
            await get().refreshModelStatus();
            const state = get();
            if (state.llmModel === model) {
              void state.initializeLlm(model, { background: true });
            }
          } catch (error) {
            const message = getErrorMessage(error);
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
            await get().refreshModelStatus();
          } catch (error) {
            set({
              error: getErrorMessage(error),
              errorModel: backend,
            });
          }
        },

        deleteEmbeddingModel: async () => {
          set({ error: null, errorModel: null });
          try {
            await api.deleteEmbedding();
            set({ embeddingDownloaded: false, embeddingReady: false });
            await get().refreshModelStatus();
          } catch (error) {
            set({
              error: getErrorMessage(error),
              errorModel: 'embedding',
            });
          }
        },

        deleteLlmModel: async (model) => {
          set({ error: null, errorModel: null });
          try {
            await api.deleteLlmModel(model);
            await get().refreshModelStatus();
          } catch (error) {
            set({
              error: getErrorMessage(error),
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
          } catch (error) {
            console.error('Failed to get unembedded meetings:', error);
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
          } catch (error) {
            set({
              error: getErrorMessage(error),
              errorModel: 'embedding',
              isBatchEmbedding: false,
              batchEmbedProgress: null,
            });
          }
        },

        setBatchEmbedProgress: (progress) => {
          set({ batchEmbedProgress: progress });
        },
      };
    },
    {
      name: 'meeting-scribe-settings',
      version: 2,
      migrate: (persistedState) => {
        const state =
          persistedState && typeof persistedState === 'object'
            ? (persistedState as Record<string, unknown>)
            : {};
        return {
          ...state,
          transcriptionBackend: normalizeTranscriptionBackend(
            state.transcriptionBackend
          ),
          llmModel: normalizeLlmModel(state.llmModel),
        };
      },
      partialize: (state) => ({
        theme: state.theme,
        transcriptionBackend: normalizeTranscriptionBackend(
          state.transcriptionBackend
        ),
        llmModel: normalizeLlmModel(state.llmModel),
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
