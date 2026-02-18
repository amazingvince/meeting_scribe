/**
 * Transcript panel with scrollable transcript segments
 */

import { useMemo, useState, useCallback, useEffect } from 'react';
import { Loader2, RefreshCw } from 'lucide-react';
import type { MeetingStatus, TranscriptSegment } from '../../types';
import { NoTranscriptEmpty } from '../ui/EmptyState';
import { SkeletonText } from '../ui/Skeleton';
import { Button } from '../ui/Button';
import { Modal, ModalFooter } from '../ui/Modal';
import { ProgressBar } from '../ui/Progress';
import { formatDuration } from '../../utils/format';
import { useTauriEvent } from '../../hooks';
import { useMeetingsStore, useSettingsStore, useToastStore } from '../../stores';
import type { MeetingProcessingProgressEvent } from '../../lib/tauri';
import * as api from '../../lib/tauri';

interface TranscriptPanelProps {
  meetingId: string;
  meetingStatus: MeetingStatus;
  segments: TranscriptSegment[];
  audioPathYou?: string | null;
  audioPathOthers?: string | null;
  isLoading: boolean;
  onTimestampClick?: (ms: number) => void;
}

function getProcessingStageLabel(stage: string): string {
  switch (stage) {
    case 'TranscribingMic':
      return 'Transcribing microphone audio...';
    case 'TranscribingSystem':
      return 'Transcribing system audio...';
    case 'Merging':
      return 'Merging transcript channels...';
    case 'Complete':
      return 'Transcript processing complete.';
    default:
      return 'Processing transcript...';
  }
}

export function TranscriptPanel({
  meetingId,
  meetingStatus,
  segments,
  audioPathYou,
  audioPathOthers,
  isLoading,
  onTimestampClick,
}: TranscriptPanelProps) {
  const [isProcessing, setIsProcessing] = useState(false);
  const [showLoadModal, setShowLoadModal] = useState(false);
  const [isLoadingModel, setIsLoadingModel] = useState(false);
  const [processingProgress, setProcessingProgress] =
    useState<MeetingProcessingProgressEvent | null>(null);
  const toast = useToastStore();
  const settings = useSettingsStore();
  const { fetchMeeting, fetchTranscript } = useMeetingsStore();

  const hasAudio = Boolean(audioPathYou || audioPathOthers);

  useEffect(() => {
    setProcessingProgress(null);
  }, [meetingId]);

  useTauriEvent<MeetingProcessingProgressEvent>(
    'meeting-processing-progress',
    (data) => {
      if (data.meeting_id !== meetingId) return;
      setProcessingProgress(data);
    }
  );

  // Actually perform the transcription
  const doTranscription = useCallback(async () => {
    setIsProcessing(true);
    setProcessingProgress({
      meeting_id: meetingId,
      stage: 'TranscribingMic',
      percent: 5,
      message: 'Starting transcription...',
    });

    try {
      await api.processMeeting(
        meetingId,
        audioPathYou ?? undefined,
        audioPathOthers ?? undefined,
        { echoBackend: settings.echoCancellationBackend }
      );
      await api.updateMeetingStatus(meetingId, 'ready');
      await fetchMeeting(meetingId);
      await fetchTranscript(meetingId);
      toast.success('Transcript generated successfully');
    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      void api.updateMeetingStatus(meetingId, 'error', errorMessage);
      toast.error('Failed to generate transcript', errorMessage);
    } finally {
      setIsProcessing(false);
      setProcessingProgress(null);
    }
  }, [
    meetingId,
    audioPathYou,
    audioPathOthers,
    fetchMeeting,
    fetchTranscript,
    settings.echoCancellationBackend,
    toast,
  ]);

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

  const hasLiveProcessingProgress = processingProgress !== null;
  const isActivelyProcessing = isProcessing || hasLiveProcessingProgress;
  const showProcessingStatus = isActivelyProcessing || meetingStatus === 'processing';
  const statusLabel = processingProgress
    ? getProcessingStageLabel(processingProgress.stage)
    : 'Processing transcript...';
  const statusMessage =
    processingProgress?.message ??
    'This can take a moment depending on audio length and model speed.';

  const processingStatusBlock = showProcessingStatus ? (
    <div className="border-b border-border bg-muted/50 px-4 py-3">
      <div className="flex items-center gap-2 text-sm text-foreground">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="font-medium">{statusLabel}</span>
      </div>
      <p className="mt-1 text-xs text-muted-foreground">
        {statusMessage}
      </p>
      {processingProgress ? (
        <div className="mt-2">
          <ProgressBar
            value={processingProgress.percent}
            size="sm"
            color="blue"
            showLabel
          />
        </div>
      ) : (
        <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
          <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
        </div>
      )}
    </div>
  ) : null;

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
          <div className="flex items-center justify-end border-b border-border px-4 py-3">
            <Button
              size="sm"
              onClick={handleGenerateTranscript}
              isLoading={isProcessing}
              disabled={isActivelyProcessing}
            >
              {isActivelyProcessing ? 'Processing...' : 'Generate Transcript'}
            </Button>
          </div>
        )}
        {processingStatusBlock}
        <div className="flex-1">
          <NoTranscriptEmpty />
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header with regenerate button */}
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <span className="text-sm text-muted-foreground">
          {segments.length} segment{segments.length !== 1 ? 's' : ''}
        </span>
        {hasAudio && (
          <Button
            variant="secondary"
            size="sm"
            onClick={handleGenerateTranscript}
            isLoading={isProcessing}
            disabled={isActivelyProcessing}
          >
            <RefreshCw className="w-4 h-4 mr-1" />
            {isActivelyProcessing ? 'Processing...' : 'Regenerate'}
          </Button>
        )}
      </div>

      {processingStatusBlock}

      {/* Transcript content */}
      <div className="no-scrollbar flex-1 space-y-5 overflow-y-auto p-4 md:p-5">
        {groupedSegments.map((group, idx) => (
          <div key={idx} className="space-y-2">
            <div className="flex items-center gap-3">
              <span
                className={`
                  text-sm font-medium px-2 py-0.5 rounded
                  ${
                    group.speaker === 'You'
                      ? 'bg-primary/10 text-primary'
                      : 'bg-muted text-muted-foreground'
                  }
                `}
              >
                {group.speaker}
              </span>
              <button
                className="font-mono text-xs text-muted-foreground hover:text-foreground"
                onClick={() => onTimestampClick?.(group.startMs)}
              >
                {formatDuration(group.startMs)}
              </button>
            </div>
            <div className="leading-relaxed text-foreground/80">
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
        <p className="text-muted-foreground">
          The transcription model is not loaded. Would you like to load it now?
        </p>
        <p className="text-sm text-muted-foreground mt-2">
          This may take a moment depending on your hardware.
        </p>
        <ModalFooter>
          <Button variant="secondary" onClick={() => setShowLoadModal(false)}>
            Cancel
          </Button>
          <Button
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
