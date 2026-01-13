/**
 * Tauri IPC command wrappers
 *
 * This module provides typed wrappers around all Tauri commands,
 * making it easy to call backend functions from the frontend.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  // Meeting types
  Meeting,
  TranscriptSegment,
  Note,
  Summary,
  SummaryType,
  DatabaseStats,
  StorageStats,
  ActionItem,
  MeetingStatus,
  // Recording types
  RecordingResult,
  RecordingStateResponse,
  AudioDevices,
  PreprocessingInfo,
  WaveformUpdate,
  // Model types
  TranscriptionBackend,
  TranscriptionConfig,
  ModelStatusResponse,
  LlmModel,
  LlmStatus,
  LlmModelInfo,
  LlmDownloadProgress,
  EmbeddingInfo,
  EmbeddingDownloadProgress,
  EmbeddingTask,
  // Chat types
  SemanticSearchResult,
} from '../types';

// ============================================
// RECORDING COMMANDS
// ============================================

/** Start a new recording session */
export async function startRecording(): Promise<string> {
  return invoke<string>('start_recording');
}

/** Stop the current recording */
export async function stopRecording(): Promise<RecordingResult> {
  return invoke<RecordingResult>('stop_recording');
}

/** Get current recording state */
export async function getRecordingState(): Promise<RecordingStateResponse> {
  return invoke<RecordingStateResponse>('get_recording_state');
}

/** List available audio devices */
export async function listAudioDevices(): Promise<AudioDevices> {
  return invoke<AudioDevices>('list_audio_devices');
}

/** Preprocess a recorded meeting (VAD + optional denoising) */
export async function preprocessMeeting(
  meetingId: string,
  denoise?: boolean
): Promise<PreprocessingInfo> {
  return invoke<PreprocessingInfo>('preprocess_meeting', {
    meetingId,
    denoise,
  });
}

// ============================================
// MEETING COMMANDS
// ============================================

/** Create a new meeting */
export async function createMeeting(title?: string): Promise<Meeting> {
  return invoke<Meeting>('create_meeting', { title });
}

/** Create a meeting with a specific ID (used after recording) */
export async function createMeetingWithId(meeting: Meeting): Promise<Meeting> {
  return invoke<Meeting>('create_meeting_with_id', { meeting });
}

/** Get a meeting by ID */
export async function getMeeting(id: string): Promise<Meeting | null> {
  return invoke<Meeting | null>('get_meeting', { id });
}

/** List meetings with optional filters */
export async function listMeetings(options?: {
  status?: MeetingStatus;
  search?: string;
  limit?: number;
  offset?: number;
}): Promise<Meeting[]> {
  return invoke<Meeting[]>('list_meetings', {
    status: options?.status,
    search: options?.search,
    limit: options?.limit,
    offset: options?.offset,
  });
}

/** Update a meeting */
export async function updateMeeting(meeting: Meeting): Promise<void> {
  return invoke<void>('update_meeting', { meeting });
}

/** Update meeting status */
export async function updateMeetingStatus(
  id: string,
  status: MeetingStatus,
  errorMessage?: string
): Promise<void> {
  return invoke<void>('update_meeting_status', {
    id,
    status,
    errorMessage,
  });
}

/** Delete a meeting */
export async function deleteMeeting(id: string): Promise<boolean> {
  return invoke<boolean>('delete_meeting', { id });
}

/** Count meetings */
export async function countMeetings(status?: MeetingStatus): Promise<number> {
  return invoke<number>('count_meetings', { status });
}

// ============================================
// TRANSCRIPT COMMANDS
// ============================================

/** Get transcript for a meeting */
export async function getTranscript(
  meetingId: string
): Promise<TranscriptSegment[]> {
  return invoke<TranscriptSegment[]>('get_transcript', {
    meetingId,
  });
}

/** Save transcript segments for a meeting */
export async function saveTranscript(
  meetingId: string,
  segments: TranscriptSegment[]
): Promise<number> {
  return invoke<number>('save_transcript', {
    meetingId,
    segments,
  });
}

/** Get full transcript text */
export async function getTranscriptText(meetingId: string): Promise<string> {
  return invoke<string>('get_transcript_text', { meetingId });
}

