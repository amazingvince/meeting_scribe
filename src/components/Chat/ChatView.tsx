/**
 * Chat view - RAG chat interface
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Sparkles, RotateCcw, Filter, ChevronDown, ChevronUp } from 'lucide-react';
import { useChat } from '../../hooks';
import { Button } from '../ui/Button';
import { ChatMessage } from './ChatMessage';
import { ChatInput } from './ChatInput';
import { ChatSuggestions } from './ChatSuggestions';
import { MeetingSelector } from './MeetingSelector';

export function ChatView() {
  const navigate = useNavigate();
  const {
    messages,
    isLoading,
    error,
    sendMessage,
    clearMessages,
    selectedMeetingIds,
    selectMeetings,
  } = useChat();

  const [showFilter, setShowFilter] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSourceClick = useCallback(
    (meetingId: string, _startMs: number | null) => {
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

  const toggleFilter = useCallback(() => {
    setShowFilter((prev) => !prev);
  }, []);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-border bg-card">
        <div className="flex items-center gap-2.5">
          <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-violet-500/10">
            <Sparkles className="w-4 h-4 text-violet-500" />
          </div>
          <div>
            <h2 className="text-foreground">Chat with Meetings</h2>
            <p className="text-xs text-muted-foreground">
              Ask questions across transcripts, decisions, and notes
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant={selectedMeetingIds.length > 0 ? 'secondary' : 'ghost'}
            size="sm"
            onClick={toggleFilter}
            className="gap-1"
          >
            <Filter className="w-4 h-4" />
            {selectedMeetingIds.length > 0 && (
              <span className="bg-indigo-600 text-white text-xs px-1.5 py-0.5 rounded-full">
                {selectedMeetingIds.length}
              </span>
            )}
            {showFilter ? (
              <ChevronUp className="w-3 h-3" />
            ) : (
              <ChevronDown className="w-3 h-3" />
            )}
          </Button>
          {hasMessages && (
            <Button
              variant="ghost"
              size="sm"
              onClick={clearMessages}
              className="text-muted-foreground gap-1.5"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              New chat
            </Button>
          )}
        </div>
      </header>

      {/* Meeting filter panel */}
      {showFilter && (
        <div className="border-b border-border bg-muted/50 px-6 py-3">
          <MeetingSelector
            selectedIds={selectedMeetingIds}
            onSelect={selectMeetings}
          />
        </div>
      )}

      {error && (
        <div className="border-b border-destructive/30 bg-destructive/5 px-6 py-2">
          <p className="text-sm text-destructive">{error}</p>
        </div>
      )}

      {/* Chat area */}
      <div className="no-scrollbar flex-1 min-h-0 overflow-y-auto">
        {!hasMessages ? (
          <div className="flex flex-col items-center justify-center h-full min-h-[500px] px-6">
            <div className="flex items-center justify-center w-14 h-14 rounded-2xl bg-violet-500/10 mb-5">
              <Sparkles className="w-7 h-7 text-violet-500" />
            </div>
            <h3 className="text-foreground mb-1.5">
              Ask anything about your meetings
            </h3>
            <p className="text-sm text-muted-foreground mb-8 text-center max-w-md">
              Search through transcripts, get summaries, find action items,
              and more — powered by AI.
            </p>
            <ChatSuggestions onSelect={handleSuggestionSelect} />
          </div>
        ) : (
          <div className="max-w-3xl mx-auto px-6 py-6 space-y-6">
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
