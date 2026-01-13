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
  placeholder = 'Ask a question about your meetings...',
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
    <div className="flex items-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={isLoading}
        rows={1}
        className="
          flex-1 resize-none
          px-4 py-2 rounded-lg
          border border-gray-300 dark:border-gray-600
          bg-white dark:bg-gray-800
          text-gray-900 dark:text-gray-100
          placeholder-gray-400 dark:placeholder-gray-500
          focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500
          disabled:bg-gray-100 dark:disabled:bg-gray-900
          max-h-32
        "
      />
      <Button
        onClick={handleSubmit}
        disabled={!value.trim() || isLoading}
        isLoading={isLoading}
        className="flex-shrink-0"
      >
        <Send className="w-4 h-4" />
      </Button>
    </div>
  );
}