/** Delete transcript for a meeting */
export async function deleteTranscript(meetingId: string): Promise<number> {
  return invoke<number>('delete_transcript', { meetingId });
}

// ============================================
// NOTES COMMANDS
// ============================================

/** Save or update a note for a meeting */
export async function saveNote(
  meetingId: string,
  content: string
): Promise<Note> {
  return invoke<Note>('save_note', {
    meetingId,
    content,
  });
}

/** Get all notes for a meeting */
export async function getNotes(meetingId: string): Promise<Note[]> {
  return invoke<Note[]>('get_notes', { meetingId });
}

/** Get the primary note for a meeting (most recent) */
export async function getNote(meetingId: string): Promise<Note | null> {
  return invoke<Note | null>('get_note', { meetingId });
}

// ============================================
// SUMMARY COMMANDS
// ============================================

/** Save or update a summary for a meeting */
export async function saveSummary(
  meetingId: string,
  summaryType: SummaryType,
  content: string,
  modelUsed?: string
): Promise<Summary> {
  return invoke<Summary>('save_summary', {
    meetingId,
    summaryType,
    content,
    modelUsed,
  });
}

/** Get all summaries for a meeting */
export async function getSummaries(meetingId: string): Promise<Summary[]> {
  return invoke<Summary[]>('get_summaries', { meetingId });
}

/** Get a specific summary by type */
export async function getSummary(
  meetingId: string,
  summaryType: SummaryType
): Promise<Summary | null> {
  return invoke<Summary | null>('get_summary', {
    meetingId,
    summaryType,
  });
}

// ============================================
// SEARCH COMMANDS
// ============================================

/** Full-text search result */
export interface SearchHit {
  meeting_id: string;
  segment_id: number;
  text: string;
  rank: number;
}

/** Search result with highlighted snippet */
export interface SearchHitWithSnippet extends SearchHit {
  snippet: string;
}

/** Search transcripts using full-text search */
export async function searchTranscripts(
  query: string,
  limit?: number
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>('search_transcripts', { query, limit });
}

/** Search transcripts with highlighted snippets */
export async function searchTranscriptsWithSnippets(
  query: string,
  limit?: number
): Promise<SearchHitWithSnippet[]> {
  return invoke<SearchHitWithSnippet[]>('search_transcripts_with_snippets', {
    query,
    limit,
  });
}

/** Search within a specific meeting */
export async function searchInMeeting(
  meetingId: string,
  query: string,
  limit?: number
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>('search_in_meeting', {
    meetingId,
    query,
    limit,
  });
}

// ============================================
// STATS COMMANDS
// ============================================

/** Get database statistics */
export async function getDatabaseStats(): Promise<DatabaseStats> {
  return invoke<DatabaseStats>('get_database_stats');
}

/** Get storage statistics (disk usage) */
export async function getStorageStats(): Promise<StorageStats> {
  return invoke<StorageStats>('get_storage_stats');
}

// ============================================
// TRANSCRIPTION COMMANDS
// ============================================

/** Get status of all transcription models */
export async function getModelStatus(): Promise<ModelStatusResponse> {
  return invoke<ModelStatusResponse>('get_model_status');
}

/** Download a transcription model */
export async function downloadTranscriptionModel(
  backend: TranscriptionBackend
): Promise<string> {
  return invoke<string>('download_transcription_model', { backend });
}

/** Delete a transcription model (auto-unloads if loaded) */
export async function deleteTranscriptionModel(
  backend: TranscriptionBackend
): Promise<void> {
  return invoke<void>('delete_transcription_model', { backend });
}

/** Initialize the transcription engine */
export async function initTranscription(
  backend: TranscriptionBackend
): Promise<void> {
  return invoke<void>('init_transcription', { backend });
}

/** Check if transcription is ready */
export async function isTranscriptionReady(): Promise<boolean> {
  return invoke<boolean>('is_transcription_ready');
}

/** Get current transcription configuration */
export async function getTranscriptionConfig(): Promise<TranscriptionConfig> {
  return invoke<TranscriptionConfig>('get_transcription_config');
}

