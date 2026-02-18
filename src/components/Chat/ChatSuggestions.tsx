/**
 * Chat suggestion prompts — simple clickable buttons with category icons
 */

import { FileText, ListChecks, Search, Lightbulb } from 'lucide-react';
import { defaultChatSuggestions } from '../../types';

const categoryIcons: Record<string, typeof FileText> = {
  summary: FileText,
  action: ListChecks,
  search: Search,
  general: Lightbulb,
};

interface ChatSuggestionsProps {
  onSelect: (prompt: string) => void;
}

export function ChatSuggestions({ onSelect }: ChatSuggestionsProps) {
  return (
    <div className="grid grid-cols-2 gap-2 w-full max-w-lg">
      {defaultChatSuggestions.map((suggestion, idx) => {
        const Icon = categoryIcons[suggestion.category] ?? Lightbulb;
        return (
          <button
            key={idx}
            onClick={() => onSelect(suggestion.prompt)}
            className="flex flex-col items-start gap-2 text-left text-sm text-foreground/80 px-4 py-3 rounded-xl border border-border hover:bg-accent/50 hover:border-border transition-colors"
          >
            <Icon className="w-4 h-4 text-brand" />
            {suggestion.label}
          </button>
        );
      })}
    </div>
  );
}
