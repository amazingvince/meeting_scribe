/**
 * Chat view - RAG chat interface
 */

import { useCallback, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Trash2 } from 'lucide-react';
import { useChat } from '../../hooks';
import { Button } from '../ui/Button';
import { NoChatMessagesEmpty } from '../ui/EmptyState';
import { ChatMessage } from './ChatMessage';
import { ChatInput } from './ChatInput';
import { ChatSuggestions } from './ChatSuggestions';

export function ChatView() {
  const navigate = useNavigate();
  const {
    messages,
    isLoading,
    error,
    sendMessage,
    clearMessages,
  } = useChat();

  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSourceClick = useCallback(
    (meetingId: string, _startMs: number | null) => {
      // Navigate to meeting detail
      // TODO: Use _startMs to seek audio player when implemented
      navigate(`/meeting/${meetingId}`);
    },
    [navigate]
  );

  const handleSuggestionSelect = useCallback(
    (prompt: string) => {
      sendMessage(prompt);
    },
    [sendMessage]
  );

  const hasMessages = messages.length > 0;

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Chat with Meetings
        </h1>
        {hasMessages && (
          <Button variant="ghost" size="sm" onClick={clearMessages}>
            <Trash2 className="w-4 h-4" />
            Clear
          </Button>
        )}
      </div>

      {/* Error banner */}
      {error && (
        <div className="px-6 py-2 bg-red-50 dark:bg-red-900/20 border-b border-red-200 dark:border-red-800">
          <p className="text-sm text-red-700 dark:text-red-400">{error}</p>
        </div>
      )}

      {/* Messages area */}
      <div className="flex-1 overflow-y-auto">
        {!hasMessages ? (
          <div className="h-full flex flex-col items-center justify-center p-4">
            <NoChatMessagesEmpty />
            <div className="mt-8 w-full max-w-2xl">
              <p className="text-sm text-gray-500 dark:text-gray-400 text-center mb-4">
                Try one of these suggestions:
              </p>
              <ChatSuggestions onSelect={handleSuggestionSelect} />
            </div>
          </div>
        ) : (
          <div className="p-6 space-y-6">
            {messages.map((message) => (
              <ChatMessage
                key={message.id}
                message={message}
                onSourceClick={handleSourceClick}
              />
            ))}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* Input */}
      <ChatInput onSend={sendMessage} isLoading={isLoading} />
    </div>
  );
}