/** Transcribe a single audio file */
export async function transcribeFile(
  audioPath: string
): Promise<TranscriptSegment[]> {
  return invoke<TranscriptSegment[]>('transcribe_file', {
    audioPath,
  });
}

/** Processing result from transcription */
export interface ProcessingResult {
  meeting_id: string;
  total_duration_ms: number;
  mic_segment_count: number;
  system_segment_count: number;
  transcript_text: string;
  backend: TranscriptionBackend;
  processing_time_ms: number;
}

/** Process a complete meeting (both mic and system audio) */
export async function processMeeting(
  meetingId: string,
  micPath?: string,
  systemPath?: string
): Promise<ProcessingResult> {
  // Tauri v2 expects camelCase keys, converts to snake_case for Rust
  return invoke<ProcessingResult>('process_meeting', {
    meetingId,
    micPath,
    systemPath,
  });
}

/** Unload transcription model */
export async function unloadTranscription(): Promise<void> {
  return invoke<void>('unload_transcription');
}

/** Get models directory path */
export async function getModelsDir(): Promise<string> {
  return invoke<string>('get_models_dir');
}

/** Check if a model is downloaded */
export async function isModelDownloaded(
  backend: TranscriptionBackend
): Promise<boolean> {
  return invoke<boolean>('is_model_downloaded', { backend });
}

// ============================================
// EMBEDDING COMMANDS
// ============================================

/** Initialize embedding service */
export async function initializeEmbedding(): Promise<boolean> {
  return invoke<boolean>('initialize_embedding');
}

/** Check if embedding model is ready */
export async function isEmbeddingReady(): Promise<boolean> {
  return invoke<boolean>('is_embedding_ready');
}

/** Check if embedding model is downloaded */
export async function isEmbeddingDownloaded(): Promise<boolean> {
  return invoke<boolean>('is_embedding_downloaded');
}

/** Generate embedding for text */
export async function embedText(
  text: string,
  task: EmbeddingTask
): Promise<number[]> {
  return invoke<number[]>('embed_text', { text, task });
}

/** Embedding processing result */
export interface EmbeddingProcessingResult {
  meeting_id: string;
  chunks_processed: number;
  embeddings_stored: number;
}

/** Process meeting transcript and store embeddings */
export async function embedMeetingTranscript(
  meetingId: string
): Promise<EmbeddingProcessingResult> {
  return invoke<EmbeddingProcessingResult>('embed_meeting_transcript', {
    meetingId,
  });
}

/** Calculate similarity between two embeddings */
export async function calculateSimilarity(
  embeddingA: number[],
  embeddingB: number[]
): Promise<number> {
  return invoke<number>('calculate_similarity', {
    embeddingA,
    embeddingB,
  });
}

/** Get embedding model info */
export async function getEmbeddingInfo(): Promise<EmbeddingInfo> {
  return invoke<EmbeddingInfo>('get_embedding_info');
}

/** Search embeddings by query text */
export async function semanticSearch(
  query: string,
  limit?: number,
  meetingId?: string
): Promise<SemanticSearchResult[]> {
  return invoke<SemanticSearchResult[]>('semantic_search', {
    query,
    limit,
    meetingId,
  });
}

/** Unload embedding model */
export async function unloadEmbedding(): Promise<boolean> {
  return invoke<boolean>('unload_embedding');
}

// ============================================
// LLM COMMANDS
// ============================================

/** Initialize LLM service (downloads model if needed) */
export async function initializeLlm(model?: LlmModel): Promise<boolean> {
  return invoke<boolean>('initialize_llm', { model });
}

/** Load an LLM model (must be downloaded first) */
export async function loadLlmModel(model: LlmModel): Promise<void> {
  return invoke<void>('load_llm_model', { model });
}

/** Unload the current LLM model */
export async function unloadLlmModel(): Promise<void> {
  return invoke<void>('unload_llm_model');
}

/** Get current LLM status */
export async function getLlmStatus(): Promise<LlmStatus> {
  return invoke<LlmStatus>('get_llm_status');
}

/** Check if an LLM model is downloaded */
export async function isLlmModelDownloaded(model: LlmModel): Promise<boolean> {
  return invoke<boolean>('is_llm_model_downloaded', { model });
}

