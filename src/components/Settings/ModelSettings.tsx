/**
 * Model settings section
 */

import { Cpu, MessageSquare, Search, Database, Loader2 } from 'lucide-react';
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
    embeddingDownloaded,
    embeddingReady,
    llmReady,
    isLoadingModels,
    isDownloading,
    isLoadingTranscription,
    isLoadingEmbedding,
    isLoadingLlm,
    downloadProgress,
    downloadingModel,
    downloadTranscriptionModel,
    downloadEmbeddingModel,
    downloadLlmModel,
    deleteTranscriptionModel,
    deleteEmbeddingModel,
    deleteLlmModel,
    initializeLlm,
    initializeTranscription,
    initializeEmbedding,
    error,
    errorModel,
    clearError,
    // Batch embedding
    unembeddedCount,
    isBatchEmbedding,
    batchEmbedProgress,
    batchEmbedMeetings,
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

        <ModelDownloadCard
          name={embeddingInfo?.model_name ?? 'EmbeddingGemma'}
          description="Used for semantic search and RAG chat. Required for chat functionality."
          size={embeddingInfo?.model_size ?? '~300 MB'}
          downloaded={embeddingDownloaded}
          isDownloading={isDownloading && downloadingModel === 'embedding'}
          downloadProgress={downloadingModel === 'embedding' ? downloadProgress : 0}
          onDownload={downloadEmbeddingModel}
          onDelete={deleteEmbeddingModel}
          error={errorModel === 'embedding' ? error : null}
          onClearError={clearError}
          isLoaded={embeddingReady}
          isLoadingModel={isLoadingEmbedding}
          onLoad={initializeEmbedding}
          isDefault={true}
          onSetDefault={() => {}} // Only one embedding model for now
        />

        {embeddingInfo && embeddingDownloaded && (
          <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
            <div className="space-y-1 text-xs text-gray-500 dark:text-gray-400">
              <div className="flex justify-between">
                <span>Dimension</span>
                <span>{embeddingInfo.dimension}</span>
              </div>
              <div className="flex justify-between">
                <span>Max Tokens</span>
                <span>{embeddingInfo.max_tokens}</span>
              </div>
            </div>
          </div>
        )}
      </Card>

      {/* Embedding Index */}
      <Card>
        <div className="flex items-center gap-2 mb-4">
          <Database className="w-5 h-5 text-purple-500" />
          <CardTitle>Embedding Index</CardTitle>
        </div>

        <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
          Process meeting transcripts to build the semantic search index for RAG chat.
        </p>

        {isBatchEmbedding && batchEmbedProgress ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span>
                Processing {batchEmbedProgress.current + 1} of {batchEmbedProgress.total}
              </span>
            </div>
            <p className="text-xs text-gray-500 truncate">
              {batchEmbedProgress.currentMeeting}
            </p>
            <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
              <div
                className="bg-purple-500 h-2 rounded-full transition-all duration-300"
                style={{
                  width: `${((batchEmbedProgress.current + 1) / batchEmbedProgress.total) * 100}%`,
                }}
              />
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-between">
            <div className="text-sm">
              {unembeddedCount > 0 ? (
                <span className="text-amber-600 dark:text-amber-400">
                  {unembeddedCount} meeting{unembeddedCount !== 1 ? 's' : ''} need{unembeddedCount === 1 ? 's' : ''} embedding
                </span>
              ) : (
                <span className="text-green-600 dark:text-green-400">
                  All meetings are indexed
                </span>
              )}
            </div>
            <button
              onClick={batchEmbedMeetings}
              disabled={!embeddingReady || unembeddedCount === 0 || isBatchEmbedding}
              className="px-4 py-2 text-sm font-medium text-white bg-purple-600 rounded-lg hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {!embeddingReady
                ? 'Load Model First'
                : unembeddedCount === 0
                  ? 'All Indexed'
                  : `Index ${unembeddedCount} Meeting${unembeddedCount !== 1 ? 's' : ''}`}
            </button>
          </div>
        )}
      </Card>
    </div>
  );
}
