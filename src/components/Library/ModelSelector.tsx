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
    <div className="grid gap-2 md:grid-cols-2">
      <div className="rounded-xl border border-border bg-card/85 p-2.5">
        <div className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          <Cpu className="h-3.5 w-3.5 text-indigo-500" />
          Transcription Model
        </div>
        <select
          value={transcriptionBackend}
          onChange={(e) => setTranscriptionBackend(e.target.value as 'Parakeet')}
          disabled={!transcriptionDownloaded}
          className="h-9 w-full rounded-md border border-input bg-input-background px-2 py-1 text-sm text-foreground disabled:cursor-not-allowed disabled:opacity-50"
        >
          {transcriptionDownloaded ? (
            <option value="Parakeet">Parakeet</option>
          ) : (
            <option value="">Not downloaded</option>
          )}
        </select>
      </div>

      <div className="rounded-xl border border-border bg-card/85 p-2.5">
        <div className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          <MessageSquare className="h-3.5 w-3.5 text-green-500" />
          Summary Model
        </div>
        <select
          value={llmModel}
          onChange={(e) => setLlmModel(e.target.value as typeof llmModel)}
          disabled={downloadedLlmModels.length === 0}
          className="h-9 w-full rounded-md border border-input bg-input-background px-2 py-1 text-sm text-foreground disabled:cursor-not-allowed disabled:opacity-50"
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
