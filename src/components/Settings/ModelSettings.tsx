/**
 * Model settings section
 */

import { Cpu, MessageSquare, Search, Database, Loader2 } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { ProgressBar } from '../ui/Progress';
import { SkeletonCard } from '../ui/Skeleton';
import { ModelDownloadCard } from './ModelDownloadCard';
import { useModels } from '../../hooks';
import { useSettingsStore } from '../../stores';
import { formatBytes } from '../../utils/format';

function formatDownloadStage(stage: string | null): string {
  if (!stage) return 'Downloading';
  return stage
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/_/g, ' ')
    .trim();
}

function formatEta(
  downloadedBytes: number | null,
  totalBytes: number | null,
  speedBps: number | null
): string | null {
  if (
    downloadedBytes === null ||
    totalBytes === null ||
    speedBps === null ||
    speedBps <= 0
  ) {
    return null;
  }

  const remaining = Math.max(0, totalBytes - downloadedBytes);
  const etaSeconds = Math.round(remaining / speedBps);
  if (!Number.isFinite(etaSeconds) || etaSeconds <= 0) {
    return null;
  }
  if (etaSeconds < 60) {
    return `${etaSeconds}s left`;
  }
  const minutes = Math.floor(etaSeconds / 60);
  const seconds = etaSeconds % 60;
  return `${minutes}m ${seconds}s left`;
}

function getDownloadLabel(model: string | null): string {
  if (!model) return 'Model';
  if (model === 'Parakeet') return 'Parakeet Transcription Model';
  if (model === 'embedding') return 'Embedding Model';
  if (model === 'Qwen3_4B') return 'Qwen3 4B';
  if (model === 'Qwen3_1_7B') return 'Qwen3 1.7B';
  if (model === 'Qwen3_8B') return 'Qwen3 8B';
  return model;
}

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
    downloadStage,
    downloadMessage,
    downloadedBytes,
    downloadTotalBytes,
    downloadSpeedBps,
    downloadFile,
    downloadSourceModelId,
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

  const transferSummary =
    downloadedBytes !== null
      ? downloadTotalBytes !== null
        ? `${formatBytes(downloadedBytes)} / ${formatBytes(downloadTotalBytes)}`
        : `${formatBytes(downloadedBytes)} downloaded`
      : null;
  const speedSummary =
    downloadSpeedBps && downloadSpeedBps > 0
      ? `${formatBytes(downloadSpeedBps)}/s`
      : null;
  const etaSummary = formatEta(downloadedBytes, downloadTotalBytes, downloadSpeedBps);
  const downloadDetails = {
    stage: downloadStage,
    message: downloadMessage,
    downloadedBytes,
    totalBytes: downloadTotalBytes,
    speedBps: downloadSpeedBps,
    file: downloadFile,
  };

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
      {isDownloading && downloadingModel && (
        <Card className="border-indigo-200 dark:border-indigo-800 bg-indigo-50/60 dark:bg-indigo-900/10">
          <div className="flex items-start gap-3">
            <Loader2 className="w-5 h-5 mt-0.5 text-indigo-600 animate-spin" />
            <div className="flex-1 min-w-0 space-y-2">
              <div className="flex items-center justify-between gap-3">
                <p className="text-sm font-medium text-indigo-900 dark:text-indigo-200">
                  Downloading {getDownloadLabel(downloadingModel)}
                </p>
                <span className="text-sm font-semibold text-indigo-700 dark:text-indigo-300">
                  {Math.round(downloadProgress)}%
                </span>
              </div>
              <p className="text-xs text-indigo-700 dark:text-indigo-300">
                {downloadMessage ?? 'Downloading model files...'}
              </p>
              <ProgressBar
                value={downloadProgress}
                size="sm"
                color="indigo"
                label={formatDownloadStage(downloadStage)}
              />
              {(transferSummary || speedSummary || etaSummary || downloadFile || downloadSourceModelId) && (
                <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-indigo-700 dark:text-indigo-300">
                  {transferSummary && <span>{transferSummary}</span>}
                  {speedSummary && <span>{speedSummary}</span>}
                  {etaSummary && <span>{etaSummary}</span>}
                  {downloadFile && <span>File: {downloadFile}</span>}
                  {downloadSourceModelId && <span>ID: {downloadSourceModelId}</span>}
                </div>
              )}
            </div>
          </div>
        </Card>
      )}

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
          downloadDetails={
            isDownloading && downloadingModel === 'Parakeet'
              ? downloadDetails
              : undefined
          }
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
            const isThisModelLoaded = llmReady && llmStatus?.current_model === model.model;
            return (
              <ModelDownloadCard
                key={model.model}
                name={model.name}
                description={`${model.context_length.toLocaleString()} token context`}
                size={model.size_formatted}
                downloaded={model.downloaded}
                isDownloading={isDownloading && downloadingModel === model.model}
                downloadProgress={downloadingModel === model.model ? downloadProgress : 0}
                downloadDetails={
                  isDownloading && downloadingModel === model.model
                    ? downloadDetails
                    : undefined
                }
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
          downloadDetails={
            isDownloading && downloadingModel === 'embedding'
              ? downloadDetails
              : undefined
          }
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
            <ProgressBar
              value={batchEmbedProgress.current + 1}
              max={batchEmbedProgress.total}
              color="blue"
              size="sm"
              showLabel
            />
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
