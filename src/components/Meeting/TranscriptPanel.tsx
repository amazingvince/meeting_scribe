/**
 * Transcript panel with scrollable transcript segments
 */

import { useMemo, useState, useCallback } from 'react';
import { RefreshCw } from 'lucide-react';
import type { TranscriptSegment } from '../../types';
import { NoTranscriptEmpty } from '../ui/EmptyState';
import { SkeletonText } from '../ui/Skeleton';
import { Button } from '../ui/Button';
import { Modal, ModalFooter } from '../ui/Modal';
import { formatDuration } from '../../utils/format';
import { useMeetingsStore, useSettingsStore, useToastStore } from '../../stores';
import * as api from '../../lib/tauri';

interface TranscriptPanelProps {
  meetingId: string;
  segments: TranscriptSegment[];
  audioPathYou?: string | null;
  audioPathOthers?: string | null;
  isLoading: boolean;
  onTimestampClick?: (ms: number) => void;
}

export function TranscriptPanel({
  meetingId,
  segments,
  audioPathYou,
  audioPathOthers,
  isLoading,
  onTimestampClick,
}: TranscriptPanelProps) {
  const [isProcessing, setIsProcessing] = useState(false);
  const [showLoadModal, setShowLoadModal] = useState(false);
  const [isLoadingModel, setIsLoadingModel] = useState(false);
  const toast = useToastStore();
  const settings = useSettingsStore();
  const { fetchTranscript } = useMeetingsStore();

  const hasAudio = audioPathYou || audioPathOthers;

  // Actually perform the transcription
  const doTranscription = useCallback(async () => {
    setIsProcessing(true);
    try {
      await api.processMeeting(
        meetingId,
        audioPathYou ?? undefined,
        audioPathOthers ?? undefined
      );
      await fetchTranscript(meetingId);
      toast.success('Transcript generated successfully');
    } catch (e) {
      toast.error(
        'Failed to generate transcript',
        e instanceof Error ? e.message : String(e)
      );
    } finally {
      setIsProcessing(false);
    }
  }, [meetingId, audioPathYou, audioPathOthers, fetchTranscript, toast]);

  // Handle clicking the generate/regenerate button
  const handleGenerateTranscript = useCallback(async () => {
    if (!settings.transcriptionReady) {
      // Show modal to ask user if they want to load the model
      setShowLoadModal(true);
      return;
    }
    await doTranscription();
  }, [settings.transcriptionReady, doTranscription]);

  // Handle loading the model from the modal
  const handleLoadModel = useCallback(async () => {
    setIsLoadingModel(true);
    try {
      const success = await settings.initializeTranscription();
      if (success) {
        setShowLoadModal(false);
        // Proceed with transcription after model loads
        await doTranscription();
      }
    } catch (e) {
      toast.error(
        'Failed to load model',
        e instanceof Error ? e.message : String(e)
      );
    } finally {
      setIsLoadingModel(false);
    }
  }, [settings, doTranscription, toast]);
  // Group consecutive segments by speaker
  const groupedSegments = useMemo(() => {
    if (segments.length === 0) return [];

    const groups: {
      speaker: string;
      startMs: number;
      segments: TranscriptSegment[];
    }[] = [];

    let currentGroup: (typeof groups)[0] | null = null;

    for (const segment of segments) {
      if (!currentGroup || currentGroup.speaker !== segment.speaker) {
        if (currentGroup) {
          groups.push(currentGroup);
        }
        currentGroup = {
          speaker: segment.speaker,
          startMs: segment.start_ms,
          segments: [segment],
        };
      } else {
        currentGroup.segments.push(segment);
      }
    }

    if (currentGroup) {
      groups.push(currentGroup);
    }

    return groups;
  }, [segments]);

  if (isLoading) {
    return (
      <div className="p-4 space-y-6">
        <SkeletonText lines={4} />
        <SkeletonText lines={3} />
        <SkeletonText lines={5} />
      </div>
    );
  }

  if (segments.length === 0) {
    return (
      <div className="h-full flex flex-col">
        {/* Header with generate button even when no transcript */}
        {hasAudio && (
          <div className="flex items-center justify-end px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <Button
              variant="primary"
              size="sm"
              onClick={handleGenerateTranscript}
              isLoading={isProcessing}
            >
              Generate Transcript
            </Button>
          </div>
        )}
        <div className="flex-1">
          <NoTranscriptEmpty />
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header with regenerate button */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <span className="text-sm text-gray-500 dark:text-gray-400">
          {segments.length} segment{segments.length !== 1 ? 's' : ''}
        </span>
        {hasAudio && (
          <Button
            variant="secondary"
            size="sm"
            onClick={handleGenerateTranscript}
            isLoading={isProcessing}
          >
            <RefreshCw className="w-4 h-4 mr-1" />
            Regenerate
          </Button>
        )}
      </div>

      {/* Transcript content */}
      <div className="flex-1 p-4 space-y-6 overflow-y-auto">
      {groupedSegments.map((group, idx) => (
        <div key={idx} className="space-y-2">
          <div className="flex items-center gap-3">
            <span
              className={`
                text-sm font-medium px-2 py-0.5 rounded
                ${
                  group.speaker === 'You'
                    ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-400'
                    : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300'
                }
              `}
            >
              {group.speaker}
            </span>
            <button
              className="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 font-mono"
              onClick={() => onTimestampClick?.(group.startMs)}
            >
              {formatDuration(group.startMs)}
            </button>
          </div>
          <div className="text-gray-700 dark:text-gray-300 leading-relaxed">
            {group.segments.map((segment, segIdx) => (
              <span
                key={segIdx}
                className="hover:bg-yellow-100 dark:hover:bg-yellow-900/20 cursor-pointer rounded px-0.5"
                onClick={() => onTimestampClick?.(segment.start_ms)}
              >
                {segment.text}{' '}
              </span>
            ))}
          </div>
        </div>
      ))}
      </div>

      {/* Model loading modal */}
      <Modal
        isOpen={showLoadModal}
        onClose={() => setShowLoadModal(false)}
        title="Load Transcription Model"
        size="sm"
      >
        <p className="text-gray-600 dark:text-gray-400">
          The transcription model is not loaded. Would you like to load it now?
        </p>
        <p className="text-sm text-gray-500 dark:text-gray-500 mt-2">
          This may take a moment depending on your hardware.
        </p>
        <ModalFooter>
          <Button variant="secondary" onClick={() => setShowLoadModal(false)}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handleLoadModel}
            isLoading={isLoadingModel}
          >
            Load Model
          </Button>
        </ModalFooter>
      </Modal>
    </div>
  );
}
