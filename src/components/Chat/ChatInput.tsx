/**
 * Chat input component
 */

import { useState, useCallback, useRef, useEffect } from 'react';
import { Send } from 'lucide-react';
import { Button } from '../ui/Button';

interface ChatInputProps {
  onSend: (message: string) => void;
  isLoading: boolean;
  placeholder?: string;
}

export function ChatInput({
  onSend,
  isLoading,
  placeholder = 'Ask about your meetings...',
}: ChatInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [value]);

  const handleSubmit = useCallback(() => {
    if (!value.trim() || isLoading) return;
    onSend(value.trim());
    setValue('');
  }, [value, isLoading, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit]
  );

  return (
    <div className="border-t border-border bg-card px-6 py-4">
      <div className="max-w-3xl mx-auto">
        <div className="relative flex items-end border border-border rounded-xl bg-background overflow-hidden focus-within:ring-1 focus-within:ring-ring/50">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            disabled={isLoading}
            rows={1}
            className="flex-1 px-4 py-3 text-sm bg-transparent resize-none outline-none max-h-32 placeholder:text-muted-foreground/50"
            style={{ minHeight: '44px' }}
          />
          <Button
            variant="ghost"
            size="sm"
            onClick={handleSubmit}
            disabled={!value.trim() || isLoading}
            isLoading={isLoading}
            className="m-1.5 text-muted-foreground hover:text-foreground"
          >
            <Send className="w-4 h-4" />
          </Button>
        </div>
        <p className="text-[11px] text-muted-foreground/50 mt-2 text-center">
          AI responses are generated from your meeting transcripts. Results may not always be accurate.
        </p>
      </div>
    </div>
  );
}
