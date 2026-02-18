/**
 * Meeting detail view with tabs for transcript, summary, and notes
 */

import { useCallback, useEffect, useState, useRef } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router-dom';
import { FileQuestion } from 'lucide-react';
import { useMeeting } from '../../hooks';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '../ui/Tabs';
import { Spinner } from '../ui/Progress';
import { EmptyState } from '../ui/EmptyState';
import { MeetingHeader } from './MeetingHeader';
import { AudioPlayer, type AudioPlayerHandle } from './AudioPlayer';
import { TranscriptPanel } from './TranscriptPanel';
import { SummaryPanel } from './SummaryPanel';
import { NotesPanel } from './NotesPanel';

export function MeetingDetailView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const {
    meeting,
    transcript,
    isLoading,
    isLoadingTranscript,
    updateMeeting,
    error,
  } = useMeeting(id ?? null);

  const audioPlayerRef = useRef<AudioPlayerHandle>(null);
  const [activeTab, setActiveTab] = useState('transcript');
  const [focusTimestampMs, setFocusTimestampMs] = useState<number | null>(null);

  const handleBack = useCallback(() => {
    navigate('/library');
  }, [navigate]);

  const handleUpdateTitle = useCallback(
    async (title: string) => {
      if (!meeting) return;
      await updateMeeting({
        ...meeting,
        title,
        updated_at: Date.now(),
      });
    },
    [meeting, updateMeeting]
  );

  const handleTimestampClick = useCallback((ms: number) => {
    setFocusTimestampMs(ms);
    audioPlayerRef.current?.seekTo(ms);
    audioPlayerRef.current?.play();
  }, []);

  useEffect(() => {
    if (!meeting) {
      return;
    }

    const targetParam = searchParams.get('t');
    if (!targetParam) {
      return;
    }

    const parsedTargetMs = Number.parseInt(targetParam, 10);
    const nextParams = new URLSearchParams(searchParams);
    nextParams.delete('t');
    setSearchParams(nextParams, { replace: true });

    if (!Number.isFinite(parsedTargetMs) || parsedTargetMs < 0) {
      return;
    }

    const clampedTargetMs =
      meeting.duration_ms != null
        ? Math.min(parsedTargetMs, meeting.duration_ms)
        : parsedTargetMs;

    setActiveTab('transcript');
    setFocusTimestampMs(clampedTargetMs);

    // Delay slightly so the audio elements are mounted before seeking.
    const timer = window.setTimeout(() => {
      handleTimestampClick(clampedTargetMs);
    }, 120);

    return () => window.clearTimeout(timer);
  }, [
    meeting,
    meeting?.duration_ms,
    handleTimestampClick,
    searchParams,
    setSearchParams,
  ]);

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center">
        <Spinner size="lg" />
      </div>
    );
  }

  if (!meeting) {
    return (
      <div className="h-full flex items-center justify-center">
        <EmptyState
          icon={<FileQuestion className="w-12 h-12" />}
          title="Meeting not found"
          description={
            error ??
            "This meeting may have been deleted, or the link is invalid."
          }
          action={{ label: 'Back to Library', onClick: handleBack }}
        />
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 flex flex-col">
      <MeetingHeader
        meeting={meeting}
        onBack={handleBack}
        onUpdateTitle={handleUpdateTitle}
      />

      {/* Audio Player */}
      <div className="border-b border-border bg-card/50 px-4 py-3 md:px-6">
        <AudioPlayer
          ref={audioPlayerRef}
          micPath={meeting.audio_path_you}
          systemPath={meeting.audio_path_others}
          durationMs={meeting.duration_ms}
        />
      </div>

      <Tabs
        defaultValue="transcript"
        value={activeTab}
        onValueChange={setActiveTab}
        className="flex-1 flex flex-col min-h-0"
      >
        <div className="border-b border-border bg-card/30 px-4 py-2 md:px-6">
          <TabsList>
            <TabsTrigger value="transcript">Transcript</TabsTrigger>
            <TabsTrigger value="summary">Summary</TabsTrigger>
            <TabsTrigger value="notes">Notes</TabsTrigger>
          </TabsList>
        </div>

        <div className="flex-1 min-h-0">
          <TabsContent value="transcript" className="h-full min-h-0">
            <TranscriptPanel
              meetingId={meeting.id}
              meetingStatus={meeting.status}
              segments={transcript}
              audioPathYou={meeting.audio_path_you}
              audioPathOthers={meeting.audio_path_others}
              isLoading={isLoadingTranscript}
              onTimestampClick={handleTimestampClick}
              focusTimestampMs={focusTimestampMs}
            />
          </TabsContent>

          <TabsContent value="summary" className="h-full min-h-0">
            <SummaryPanel
              meetingId={meeting.id}
              hasTranscript={transcript.length > 0}
            />
          </TabsContent>

          <TabsContent value="notes" className="h-full min-h-0">
            <NotesPanel meetingId={meeting.id} />
          </TabsContent>
        </div>
      </Tabs>
    </div>
  );
}
