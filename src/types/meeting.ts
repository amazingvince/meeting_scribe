/**
 * Meeting-related TypeScript types
 * Matches backend types from src-tauri/src/storage/models.rs
 */

/** Meeting status enum */
export type MeetingStatus =
  | 'recording'
  | 'processing'
  | 'ready'
  | 'archived'
  | 'error';

/** Summary type enum */
export type SummaryType = 'key_points' | 'action_items' | 'full';

/** Speaker identification */
export type Speaker = 'You' | 'Others' | 'Unknown';

/** Meeting entity */
export interface Meeting {
  /** Unique meeting ID (UUID) */
  id: string;
  /** Meeting title */
  title: string;
  /** Creation timestamp (Unix ms) */
  created_at: number;
  /** Last update timestamp (Unix ms) */
  updated_at: number;
  /** Total duration in milliseconds */
  duration_ms: number | null;
  /** Path to "you" audio file */
  audio_path_you: string | null;
  /** Path to "others" audio file */
  audio_path_others: string | null;
  /** Processing status */
  status: MeetingStatus;
  /** Error message if status is Error */
  error_message: string | null;
  /** Tags as JSON array */
  tags: string[];
}

/** Transcript segment for database storage */
export interface TranscriptSegment {
  /** Database row ID (auto-generated) */
  id: number | null;
  /** Meeting this segment belongs to */
  meeting_id: string;
  /** Start time in milliseconds */
  start_ms: number;
  /** End time in milliseconds */
  end_ms: number;
  /** Transcribed text */
  text: string;
  /** Speaker label */
  speaker: Speaker;
  /** Confidence score (0.0 - 1.0) */
  confidence: number | null;
  /** Reference to embedding in vector store */
  embedding_id: string | null;
}

/** User note attached to a meeting */
export interface Note {
  /** Database row ID */
  id: number | null;
  /** Meeting this note belongs to */
  meeting_id: string;
  /** Note content */
  content: string;
  /** Creation timestamp (Unix ms) */
  created_at: number;
  /** Last update timestamp (Unix ms) */
  updated_at: number;
  /** Reference to embedding in vector store */
  embedding_id: string | null;
}

/** Generated summary for a meeting */
export interface Summary {
  /** Database row ID */
  id: number | null;
  /** Meeting this summary belongs to */
  meeting_id: string;
  /** Type of summary */
  summary_type: SummaryType;
  /** Summary content */
  content: string;
  /** Model used to generate summary */
  model_used: string | null;
  /** Creation timestamp (Unix ms) */
  created_at: number;
  /** Reference to embedding in vector store */
  embedding_id: string | null;
}

/** Database statistics */
export interface DatabaseStats {
  /** Total number of meetings */
  meeting_count: number;
  /** Total number of transcript segments */
  segment_count: number;
  /** Total duration of all meetings in milliseconds */
  total_duration_ms: number;
  /** Total number of notes */
  note_count: number;
}

/** Storage usage statistics */
export interface StorageStats {
  /** Database file size in bytes */
  database_bytes: number;
  /** Vector store size in bytes */
  vectors_bytes: number;
  /** Audio files size in bytes */
  audio_bytes: number;
  /** ML models size in bytes */
  models_bytes: number;
  /** Total storage used */
  total_bytes: number;
}

/** Action item extracted from meeting */
export interface ActionItem {
  /** The task description */
  task: string;
  /** Person responsible (if identified) */
  owner: string | null;
  /** Deadline (if mentioned) */
  deadline: string | null;
  /** Priority level */
  priority: 'high' | 'medium' | 'low';
}

/** Text search result with snippets */
export interface SearchResult {
  /** Meeting ID */
  meeting_id: string;
  /** Matching text snippet */
  snippet: string;
  /** Start position in original text */
  start_ms: number | null;
}
