/**
 * Chat suggestion prompts — simple clickable buttons
 */

import { defaultChatSuggestions } from '../../types';

interface ChatSuggestionsProps {
  onSelect: (prompt: string) => void;
}

export function ChatSuggestions({ onSelect }: ChatSuggestionsProps) {
  return (
    <div className="grid grid-cols-2 gap-2 w-full max-w-lg">
      {defaultChatSuggestions.map((suggestion, idx) => (
        <button
          key={idx}
          onClick={() => onSelect(suggestion.prompt)}
          className="text-left text-sm text-foreground/80 px-4 py-3 rounded-xl border border-border hover:bg-accent/50 hover:border-border transition-colors"
        >
          {suggestion.label}
        </button>
      ))}
    </div>
  );
}
