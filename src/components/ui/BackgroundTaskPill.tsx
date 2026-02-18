import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Cpu, Loader2, Sparkles, Wand2 } from 'lucide-react';
import { useTauriEvent } from '../../hooks';
import { useMeetingsStore } from '../../stores';
import type {
  BatchEmbedProgress,
  DownloadProgressEvent,
  MeetingProcessingProgressEvent,
  SummaryGenerationFinishedEvent,
  SummaryGenerationProgressEvent,
} from '../../lib/tauri';
import type {
  EmbeddingDownloadProgress,
  LlmDownloadProgress,
  MeetingProcessingFinishedEvent,
} from '../../types';

const TRANSCRIPTION_MODEL_LABELS: Record<string, string> = {
  'parakeet-tdt-0.6b-v3-int8': 'Parakeet model',
  'whisper-medium-q4_1': 'Whisper model',
  'moonshine-tiny': 'Moonshine model',
};

interface TimedMeetingTask {
  meetingId: string;
  stage: string;
  percent: number;
  message: string;
  updatedAt: number;
}

interface TimedDownloadTask {
  label: string;
  percent: number;
  message: string;
  updatedAt: number;
}

interface TimedBatchEmbedTask {
  current: number;
  total: number;
  currentMeeting: string;
  updatedAt: number;
}

interface TimedSummaryTask {
  meetingId: string;
  summaryType: 'full' | 'action_items';
  stage: string;
  percent: number;
  message: string;
  updatedAt: number;
}

function meetingStageLabel(stage: string): string {
  switch (stage) {
    case 'TranscribingMic':
      return 'Transcribing microphone audio';
    case 'TranscribingSystem':
      return 'Transcribing system audio';
    case 'Merging':
      return 'Merging transcript channels';
    case 'Complete':
      return 'Transcript processing complete';
    case 'Failed':
      return 'Transcript processing failed';
    default:
      return 'Processing transcript';
  }
}

