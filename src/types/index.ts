/**
 * TypeScript types index
 * Re-exports all types for easy importing
 */

// Meeting types
export type {
  MeetingStatus,
  SummaryType,
  Speaker,
  Meeting,
  TranscriptSegment,
  Note,
  Summary,
  DatabaseStats,
  StorageStats,
  ActionItem,
  SearchResult,
} from './meeting';

// Recording types
export type {
  RecordingState,
  AudioChannel,
  RecordingResult,
  RecordingStateResponse,
  RecordingStateChangedEvent,
  AudioDevices,
  MacSystemAudioBackend,
  EchoCancellationBackend,
  MacSystemAudioSettings,
  StartRecordingOptions,
  ProcessMeetingOptions,
  LivePreviewOptions,
  LiveTranscriptSegment,
  LiveTranscriptPreview,
  MeetingProcessingFinishedEvent,
  SpeechSegment,
  PreprocessingInfo,
  ChannelMetrics,
  WaveformUpdate,
} from './recording';

// Model types
export type {
  ModelType,
  TranscriptionBackend,
  EmbeddingModel,
  LlmModel,
  ModelStatus,
  ArchiveFormat,
  ModelInfo,
  ModelStatusItem,
  ModelStatusResponse,
  DownloadProgress,
  DownloadStage,
  TranscriptionConfig,
  LlmStatus,
  LlmModelInfo,
  LlmDownloadProgress,
  EmbeddingInfo,
  EmbeddingDownloadProgress,
  EmbeddingTask,
} from './models';

// Chat types
export type {
  ChatRole,
  ChatMessage,
  ChatSource,
  ChatSession,
  SemanticSearchResult,
  ChatInputState,
  ChatSuggestion,
} from './chat';

export { defaultChatSuggestions } from './chat';
