/**
 * Chat message component
 */

import { User, Bot, ExternalLink } from 'lucide-react';
import type { ChatMessage as ChatMessageType, ChatSource } from '../../types';
import { Spinner } from '../ui/Progress';

interface ChatMessageProps {
  message: ChatMessageType;
  onSourceClick?: (meetingId: string, startMs: number | null) => void;
}

export function ChatMessage({ message, onSourceClick }: ChatMessageProps) {
  const isUser = message.role === 'user';

  return (
    <div className={`flex gap-3 ${isUser ? 'flex-row-reverse' : ''}`}>
      <div
        className={`
          flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center
          ${isUser ? 'bg-indigo-100 dark:bg-indigo-900/30' : 'bg-gray-100 dark:bg-gray-700'}
        `}
      >
        {isUser ? (
          <User className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
        ) : (
          <Bot className="w-4 h-4 text-gray-600 dark:text-gray-400" />
        )}
      </div>

      <div className={`flex-1 max-w-[80%] ${isUser ? 'text-right' : ''}`}>
        <div
          className={`
            inline-block p-3 rounded-lg
            ${
              isUser
                ? 'bg-indigo-600 text-white'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
            }
          `}
        >
          {message.isStreaming ? (
            <div className="flex items-center gap-2">
              <Spinner size="sm" />
              <span className="text-gray-500">Thinking...</span>
            </div>
          ) : (
            <p className="whitespace-pre-wrap">{message.content}</p>
          )}
        </div>

        {/* Sources */}
        {message.sources && message.sources.length > 0 && (
          <div className="mt-2 space-y-1">
            <p className="text-xs text-gray-500 dark:text-gray-400">Sources:</p>
            {message.sources.map((source, idx) => (
              <SourceCard
                key={idx}
                source={source}
                onClick={() =>
                  onSourceClick?.(source.meeting_id, source.start_ms)
                }
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface SourceCardProps {
  source: ChatSource;
  onClick: () => void;
}

function SourceCard({ source, onClick }: SourceCardProps) {
  return (
    <button
      onClick={onClick}
      className="block w-full text-left p-2 bg-gray-50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700 rounded hover:border-indigo-300 dark:hover:border-indigo-600 transition-colors"
    >
      <div className="flex items-center gap-2 text-xs text-indigo-600 dark:text-indigo-400">
        <ExternalLink className="w-3 h-3" />
        <span className="truncate">
          {source.meeting_title || 'Meeting'}
        </span>
        <span className="text-gray-400">
          ({Math.round(source.similarity * 100)}% match)
        </span>
      </div>
      <p className="text-xs text-gray-600 dark:text-gray-400 mt-1 line-clamp-2">
        {source.excerpt}
      </p>
    </button>
  );
}
