/**
 * Chat message component — flat layout with avatar icons
 */

import { User, Bot, ExternalLink } from 'lucide-react';
import type { ChatMessage as ChatMessageType, ChatSource } from '../../types';

interface ChatMessageProps {
  message: ChatMessageType;
  onSourceClick?: (meetingId: string, startMs: number | null) => void;
}

export function ChatMessage({ message, onSourceClick }: ChatMessageProps) {
  const isUser = message.role === 'user';

  return (
    <div className="flex gap-3">
      <div
        className={`flex items-center justify-center w-7 h-7 rounded-lg shrink-0 mt-0.5 ${
          isUser
            ? 'bg-primary text-primary-foreground'
            : 'bg-violet-500/10'
        }`}
      >
        {isUser ? (
          <User className="w-3.5 h-3.5" />
        ) : (
          <Bot className="w-3.5 h-3.5 text-violet-500" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <span className="text-xs text-muted-foreground">
          {isUser ? 'You' : 'AI Assistant'}
        </span>

        <div className="mt-1 text-sm text-foreground/90 leading-relaxed whitespace-pre-wrap">
          {message.isStreaming ? (
            <div className="flex items-center gap-1.5 pt-1">
              <span className="w-1.5 h-1.5 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:0ms]" />
              <span className="w-1.5 h-1.5 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:150ms]" />
              <span className="w-1.5 h-1.5 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:300ms]" />
            </div>
          ) : (
            message.content
          )}
        </div>

        {/* Sources */}
        {message.sources && message.sources.length > 0 && (
          <div className="mt-3 flex items-center gap-1.5 flex-wrap">
            <span className="text-[11px] text-muted-foreground/60">
              Sources:
            </span>
            {message.sources.map((source, idx) => (
              <SourceBadge
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

interface SourceBadgeProps {
  source: ChatSource;
  onClick: () => void;
}

function SourceBadge({ source, onClick }: SourceBadgeProps) {
  return (
    <button
      onClick={onClick}
      className="inline-flex items-center gap-1 text-[11px] py-0 px-2 rounded-full bg-secondary text-secondary-foreground hover:bg-accent transition-colors cursor-pointer"
    >
      <ExternalLink className="w-2.5 h-2.5" />
      {source.meeting_title || 'Meeting'} · {Math.round(source.similarity * 100)}%
    </button>
  );
}
