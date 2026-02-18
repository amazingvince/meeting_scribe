import type { LlmModel } from '../types';
import * as api from '../lib/tauri';
import { useSettingsStore } from '../stores/settingsStore';

type ModelKind = 'transcription' | 'embedding' | 'llm';

const MEMORY_ERROR_PATTERNS = [
  'out of memory',
  'not enough memory',
  'insufficient memory',
  'failed to allocate',
  'allocation failed',
  'resource exhausted',
  'cuda out of memory',
  'metal out of memory',
];

class ModelManager {
  private lifecycleQueue: Promise<void> = Promise.resolve();

  private runExclusive<T>(task: () => Promise<T>): Promise<T> {
    const previous = this.lifecycleQueue.catch(() => undefined);
    let release: (() => void) | null = null;
    this.lifecycleQueue = new Promise<void>((resolve) => {
      release = resolve;
    });

    return previous
      .then(task)
      .finally(() => {
        release?.();
      });
  }

  private async safeRefreshModelStatus(): Promise<void> {
    try {
      await useSettingsStore.getState().refreshModelStatus();
    } catch (error) {
      console.warn('Failed to refresh model status:', error);
    }
  }

  private isMemoryPressureError(error: string | null | undefined): boolean {
    if (!error) {
      return false;
    }

    const normalized = error.toLowerCase();
    return MEMORY_ERROR_PATTERNS.some((pattern) => normalized.includes(pattern));
  }

  private async safeUnloadTranscription(): Promise<void> {
    try {
      await api.unloadTranscription();
    } catch (error) {
      console.warn('Failed to unload transcription model:', error);
    }
    useSettingsStore.setState({ transcriptionReady: false });
  }

  private async safeUnloadEmbedding(): Promise<void> {
    try {
      await api.unloadEmbedding();
    } catch (error) {
      console.warn('Failed to unload embedding model:', error);
    }
    useSettingsStore.setState({ embeddingReady: false });
  }

  private async safeUnloadLlm(): Promise<void> {
    try {
      await api.unloadLlmModel();
    } catch (error) {
      console.warn('Failed to unload language model:', error);
    }
    useSettingsStore.setState({ llmReady: false });
  }

  private async freeMemoryFor(target: ModelKind): Promise<void> {
    // Unload non-target models unconditionally: readiness flags can be stale.
    if (target !== 'transcription') {
      await this.safeUnloadTranscription();
    }

    if (target !== 'embedding') {
      await this.safeUnloadEmbedding();
    }

    if (target !== 'llm') {
      await this.safeUnloadLlm();
    }

    await this.safeRefreshModelStatus();
  }

  async ensureWarmup(): Promise<void> {
    await this.runExclusive(async () => {
      try {
        await useSettingsStore.getState().ensureModelsLoadedInBackground();
      } catch (error) {
        console.warn('Background model warmup failed:', error);
      }
    });
  }

  async ensureTranscriptionReady(): Promise<boolean> {
    return this.runExclusive(async () => {
      const initial = useSettingsStore.getState();
      if (initial.transcriptionReady) {
        return true;
      }

      await this.safeRefreshModelStatus();

      let refreshed = useSettingsStore.getState();
      if (refreshed.transcriptionReady) {
        return true;
      }

      if (!refreshed.transcriptionDownloaded) {
        try {
          const downloaded = await api.isModelDownloaded(
            refreshed.transcriptionBackend
          );
          if (!downloaded) {
            return false;
          }
          useSettingsStore.setState({ transcriptionDownloaded: true });
          refreshed = useSettingsStore.getState();
        } catch (error) {
          console.warn('Failed to verify transcription model download state:', error);
          return false;
        }
      }

      const loaded = await refreshed.initializeTranscription({ background: true });
      if (loaded) {
        return true;
      }

      const failedState = useSettingsStore.getState();
      if (!this.isMemoryPressureError(failedState.error)) {
        return false;
      }

      console.warn(
        'Transcription load hit memory pressure; unloading other models and retrying.'
      );
      await this.freeMemoryFor('transcription');
      return useSettingsStore
        .getState()
        .initializeTranscription({ background: true });
    });
  }

  async ensureEmbeddingReady(): Promise<boolean> {
    return this.runExclusive(async () => {
      const initial = useSettingsStore.getState();
      if (initial.embeddingReady) {
        return true;
      }

      await this.safeRefreshModelStatus();

      let refreshed = useSettingsStore.getState();
      if (refreshed.embeddingReady) {
        return true;
      }

      if (!refreshed.embeddingDownloaded) {
        try {
          const downloaded = await api.isEmbeddingDownloaded();
          if (!downloaded) {
            return false;
          }
          useSettingsStore.setState({ embeddingDownloaded: true });
          refreshed = useSettingsStore.getState();
        } catch (error) {
          console.warn('Failed to verify embedding model download state:', error);
          return false;
        }
      }

      const loaded = await refreshed.initializeEmbedding({ background: true });
      if (loaded) {
        return true;
      }

      const failedState = useSettingsStore.getState();
      if (!this.isMemoryPressureError(failedState.error)) {
        return false;
      }

      console.warn(
        'Embedding load hit memory pressure; unloading other models and retrying.'
      );
      await this.freeMemoryFor('embedding');
      return useSettingsStore.getState().initializeEmbedding({ background: true });
    });
  }

  private async tryInitializeLlm(preferredModel?: LlmModel): Promise<boolean> {
    const state = useSettingsStore.getState();
    const preferred = preferredModel ?? state.llmModel;

    const preferredDownloaded = state.llmModels.some(
      (model) => model.model === preferred && model.downloaded
    );
    if (preferredDownloaded) {
      return state.initializeLlm(preferred, { background: true });
    }

    const fallback = state.llmModels.find((model) => model.downloaded)?.model;
    if (!fallback) {
      return false;
    }

    if (fallback !== state.llmModel) {
      state.setLlmModel(fallback);
    }

    return useSettingsStore
      .getState()
      .initializeLlm(fallback, { background: true });
  }

  async ensureLlmReady(preferredModel?: LlmModel): Promise<boolean> {
    return this.runExclusive(async () => {
      const initial = useSettingsStore.getState();
      if (initial.llmReady) {
        return true;
      }

      await this.safeRefreshModelStatus();

      let refreshed = useSettingsStore.getState();
      if (refreshed.llmReady) {
        return true;
      }

      let loaded = await this.tryInitializeLlm(preferredModel);
      if (loaded) {
        return true;
      }

      refreshed = useSettingsStore.getState();
      if (!this.isMemoryPressureError(refreshed.error)) {
        return false;
      }

      console.warn('LLM load hit memory pressure; unloading other models and retrying.');
      await this.freeMemoryFor('llm');
      loaded = await this.tryInitializeLlm(preferredModel);
      return loaded;
    });
  }
}

export const modelManager = new ModelManager();
