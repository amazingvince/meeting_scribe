/**
 * Model settings section
 */

import { Cpu, MessageSquare, Search } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { SkeletonCard } from '../ui/Skeleton';
import { ModelDownloadCard } from './ModelDownloadCard';
import { useModels } from '../../hooks';
import { useSettingsStore } from '../../stores';

export function ModelSettings() {
  const {
    llmModels,
    llmStatus,
    embeddingInfo,
    transcriptionDownloaded,
    transcriptionReady,
    llmReady,
    isLoadingModels,
    isDownloading,
    isLoadingTranscription,
    isLoadingLlm,
    downloadProgress,
    downloadingModel,
    downloadTranscriptionModel,
    downloadLlmModel,
    deleteTranscriptionModel,
    deleteLlmModel,
    initializeLlm,
    initializeTranscription,
    error,
    errorModel,
    clearError,
  } = useModels();

  const {
    transcriptionBackend,
    llmModel,
    setTranscriptionBackend,
    setLlmModel,
  } = useSettingsStore();
  // Note: refreshModelStatus is called on mount by useModels hook

  if (isLoadingModels) {
    return (
      <div className="space-y-4">
        <SkeletonCard />
        <SkeletonCard />
        <SkeletonCard />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Transcription Models */}
      <Card>
        <div className="flex items-center gap-2 mb-4">
          <Cpu className="w-5 h-5 text-indigo-500" />
          <CardTitle>Transcription Model</CardTitle>
        </div>

        <ModelDownloadCard
          name="Parakeet TDT 0.6B (Int8)"
          description="NVIDIA's Parakeet model for fast, accurate transcription."
          size="~650 MB"
          downloaded={transcriptionDownloaded}
          isDownloading={isDownloading && downloadingModel === 'Parakeet'}
          downloadProgress={downloadingModel === 'Parakeet' ? downloadProgress : 0}
          onDownload={() => downloadTranscriptionModel('Parakeet')}
          onDelete={() => deleteTranscriptionModel('Parakeet')}
          error={errorModel === 'Parakeet' ? error : null}
          onClearError={clearError}
          isLoaded={transcriptionReady}
          isLoadingModel={isLoadingTranscription}
          onLoad={initializeTranscription}
          isDefault={transcriptionBackend === 'Parakeet'}
          onSetDefault={() => setTranscriptionBackend('Parakeet')}
        />
      </Card>

      {/* LLM Models */}
      <Card>
        <div className="flex items-center gap-2 mb-4">
          <MessageSquare className="w-5 h-5 text-green-500" />
          <CardTitle>Language Models</CardTitle>
        </div>

        <div className="space-y-3">
          {llmModels.map((model) => {
            // Check if this specific model is currently loaded
            const isThisModelLoaded = llmReady && llmStatus?.current_model === model.name;
            return (
              <ModelDownloadCard
                key={model.model}
                name={model.name}
                description={`${model.context_length.toLocaleString()} token context`}
                size={model.size_formatted}
                downloaded={model.downloaded}
                isDownloading={isDownloading && downloadingModel === model.model}
                downloadProgress={downloadingModel === model.model ? downloadProgress : 0}
                onDownload={() => downloadLlmModel(model.model)}
                onDelete={() => deleteLlmModel(model.model)}
                error={errorModel === model.model ? error : null}
                onClearError={clearError}
                isLoaded={isThisModelLoaded}
                isLoadingModel={isLoadingLlm}
                onLoad={() => initializeLlm(model.model)}
                isDefault={llmModel === model.model}
                onSetDefault={() => setLlmModel(model.model)}
              />
            );
          })}
        </div>

        {llmStatus?.loaded && (
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-4">
            Currently loaded: {llmStatus.current_model}
          </p>
        )}
      </Card>

      {/* Embedding Model */}
      <Card>
        <div className="flex items-center gap-2 mb-4">
          <Search className="w-5 h-5 text-blue-500" />
          <CardTitle>Embedding Model</CardTitle>
        </div>

        {embeddingInfo && (
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">Model</span>
              <span className="text-gray-900 dark:text-gray-100">
                {embeddingInfo.model_name}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">Size</span>
              <span className="text-gray-900 dark:text-gray-100">
                {embeddingInfo.model_size}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">Status</span>
              <span
                className={
                  embeddingInfo.loaded
                    ? 'text-green-600'
                    : 'text-gray-500 dark:text-gray-400'
                }
              >
                {embeddingInfo.loaded ? 'Loaded' : 'Not loaded'}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">Dimension</span>
              <span className="text-gray-900 dark:text-gray-100">
                {embeddingInfo.dimension}
              </span>
            </div>
          </div>
        )}
      </Card>
    </div>
  );
}
