/**
 * Chat suggestion prompts
 */

import { MessageSquare, ListChecks, Search, Lightbulb } from 'lucide-react';
import { defaultChatSuggestions, type ChatSuggestion } from '../../types';

interface ChatSuggestionsProps {
  onSelect: (prompt: string) => void;
}

const icons = {
  summary: MessageSquare,
  action: ListChecks,
  search: Search,
  general: Lightbulb,
};

export function ChatSuggestions({ onSelect }: ChatSuggestionsProps) {
  return (
    <div className="grid grid-cols-2 gap-3 p-4">
      {defaultChatSuggestions.map((suggestion, idx) => (
        <SuggestionCard
          key={idx}
          suggestion={suggestion}
          onClick={() => onSelect(suggestion.prompt)}
        />
      ))}
    </div>
  );
}

interface SuggestionCardProps {
  suggestion: ChatSuggestion;
  onClick: () => void;
}

function SuggestionCard({ suggestion, onClick }: SuggestionCardProps) {
  const Icon = icons[suggestion.category];

  return (
    <button
      onClick={onClick}
      className="
        flex items-start gap-3 p-4
        bg-gray-50 dark:bg-gray-800/50
        border border-gray-200 dark:border-gray-700
        rounded-lg
        hover:border-indigo-300 dark:hover:border-indigo-600
        hover:bg-gray-100 dark:hover:bg-gray-800
        transition-colors
        text-left
      "
    >
      <Icon className="w-5 h-5 text-indigo-500 flex-shrink-0 mt-0.5" />
      <span className="text-sm text-gray-700 dark:text-gray-300">
        {suggestion.label}
      </span>
    </button>
  );
}
