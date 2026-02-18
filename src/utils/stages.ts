/**
 * Shared processing stage label mapping
 * Used by RecordingView, TranscriptPanel, and BackgroundTaskPill
 */

export function processingStageLabel(stage: string): string {
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
