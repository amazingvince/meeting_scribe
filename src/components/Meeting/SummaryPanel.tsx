/**
 * Summary panel with generated summary and action items
 * Fetches and saves summaries via Tauri API
 */

import { useState, useCallback, useEffect } from 'react';
import { Sparkles, ListChecks, RefreshCw, Loader2 } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { ActionItem } from '../../types';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { Card } from '../ui/Card';
import { SkeletonText } from '../ui/Skeleton';
import { useTauriEvent } from '../../hooks';
import * as api from '../../lib/tauri';
import { useToastStore, useSettingsStore } from '../../stores';

interface SummaryPanelProps {
  meetingId: string;
  hasTranscript: boolean;
}

export function SummaryPanel({ meetingId, hasTranscript }: SummaryPanelProps) {
  const toast = useToastStore();
  const settings = useSettingsStore();
  const [summary, setSummary] = useState<string | null>(null);
  const [actionItems, setActionItems] = useState<ActionItem[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingSummary, setIsLoadingSummary] = useState(false);
  const [isLoadingActions, setIsLoadingActions] = useState(false);
  const [summaryStatus, setSummaryStatus] = useState<string | null>(null);
  const [actionsStatus, setActionsStatus] = useState<string | null>(null);

  // Load saved summaries on mount
  useEffect(() => {
    // Reset panel state when meeting context changes so stale data never carries over.
    setSummary(null);
    setActionItems([]);
    setIsLoadingSummary(false);
    setIsLoadingActions(false);
    setSummaryStatus(null);
    setActionsStatus(null);

    const loadSummaries = async () => {
      setIsLoading(true);
      try {
        // Load saved full summary
        const savedSummary = await api.getSummary(meetingId, 'full');
        if (savedSummary) {
          setSummary(savedSummary.content);
        }

        // Load saved action items
        const savedActions = await api.getSummary(meetingId, 'action_items');
        if (savedActions) {
          try {
            const items = JSON.parse(savedActions.content) as ActionItem[];
            setActionItems(items);
          } catch {
            // Invalid JSON, ignore
          }
        }
      } catch (e) {
        console.error('Failed to load summaries:', e);
      } finally {
        setIsLoading(false);
      }
    };

    if (hasTranscript) {
      loadSummaries();
    } else {
      setIsLoading(false);
    }
  }, [meetingId, hasTranscript]);

  useTauriEvent<api.SummaryGenerationProgressEvent>(
    'summary-generation-progress',
    (event) => {
      if (event.meeting_id !== meetingId) return;

      if (event.summary_type === 'full') {
        setIsLoadingSummary(true);
        setSummaryStatus(event.message || 'Generating summary...');
      } else if (event.summary_type === 'action_items') {
        setIsLoadingActions(true);
        setActionsStatus(event.message || 'Extracting action items...');
      }
    }
  );

  useTauriEvent<api.SummaryGenerationFinishedEvent>(
    'summary-generation-finished',
    (event) => {
      if (event.meeting_id !== meetingId) return;

      if (event.summary_type === 'full') {
        setIsLoadingSummary(false);
        setSummaryStatus(null);

        if (event.success) {
          const nextSummary = event.summary?.trim() ?? '';
          if (nextSummary.length > 0) {
            setSummary(nextSummary);
          } else {
            void (async () => {
              const savedSummary = await api.getSummary(meetingId, 'full');
              if (savedSummary?.content) {
                setSummary(savedSummary.content);
              }
            })();
          }
        } else {
          toast.error(
            'Failed to generate summary',
            event.error_message ?? 'Background summary generation failed.'
          );
        }
        return;
      }

      if (event.summary_type === 'action_items') {
        setIsLoadingActions(false);
        setActionsStatus(null);

        if (event.success) {
          if (event.action_items && event.action_items.length > 0) {
            setActionItems(event.action_items);
          } else if (event.action_items) {
            setActionItems([]);
          } else {
            void (async () => {
              const savedActions = await api.getSummary(meetingId, 'action_items');
              if (savedActions?.content) {
                try {
                  const parsed = JSON.parse(savedActions.content) as ActionItem[];
                  setActionItems(parsed);
                } catch {
                  setActionItems([]);
                }
              } else {
                setActionItems([]);
              }
            })();
          }
        } else {
          toast.error(
            'Failed to extract action items',
            event.error_message ?? 'Background action-item extraction failed.'
          );
        }
      }
    }
  );

  const generateSummary = useCallback(async () => {
    setIsLoadingSummary(true);
    setSummaryStatus('Checking language model...');
    try {
      // Check if LLM is ready, try to load if not
      let llmReady = settings.llmReady;
      if (!llmReady) {
        setSummaryStatus('Loading language model...');
        llmReady = await settings.initializeLlm();
      }

      if (!llmReady) {
        toast.warning(
          'LLM not available',
          'Download a language model in Settings to generate summaries.'
        );
        setIsLoadingSummary(false);
        setSummaryStatus('Language model unavailable.');
        return;
      }

      setSummaryStatus('Queued for background generation...');
      await api.startSummaryGeneration(meetingId, 'full');
    } catch (e) {
      setIsLoadingSummary(false);
      setSummaryStatus(null);
      toast.error(
        'Failed to generate summary',
        e instanceof Error ? e.message : String(e)
      );
    }
  }, [meetingId, settings, toast]);

  const extractActionItems = useCallback(async () => {
    setIsLoadingActions(true);
    setActionsStatus('Checking language model...');
    try {
      // Check if LLM is ready, try to load if not
      let llmReady = settings.llmReady;
      if (!llmReady) {
        setActionsStatus('Loading language model...');
        llmReady = await settings.initializeLlm();
      }

      if (!llmReady) {
        toast.warning(
          'LLM not available',
          'Download a language model in Settings to extract action items.'
        );
        setIsLoadingActions(false);
        setActionsStatus('Language model unavailable.');
        return;
      }

      setActionsStatus('Queued for background extraction...');
      await api.startSummaryGeneration(meetingId, 'action_items');
    } catch (e) {
      setIsLoadingActions(false);
      setActionsStatus(null);
      toast.error(
        'Failed to extract action items',
        e instanceof Error ? e.message : String(e)
      );
    }
  }, [meetingId, settings, toast]);

  const isAnyGenerationRunning = isLoadingSummary || isLoadingActions;
  const activeTaskMessage = isLoadingSummary
    ? summaryStatus
    : isLoadingActions
      ? actionsStatus
      : null;

  if (!hasTranscript) {
    return (
      <div className="p-4 text-center text-muted-foreground">
        <p>Generate a transcript first to create summaries.</p>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="p-4 space-y-6">
        <Card>
          <SkeletonText lines={4} />
        </Card>
        <Card>
          <SkeletonText lines={3} />
        </Card>
      </div>
    );
  }

  return (
    <div className="no-scrollbar h-full overflow-y-auto p-4 md:p-5 space-y-5">
      {isAnyGenerationRunning && activeTaskMessage && (
        <Card className="bg-muted/50">
          <div className="flex items-center gap-2 text-sm text-foreground">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span className="font-medium">{activeTaskMessage}</span>
          </div>
        </Card>
      )}

      {/* Summary section */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-foreground flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-brand" />
            Summary
          </h3>
          <Button
            variant="secondary"
            size="sm"
            onClick={generateSummary}
            isLoading={isLoadingSummary}
            disabled={isLoadingSummary}
          >
            {summary ? (
              <>
                <RefreshCw className="w-4 h-4" />
                Regenerate
              </>
            ) : (
              'Generate'
            )}
          </Button>
        </div>

        {isLoadingSummary && summaryStatus && (
          <p className="text-xs text-muted-foreground mb-3">
            {summaryStatus}
          </p>
        )}

        {isLoadingSummary ? (
          <SkeletonText lines={4} />
        ) : summary ? (
          <div className="text-foreground/80 text-sm leading-relaxed prose-sm">
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
                strong: ({ children }) => (
                  <strong className="font-semibold">{children}</strong>
                ),
                blockquote: ({ children }) => (
                  <blockquote className="border-l-2 border-brand pl-3 my-2 text-muted-foreground italic">
                    {children}
                  </blockquote>
                ),
                hr: () => <hr className="my-3 border-border" />,
              }}
            >
              {summary}
            </ReactMarkdown>
          </div>
        ) : (
          <p className="text-muted-foreground text-sm">
            Click "Generate" to create an AI summary of this meeting.
          </p>
        )}
      </Card>

      {/* Action items section */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-foreground flex items-center gap-2">
            <ListChecks className="w-5 h-5 text-success" />
            Action Items
          </h3>
          <Button
            variant="secondary"
            size="sm"
            onClick={extractActionItems}
            isLoading={isLoadingActions}
            disabled={isLoadingActions}
          >
            {actionItems.length > 0 ? (
              <>
                <RefreshCw className="w-4 h-4" />
                Re-extract
              </>
            ) : (
              'Extract'
            )}
          </Button>
        </div>

        {isLoadingActions && actionsStatus && (
          <p className="text-xs text-muted-foreground mb-3">
            {actionsStatus}
          </p>
        )}

        {isLoadingActions ? (
          <SkeletonText lines={3} />
        ) : actionItems.length > 0 ? (
          <ul className="space-y-3">
            {actionItems.map((item, idx) => (
              <li key={idx} className="flex items-start gap-3">
                <Badge
                  variant={
                    item.priority === 'high'
                      ? 'error'
                      : item.priority === 'medium'
                        ? 'warning'
                        : 'secondary'
                  }
                  className="mt-0.5"
                >
                  {item.priority}
                </Badge>
                <div className="flex-1">
                  <p className="text-foreground/80">{item.task}</p>
                  {(item.owner || item.deadline) && (
                    <p className="text-sm text-muted-foreground mt-1">
                      {item.owner && <span>Owner: {item.owner}</span>}
                      {item.owner && item.deadline && <span> | </span>}
                      {item.deadline && <span>Due: {item.deadline}</span>}
                    </p>
                  )}
                </div>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted-foreground text-sm">
            Click "Extract" to find action items from this meeting.
          </p>
        )}
      </Card>
    </div>
  );
}
