/**
 * Meeting detail view with tabs for transcript, summary, and notes
 */

import { useCallback, useState, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
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
    audioPlayerRef.current?.seekTo(ms);
    audioPlayerRef.current?.play();
  }, []);

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
    <div className="h-full flex flex-col">
      <MeetingHeader
        meeting={meeting}
        onBack={handleBack}
        onUpdateTitle={handleUpdateTitle}
      />

      {/* Audio Player */}
      <div className="px-6 py-3 border-b border-gray-200 dark:border-gray-700">
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
        <div className="px-6 py-2 border-b border-gray-200 dark:border-gray-700">
          <TabsList>
            <TabsTrigger value="transcript">Transcript</TabsTrigger>
            <TabsTrigger value="summary">Summary</TabsTrigger>
            <TabsTrigger value="notes">Notes</TabsTrigger>
          </TabsList>
        </div>

        <div className="flex-1 overflow-y-auto">
          <TabsContent value="transcript" className="h-full">
            <TranscriptPanel
              meetingId={meeting.id}
              segments={transcript}
              audioPathYou={meeting.audio_path_you}
              audioPathOthers={meeting.audio_path_others}
              isLoading={isLoadingTranscript}
              onTimestampClick={handleTimestampClick}
            />
          </TabsContent>

          <TabsContent value="summary" className="h-full">
            <SummaryPanel
              meetingId={meeting.id}
              hasTranscript={transcript.length > 0}
            />
          </TabsContent>

          <TabsContent value="notes" className="h-full">
            <NotesPanel meetingId={meeting.id} />
          </TabsContent>
        </div>
      </Tabs>
    </div>
  );
}
