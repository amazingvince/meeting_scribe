/**
 * Model selector for Library page
 * Allows selecting default transcription and LLM models
 */

import { Cpu, MessageSquare } from 'lucide-react';
import { useModels } from '../../hooks';
import { useSettingsStore } from '../../stores';
import { Select } from '../ui/Select';

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
          <Cpu className="h-3.5 w-3.5 text-brand" />
          Transcription Model
        </div>
        <Select
          value={transcriptionBackend}
          onChange={(e) => setTranscriptionBackend(e.target.value as 'Parakeet')}
          disabled={!transcriptionDownloaded}
          className="h-9"
        >
          {transcriptionDownloaded ? (
            <option value="Parakeet">Parakeet</option>
          ) : (
            <option value="">Not downloaded</option>
          )}
        </Select>
      </div>

      <div className="rounded-xl border border-border bg-card/85 p-2.5">
        <div className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          <MessageSquare className="h-3.5 w-3.5 text-success" />
          Summary Model
        </div>
        <Select
          value={llmModel}
          onChange={(e) => setLlmModel(e.target.value as typeof llmModel)}
          disabled={downloadedLlmModels.length === 0}
          className="h-9"
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
        </Select>
      </div>
    </div>
  );
}
