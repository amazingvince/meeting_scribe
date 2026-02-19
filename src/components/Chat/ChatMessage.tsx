/**
 * Chat message component — flat layout with avatar icons
 */

import { User, Bot, ExternalLink } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { ChatMessage as ChatMessageType, ChatSource } from '../../types';
import { formatDuration } from '../../utils/format';

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
            : 'bg-brand/10'
        }`}
      >
        {isUser ? (
          <User className="w-3.5 h-3.5" />
        ) : (
          <Bot className="w-3.5 h-3.5 text-brand" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <span className="text-xs text-muted-foreground">
          {isUser ? 'You' : 'AI Assistant'}
        </span>

        <div className="mt-1 text-sm text-foreground/90 leading-relaxed">
          {message.isStreaming ? (
            <div className="flex items-center gap-1.5 pt-1">
              <span className="w-1.5 h-1.5 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:0ms]" />
              <span className="w-1.5 h-1.5 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:150ms]" />
              <span className="w-1.5 h-1.5 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:300ms]" />
            </div>
          ) : isUser ? (
            <p className="whitespace-pre-wrap">{message.content}</p>
          ) : (
            <MarkdownContent content={message.content} />
          )}
        </div>

        {/* Sources */}
        {message.sources && message.sources.length > 0 && (
          <div className="mt-3 space-y-2">
            <div className="flex items-center gap-1.5 flex-wrap">
              <span className="text-[11px] text-muted-foreground/60">
                Sources:
              </span>
              {message.sources.map((source, idx) => (
                <SourceBadge
                  key={`${source.meeting_id}:${source.start_ms ?? idx}`}
                  source={source}
                  onClick={() =>
                    onSourceClick?.(source.meeting_id, source.start_ms)
                  }
                />
              ))}
            </div>
            <div className="space-y-1.5">
              {message.sources.slice(0, 5).map((source, idx) => (
                <SourceExcerpt
                  key={`excerpt:${source.meeting_id}:${source.start_ms ?? idx}`}
                  source={source}
                  onClick={() => onSourceClick?.(source.meeting_id, source.start_ms)}
                />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function MarkdownContent({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        p: ({ children }) => <p className="mb-2 last:mb-0">{children}</p>,
        h1: ({ children }) => (
          <h1 className="text-base font-semibold mt-4 mb-2">{children}</h1>
        ),
        h2: ({ children }) => (
          <h2 className="text-sm font-semibold mt-3 mb-1.5">{children}</h2>
        ),
        h3: ({ children }) => (
          <h3 className="text-sm font-semibold mt-2 mb-1">{children}</h3>
        ),
        ul: ({ children }) => (
          <ul className="list-disc pl-5 mb-2 space-y-0.5">{children}</ul>
        ),
        ol: ({ children }) => (
          <ol className="list-decimal pl-5 mb-2 space-y-0.5">{children}</ol>
        ),
        li: ({ children }) => <li>{children}</li>,
        code: ({ className, children, ...props }) => {
          const isBlock = className?.includes('language-');
          if (isBlock) {
            return (
              <pre className="my-2 rounded-lg bg-muted p-3 overflow-x-auto text-xs">
                <code className={className} {...props}>
                  {children}
                </code>
              </pre>
            );
          }
          return (
            <code
              className="rounded bg-muted px-1.5 py-0.5 text-xs font-mono"
              {...props}
            >
              {children}
            </code>
          );
        },
        pre: ({ children }) => <>{children}</>,
        blockquote: ({ children }) => (
          <blockquote className="border-l-2 border-brand pl-3 my-2 text-muted-foreground italic">
            {children}
          </blockquote>
        ),
        strong: ({ children }) => (
          <strong className="font-semibold">{children}</strong>
        ),
        a: ({ href, children }) => (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="text-brand underline underline-offset-2 hover:text-brand/80"
          >
            {children}
          </a>
        ),
        hr: () => <hr className="my-3 border-border" />,
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

interface SourceBadgeProps {
  source: ChatSource;
  onClick: () => void;
}

function SourceBadge({ source, onClick }: SourceBadgeProps) {
  const timeLabel =
    source.start_ms !== null
      ? source.end_ms != null
        ? `${formatDuration(source.start_ms)}-${formatDuration(source.end_ms)}`
        : formatDuration(source.start_ms)
      : null;

  return (
    <button
      onClick={onClick}
      title={truncateExcerpt(source.excerpt)}
      className="inline-flex items-center gap-1 text-[11px] py-0 px-2 rounded-full bg-secondary text-secondary-foreground hover:bg-accent transition-colors cursor-pointer"
    >
      <ExternalLink className="w-2.5 h-2.5" />
      {source.meeting_title || 'Meeting'} · {Math.round(source.similarity * 100)}%
      {timeLabel ? ` · ${timeLabel}` : ''}
    </button>
  );
}

interface SourceExcerptProps {
  source: ChatSource;
  onClick: () => void;
}

function SourceExcerpt({ source, onClick }: SourceExcerptProps) {
  const timeLabel =
    source.start_ms !== null
      ? source.end_ms != null
        ? `${formatDuration(source.start_ms)}-${formatDuration(source.end_ms)}`
        : formatDuration(source.start_ms)
      : 'open';

  return (
    <button
      type="button"
      className="w-full rounded-md border border-border/70 bg-card/40 px-2.5 py-2 text-left text-xs transition-colors hover:bg-accent/40"
      onClick={onClick}
    >
      <div className="flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
        <span className="truncate">{source.meeting_title || 'Meeting'}</span>
        <span className="font-mono text-brand">{timeLabel}</span>
      </div>
      <p className="mt-1 line-clamp-2 text-foreground/90">
        {truncateExcerpt(source.excerpt)}
      </p>
    </button>
  );
}

function truncateExcerpt(text: string, maxLength = 180): string {
  const compact = text.replace(/\s+/g, ' ').trim();
  if (compact.length <= maxLength) {
    return compact;
  }
  return `${compact.slice(0, maxLength - 1).trimEnd()}…`;
}
