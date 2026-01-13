/**
 * Model selector for Library page
 * Allows selecting default transcription and LLM models
 */

import { Cpu, MessageSquare } from 'lucide-react';
import { useModels } from '../../hooks';
import { useSettingsStore } from '../../stores';

export function ModelSelector() {
  const {
    transcriptionBackend,
    llmModel,
    setTranscriptionBackend,
    setLlmModel,
  } = useSettingsStore();

  const { transcriptionDownloaded, llmModels } = useModels();

  // Filter to only downloaded LLM models
  const downloadedLlmModels = llmModels.filter((m) => m.downloaded);

  return (
    <div className="flex flex-wrap items-center gap-4 p-3 bg-gray-50 dark:bg-gray-800/50 rounded-lg">
      {/* Transcription Model */}
      <div className="flex items-center gap-2">
        <Cpu className="w-4 h-4 text-indigo-500" />
        <label className="text-sm text-gray-600 dark:text-gray-400">
          Transcription:
        </label>
        <select
          value={transcriptionBackend}
          onChange={(e) => setTranscriptionBackend(e.target.value as 'Parakeet')}
          disabled={!transcriptionDownloaded}
          className="text-sm bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded px-2 py-1 text-gray-900 dark:text-gray-100 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {transcriptionDownloaded ? (
            <option value="Parakeet">Parakeet</option>
          ) : (
            <option value="">Not downloaded</option>
          )}
        </select>
      </div>

      {/* LLM Model */}
      <div className="flex items-center gap-2">
        <MessageSquare className="w-4 h-4 text-green-500" />
        <label className="text-sm text-gray-600 dark:text-gray-400">LLM:</label>
        <select
          value={llmModel}
          onChange={(e) => setLlmModel(e.target.value as typeof llmModel)}
          disabled={downloadedLlmModels.length === 0}
          className="text-sm bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded px-2 py-1 text-gray-900 dark:text-gray-100 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {downloadedLlmModels.length > 0 ? (
            downloadedLlmModels.map((model) => (
              <option key={model.model} value={model.model}>
                {model.name}
              </option>
            ))
          ) : (
            <option value="">Not downloaded</option>
          )}
        </select>
      </div>
    </div>
  );
}