export function BackgroundTaskPill() {
  const navigate = useNavigate();
  const fetchMeetings = useMeetingsStore((state) => state.fetchMeetings);
  const [meetingTask, setMeetingTask] = useState<TimedMeetingTask | null>(null);
  const [summaryTask, setSummaryTask] = useState<TimedSummaryTask | null>(null);
  const [downloadTask, setDownloadTask] = useState<TimedDownloadTask | null>(null);
  const [batchEmbedTask, setBatchEmbedTask] = useState<TimedBatchEmbedTask | null>(null);

  const clearMeetingTimerRef = useRef<number | null>(null);
  const clearSummaryTimerRef = useRef<number | null>(null);
  const clearDownloadTimerRef = useRef<number | null>(null);

  const scheduleMeetingClear = (delayMs: number) => {
    if (clearMeetingTimerRef.current !== null) {
      window.clearTimeout(clearMeetingTimerRef.current);
    }
    clearMeetingTimerRef.current = window.setTimeout(() => {
      setMeetingTask(null);
      clearMeetingTimerRef.current = null;
    }, delayMs);
  };

  const scheduleDownloadClear = (delayMs: number) => {
    if (clearDownloadTimerRef.current !== null) {
      window.clearTimeout(clearDownloadTimerRef.current);
    }
    clearDownloadTimerRef.current = window.setTimeout(() => {
      setDownloadTask(null);
      clearDownloadTimerRef.current = null;
    }, delayMs);
  };

  const scheduleSummaryClear = (delayMs: number) => {
    if (clearSummaryTimerRef.current !== null) {
      window.clearTimeout(clearSummaryTimerRef.current);
    }
    clearSummaryTimerRef.current = window.setTimeout(() => {
      setSummaryTask(null);
      clearSummaryTimerRef.current = null;
    }, delayMs);
  };

  useEffect(() => {
    return () => {
      if (clearMeetingTimerRef.current !== null) {
        window.clearTimeout(clearMeetingTimerRef.current);
      }
      if (clearSummaryTimerRef.current !== null) {
        window.clearTimeout(clearSummaryTimerRef.current);
      }
      if (clearDownloadTimerRef.current !== null) {
        window.clearTimeout(clearDownloadTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const staleCheckId = window.setInterval(() => {
      const now = Date.now();
      setMeetingTask((current) =>
        current && now - current.updatedAt > 30000 ? null : current
      );
      setSummaryTask((current) =>
        current && now - current.updatedAt > 30000 ? null : current
      );
      setDownloadTask((current) =>
        current && now - current.updatedAt > 30000 ? null : current
      );
      setBatchEmbedTask((current) =>
        current && now - current.updatedAt > 30000 ? null : current
      );
    }, 5000);

    return () => {
      window.clearInterval(staleCheckId);
    };
  }, []);

  useTauriEvent<MeetingProcessingProgressEvent>(
    'meeting-processing-progress',
    (event) => {
      const updated: TimedMeetingTask = {
        meetingId: event.meeting_id,
        stage: event.stage,
        percent: event.percent,
        message: event.message,
        updatedAt: Date.now(),
      };
      setMeetingTask(updated);

      if (event.percent >= 100 || event.stage === 'Complete') {
        scheduleMeetingClear(1800);
      } else if (clearMeetingTimerRef.current !== null) {
        window.clearTimeout(clearMeetingTimerRef.current);
        clearMeetingTimerRef.current = null;
      }
    }
  );

  useTauriEvent<SummaryGenerationProgressEvent>(
    'summary-generation-progress',
    (event) => {
      const summaryType = event.summary_type === 'action_items' ? 'action_items' : 'full';
      setSummaryTask({
        meetingId: event.meeting_id,
        summaryType,
        stage: event.stage,
        percent: event.percent,
        message: event.message,
        updatedAt: Date.now(),
      });

      if (event.percent >= 100 || event.stage === 'Complete') {
        scheduleSummaryClear(1800);
      } else if (clearSummaryTimerRef.current !== null) {
        window.clearTimeout(clearSummaryTimerRef.current);
        clearSummaryTimerRef.current = null;
      }
    }
  );

  useTauriEvent<SummaryGenerationFinishedEvent>(
    'summary-generation-finished',
    (event) => {
      const summaryType = event.summary_type === 'action_items' ? 'action_items' : 'full';
      setSummaryTask({
        meetingId: event.meeting_id,
        summaryType,
        stage: event.success ? 'Complete' : 'Failed',
        percent: 100,
        message: event.success
          ? summaryType === 'action_items'
            ? 'Action items ready'
            : 'Summary ready'
          : event.error_message || 'Summary generation failed',
        updatedAt: Date.now(),
      });
      scheduleSummaryClear(event.success ? 1800 : 4500);
    }
  );

  useTauriEvent<MeetingProcessingFinishedEvent>(
    'meeting-processing-finished',
    (event) => {
      const message = event.success
        ? 'Transcript processing complete'
        : event.error_message || 'Transcript processing failed';

      setMeetingTask({
        meetingId: event.meeting_id,
        stage: event.success ? 'Complete' : 'Failed',
        percent: 100,
        message,
        updatedAt: Date.now(),
      });
      scheduleMeetingClear(event.success ? 1800 : 4500);
      void fetchMeetings();
    }
  );

  useTauriEvent<DownloadProgressEvent>('model-download-progress', (event) => {
    const label = TRANSCRIPTION_MODEL_LABELS[event.model_id] ?? 'Transcription model';
    setDownloadTask({
      label,
      percent: event.percent,
      message: event.message,
      updatedAt: Date.now(),
    });

    if (event.percent >= 100) {
      scheduleDownloadClear(1500);
    } else if (clearDownloadTimerRef.current !== null) {
      window.clearTimeout(clearDownloadTimerRef.current);
      clearDownloadTimerRef.current = null;
    }
  });

  useTauriEvent<LlmDownloadProgress>('llm-download-progress', (event) => {
    setDownloadTask({
      label: event.model,
      percent: event.percent,
      message: 'Downloading language model',
      updatedAt: Date.now(),
    });

    if (event.percent >= 100) {
      scheduleDownloadClear(1500);
    } else if (clearDownloadTimerRef.current !== null) {
      window.clearTimeout(clearDownloadTimerRef.current);
      clearDownloadTimerRef.current = null;
    }
  });

  useTauriEvent<EmbeddingDownloadProgress>('embedding-download-progress', (event) => {
    setDownloadTask({
      label: 'Embedding model',
      percent: event.percent,
      message: `Downloading ${event.file}`,
      updatedAt: Date.now(),
    });

    if (event.percent >= 100 || event.status === 'complete') {
      scheduleDownloadClear(1500);
    } else if (clearDownloadTimerRef.current !== null) {
      window.clearTimeout(clearDownloadTimerRef.current);
      clearDownloadTimerRef.current = null;
    }
  });

  useTauriEvent<BatchEmbedProgress>('batch-embed-progress', (event) => {
    if (event.status === 'complete') {
      setBatchEmbedTask(null);
      return;
    }

    setBatchEmbedTask({
      current: event.current,
      total: event.total,
      currentMeeting: event.current_meeting,
      updatedAt: Date.now(),
    });
  });

  const activeTask = useMemo(() => {
    if (meetingTask) {
      return {
        kind: 'meeting' as const,
        title: 'Transcript Processing',
        subtitle: meetingTask.message || meetingStageLabel(meetingTask.stage),
        percent: Math.round(meetingTask.percent),
        action: () => navigate(`/meeting/${meetingTask.meetingId}`),
        icon: <Wand2 className="w-4 h-4" />,
      };
    }

    if (summaryTask) {
      return {
        kind: 'summary' as const,
        title: summaryTask.summaryType === 'action_items' ? 'Action Items' : 'Summary',
        subtitle: summaryTask.message,
        percent: Math.round(summaryTask.percent),
        action: () => navigate(`/meeting/${summaryTask.meetingId}`),
        icon: <Sparkles className="w-4 h-4" />,
      };
    }

    if (downloadTask) {
      return {
        kind: 'download' as const,
        title: 'Model Download',
        subtitle: `${downloadTask.label}: ${downloadTask.message}`,
        percent: Math.round(downloadTask.percent),
        action: () => navigate('/settings'),
        icon: <Cpu className="w-4 h-4" />,
      };
    }

    if (batchEmbedTask) {
      const percent =
        batchEmbedTask.total > 0
          ? Math.round(
              ((batchEmbedTask.current + 1) / batchEmbedTask.total) * 100
            )
          : 0;

      return {
        kind: 'embedding' as const,
        title: 'Indexing Meetings',
        subtitle: `Meeting ${Math.min(batchEmbedTask.current + 1, batchEmbedTask.total)} of ${batchEmbedTask.total}`,
        percent,
        action: () => navigate('/settings'),
        icon: <Sparkles className="w-4 h-4" />,
      };
    }

    return null;
  }, [batchEmbedTask, downloadTask, meetingTask, navigate, summaryTask]);

  if (!activeTask) {
    return null;
  }

  return (
    <div className="fixed top-4 right-4 z-40 pointer-events-none">
      <button
        type="button"
        onClick={activeTask.action}
        className="pointer-events-auto w-[260px] rounded-xl border border-border bg-card/95 shadow-lg backdrop-blur px-3 py-2 text-left"
      >
        <div className="flex items-center gap-2 text-primary">
          <Loader2 className="w-4 h-4 animate-spin" />
          {activeTask.icon}
          <span className="text-xs font-semibold tracking-wide uppercase">
            {activeTask.title}
          </span>
          <span className="ml-auto text-xs font-semibold">
            {activeTask.percent}%
          </span>
        </div>
        <p className="mt-1 text-xs text-muted-foreground truncate">
          {activeTask.subtitle}
        </p>
        <div className="mt-2 h-1.5 bg-muted rounded-full overflow-hidden">
          <div
            className="h-full bg-primary rounded-full transition-all duration-300"
            style={{ width: `${activeTask.percent}%` }}
          />
        </div>
      </button>
    </div>
  );
}
