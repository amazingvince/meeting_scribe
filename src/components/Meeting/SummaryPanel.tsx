/**
 * Summary panel with generated summary and action items
 * Fetches and saves summaries via Tauri API
 */

import { useState, useCallback, useEffect } from 'react';
import { Sparkles, ListChecks, RefreshCw } from 'lucide-react';
import type { ActionItem } from '../../types';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SkeletonText } from '../ui/Skeleton';
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

  // Load saved summaries on mount
  useEffect(() => {
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

  const generateSummary = useCallback(async () => {
    setIsLoadingSummary(true);
    try {
      // Check if LLM is ready, try to load if not
      let llmReady = settings.llmReady;
      if (!llmReady) {
        toast.info('Loading language model...');
        llmReady = await settings.initializeLlm();
      }

      if (!llmReady) {
        toast.warning(
          'LLM not available',
          'Download a language model in Settings to generate summaries.'
        );
        setIsLoadingSummary(false);
        return;
      }

      // Generate the summary via LLM
      const result = await api.generateSummary(meetingId);
      setSummary(result);

      // Save the summary for persistence
      await api.saveSummary(
        meetingId,
        'full',
        result,
        settings.llmStatus?.current_model ?? undefined
      );

      toast.success('Summary generated and saved');
    } catch (e) {
      toast.error(
        'Failed to generate summary',
        e instanceof Error ? e.message : String(e)
      );
    } finally {
      setIsLoadingSummary(false);
    }
  }, [meetingId, settings, toast]);

  const extractActionItems = useCallback(async () => {
    setIsLoadingActions(true);
    try {
      // Check if LLM is ready, try to load if not
      let llmReady = settings.llmReady;
      if (!llmReady) {
        toast.info('Loading language model...');
        llmReady = await settings.initializeLlm();
      }

      if (!llmReady) {
        toast.warning(
          'LLM not available',
          'Download a language model in Settings to extract action items.'
        );
        setIsLoadingActions(false);
        return;
      }

      // Extract action items via LLM
      const result = await api.extractActionItems(meetingId);
      setActionItems(result);

      // Save action items as JSON for persistence
      await api.saveSummary(
        meetingId,
        'action_items',
        JSON.stringify(result),
        settings.llmStatus?.current_model ?? undefined
      );

      toast.success(`Found ${result.length} action items`);
    } catch (e) {
      toast.error(
        'Failed to extract action items',
        e instanceof Error ? e.message : String(e)
      );
    } finally {
      setIsLoadingActions(false);
    }
  }, [meetingId, settings, toast]);

  if (!hasTranscript) {
    return (
      <div className="p-4 text-center text-gray-500 dark:text-gray-400">
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
    <div className="p-4 space-y-6">
      {/* Summary section */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-indigo-500" />
            Summary
          </h3>
          <Button
            variant="secondary"
            size="sm"
            onClick={generateSummary}
            isLoading={isLoadingSummary}
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

        {isLoadingSummary ? (
          <SkeletonText lines={4} />
        ) : summary ? (
          <p className="text-gray-700 dark:text-gray-300 whitespace-pre-wrap">
            {summary}
          </p>
        ) : (
          <p className="text-gray-500 dark:text-gray-400 text-sm">
            Click "Generate" to create an AI summary of this meeting.
          </p>
        )}
      </Card>

      {/* Action items section */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <ListChecks className="w-5 h-5 text-green-500" />
            Action Items
          </h3>
          <Button
            variant="secondary"
            size="sm"
            onClick={extractActionItems}
            isLoading={isLoadingActions}
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

        {isLoadingActions ? (
          <SkeletonText lines={3} />
        ) : actionItems.length > 0 ? (
          <ul className="space-y-3">
            {actionItems.map((item, idx) => (
              <li key={idx} className="flex items-start gap-3">
                <span
                  className={`
                    text-xs font-medium px-2 py-0.5 rounded mt-0.5
                    ${
                      item.priority === 'high'
                        ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400'
                        : item.priority === 'medium'
                          ? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400'
                          : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300'
                    }
                  `}
                >
                  {item.priority}
                </span>
                <div className="flex-1">
                  <p className="text-gray-700 dark:text-gray-300">{item.task}</p>
                  {(item.owner || item.deadline) && (
                    <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
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
          <p className="text-gray-500 dark:text-gray-400 text-sm">
            Click "Extract" to find action items from this meeting.
          </p>
        )}
      </Card>
    </div>
  );
}
