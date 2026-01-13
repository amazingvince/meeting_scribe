/**
 * Recording-related TypeScript types
 * Matches backend types from src-tauri/src/audio/ and commands/recording.rs
 */

/** Recording state */
export type RecordingState = 'Idle' | 'Recording' | 'Paused' | 'Processing';

/** Audio channel identifier */
export type AudioChannel = 'Mic' | 'System';

/** Recording result returned when stopping */
export interface RecordingResult {
  meeting_id: string;
  duration_ms: number;
  mic_path: string | null;
  system_path: string | null;
}

/** Recording state response */
export interface RecordingStateResponse {
  state: RecordingState;
  meeting_id: string | null;
  duration_ms: number;
}

/** Available audio devices */
export interface AudioDevices {
  input_devices: string[];
  output_devices: string[];
}

/** Speech segment from VAD */
export interface SpeechSegment {
  start_ms: number;
  end_ms: number;
  confidence: number;
}

/** Preprocessing info after VAD */
export interface PreprocessingInfo {
  meeting_id: string;
  mic_segments: SpeechSegment[];
  system_segments: SpeechSegment[];
  mic_speech_ratio: number;
  system_speech_ratio: number;
  mic_duration_ms: number;
  system_duration_ms: number;
}

/** Waveform metrics for a single channel */
export interface ChannelMetrics {
  /** Root mean square (0.0 - 1.0) */
  rms: number;
  /** Peak amplitude (0.0 - 1.0) */
  peak: number;
  /** Downsampled waveform points for rendering */
  samples: number[];
  /** VAD speech probability (if available) */
  speech_probability: number | null;
}

/** Combined waveform update for both channels */
export interface WaveformUpdate {
  /** Timestamp in milliseconds since recording start */
  timestamp_ms: number;
  /** Mic channel metrics */
  mic: ChannelMetrics;
  /** System audio channel metrics */
  system: ChannelMetrics;
  /** Recording duration in milliseconds */
  duration_ms: number;
}