/** Download an LLM model */
export async function downloadLlm(model: LlmModel): Promise<void> {
  return invoke<void>('download_llm', { model });
}

/** Delete an LLM model (auto-unloads if loaded) */
export async function deleteLlmModel(model: LlmModel): Promise<void> {
  return invoke<void>('delete_llm', { model });
}

/** List available LLM models with their status */
export async function listLlmModels(): Promise<LlmModelInfo[]> {
  return invoke<LlmModelInfo[]>('list_llm_models');
}

/** Generate a meeting summary */
export async function generateSummary(meetingId: string): Promise<string> {
  return invoke<string>('generate_summary', { meetingId });
}

/** Extract action items from a meeting */
export async function extractActionItems(
  meetingId: string
): Promise<ActionItem[]> {
  return invoke<ActionItem[]>('extract_action_items', {
    meetingId,
  });
}

/** Generate a meeting title */
export async function generateMeetingTitle(meetingId: string): Promise<string> {
  return invoke<string>('generate_meeting_title', { meetingId });
}

/** Answer a question about a meeting */
export async function askMeetingQuestion(
  meetingId: string,
  question: string
): Promise<string> {
  return invoke<string>('ask_meeting_question', {
    meetingId,
    question,
  });
}

/** Generate raw text (for testing/debugging) */
export async function generateText(
  prompt: string,
  maxTokens?: number
): Promise<string> {
  return invoke<string>('generate_text', {
    prompt,
    maxTokens,
  });
}

/** Get estimated token count for text */
export async function countTokens(text: string): Promise<number> {
  return invoke<number>('count_tokens', { text });
}

// ============================================
// APP COMMANDS
// ============================================

/** Application info */
export interface AppInfo {
  version: string;
  data_dir: string;
  platform: string;
}

/** Get application info */
export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('get_app_info');
}

/** Basic greeting command for testing IPC */
export async function greet(name: string): Promise<string> {
  return invoke<string>('greet', { name });
}

// ============================================
// EVENT LISTENERS
// ============================================

/** Download progress event */
export interface DownloadProgressEvent {
  model_id: string;
  stage: string;
  percent: number;
  message: string;
}

/** Embedding progress event */
export interface EmbeddingProgressEvent {
  meeting_id: string;
  stage: string;
  progress: number;
  chunks_processed: number;
  total_chunks: number;
}

/** Meeting processing progress event */
export interface MeetingProcessingProgressEvent {
  meeting_id: string;
  stage: string;
  percent: number;
  message: string;
}

/** Listen for waveform updates during recording */
export function onWaveformUpdate(
  callback: (data: WaveformUpdate) => void
): Promise<UnlistenFn> {
  return listen<WaveformUpdate>('waveform-update', (event) => {
    callback(event.payload);
  });
}

/** Listen for model download progress */
export function onModelDownloadProgress(
  callback: (data: DownloadProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<DownloadProgressEvent>('model-download-progress', (event) => {
    callback(event.payload);
  });
}

/** Listen for LLM download progress */
export function onLlmDownloadProgress(
  callback: (data: LlmDownloadProgress) => void
): Promise<UnlistenFn> {
  return listen<LlmDownloadProgress>('llm-download-progress', (event) => {
    callback(event.payload);
  });
}

/** Listen for embedding download progress */
export function onEmbeddingDownloadProgress(
  callback: (data: EmbeddingDownloadProgress) => void
): Promise<UnlistenFn> {
  return listen<EmbeddingDownloadProgress>(
    'embedding-download-progress',
    (event) => {
      callback(event.payload);
    }
  );
}

/** Listen for embedding processing progress */
export function onEmbeddingProgress(
  callback: (data: EmbeddingProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<EmbeddingProgressEvent>('embedding-progress', (event) => {
    callback(event.payload);
  });
}

/** Listen for meeting processing progress */
export function onMeetingProcessingProgress(
  callback: (data: MeetingProcessingProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<MeetingProcessingProgressEvent>(
    'meeting-processing-progress',
    (event) => {
      callback(event.payload);
    }
  );
}
