import { useEffect, useState, useCallback, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { Waveform } from "./Waveform";
import { formatDuration } from "../../utils/format";
import * as api from "../../lib/tauri";
import { useSettingsStore, useToastStore } from "../../stores";

type RecordingState = "Idle" | "Recording" | "Paused" | "Processing";

interface ProcessingProgress {
  meeting_id: string;
  stage: string;
  percent: number;
  message: string;
}

interface ChannelMetrics {
  rms: number;
  peak: number;
  samples: number[];
  speech_probability: number | null;
}

interface WaveformUpdate {
  timestamp_ms: number;
  mic: ChannelMetrics;
  system: ChannelMetrics;
  duration_ms: number;
}

interface RecordingStateResponse {
  state: RecordingState;
  meeting_id: string | null;
  duration_ms: number;
}

interface RecordingResult {
  meeting_id: string;
  duration_ms: number;
  mic_path: string | null;
  system_path: string | null;
}

const EMPTY_SAMPLES = new Array(64).fill(0);

export function RecordingView() {
  const navigate = useNavigate();
  const settings = useSettingsStore();
  const toast = useToastStore();

  const [recordingState, setRecordingState] = useState<RecordingState>("Idle");
  const [meetingId, setMeetingId] = useState<string | null>(null);
  const [durationMs, setDurationMs] = useState(0);
  const [micMetrics, setMicMetrics] = useState<ChannelMetrics>({
    rms: 0,
    peak: 0,
    samples: EMPTY_SAMPLES,
    speech_probability: null,
  });
  const [systemMetrics, setSystemMetrics] = useState<ChannelMetrics>({
    rms: 0,
    peak: 0,
    samples: EMPTY_SAMPLES,
    speech_probability: null,
  });
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [processingProgress, setProcessingProgress] = useState<ProcessingProgress | null>(null);

  // Track if we should abort processing navigation (user clicked skip)
  const skipProcessingRef = useRef(false);

  // Fetch initial state on mount
  useEffect(() => {
    const fetchState = async () => {
      try {
        const state = await invoke<RecordingStateResponse>("get_recording_state");
        setRecordingState(state.state);
        setMeetingId(state.meeting_id);
        setDurationMs(state.duration_ms);
      } catch (e) {
        console.error("Failed to get recording state:", e);
      }
    };
    fetchState();
  }, []);

  // Listen for waveform updates
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<WaveformUpdate>("waveform-update", (event) => {
        const { mic, system, duration_ms } = event.payload;
        setMicMetrics(mic);
        setSystemMetrics(system);
        setDurationMs(duration_ms);
      });
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, []);

  // Listen for processing progress events
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<ProcessingProgress>("meeting-processing-progress", (event) => {
        setProcessingProgress(event.payload);
      });
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, []);

  const handleStartRecording = useCallback(async () => {
    setError(null);
    setIsLoading(true);

    try {
      const id = await invoke<string>("start_recording");
      setMeetingId(id);
      setRecordingState("Recording");
      setDurationMs(0);
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      console.error("Failed to start recording:", e);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const handleStopRecording = useCallback(async () => {
    setError(null);
    setIsLoading(true);
    skipProcessingRef.current = false;

    try {
      const result = await invoke<RecordingResult>("stop_recording");

      // Reset recording UI state
      setMicMetrics({ rms: 0, peak: 0, samples: EMPTY_SAMPLES, speech_probability: null });
      setSystemMetrics({ rms: 0, peak: 0, samples: EMPTY_SAMPLES, speech_probability: null });

      // Keep meeting ID for processing
      setMeetingId(result.meeting_id);

      // Create meeting in database with the recording's ID
      const now = Date.now();
      await api.createMeetingWithId({
        id: result.meeting_id,
        title: `Meeting ${new Date().toLocaleString()}`,
        created_at: now,
        updated_at: now,
        duration_ms: result.duration_ms,
        audio_path_you: result.mic_path ?? null,
        audio_path_others: result.system_path ?? null,
        status: 'processing',
        error_message: null,
        tags: [],
      });

      // Check if transcription is ready, try to initialize if not
      let transcriptionReady = settings.transcriptionReady;
      if (!transcriptionReady) {
        // Try to initialize the transcription engine (will load model if downloaded)
        toast.info("Loading transcription model...");
        transcriptionReady = await settings.initializeTranscription();
      }

      if (!transcriptionReady) {
        toast.warning(
          "Transcription not available",
          "Download a transcription model in Settings to process recordings."
        );
        // Update meeting status to indicate it needs processing
        await api.updateMeetingStatus(result.meeting_id, "ready");
        setRecordingState("Idle");
        setMeetingId(null);
        setIsLoading(false);
        // Navigate to library to see the unprocessed meeting
        navigate("/library");
        return;
      }

      // Start automatic transcription
      setRecordingState("Processing");
      setIsLoading(false);

      try {
        const processingResult = await api.processMeeting(
          result.meeting_id,
          result.mic_path ?? undefined,
          result.system_path ?? undefined
        );

        // Update meeting status to ready
        await api.updateMeetingStatus(result.meeting_id, "ready");

        // Check if user skipped (still update status above)
        if (skipProcessingRef.current) {
          return;
        }

        toast.success(
          "Transcription complete",
          `Processed ${processingResult.mic_segment_count + processingResult.system_segment_count} segments`
        );

        // Navigate to the meeting detail view
        navigate(`/meeting/${result.meeting_id}`);
      } catch (procError) {
        const errorMsg = procError instanceof Error ? procError.message : String(procError);
        setError(`Transcription failed: ${errorMsg}`);
        toast.error("Transcription failed", errorMsg);
        // Update meeting status to error
        try {
          await api.updateMeetingStatus(result.meeting_id, "error", errorMsg);
        } catch {
          // Ignore status update errors
        }
      } finally {
        setRecordingState("Idle");
        setMeetingId(null);
        setProcessingProgress(null);
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      console.error("Failed to stop recording:", e);
      setRecordingState("Idle");
      setIsLoading(false);
    }
  }, [navigate, settings, toast]);

  const handleSkipProcessing = useCallback(() => {
    skipProcessingRef.current = true;
    navigate("/library");
  }, [navigate]);

  const isRecording = recordingState === "Recording";
  const isProcessing = recordingState === "Processing";

  // Get processing stage label
  const getStageLabel = (stage: string) => {
    switch (stage) {
      case "TranscribingMic":
        return "Transcribing mic audio...";
      case "TranscribingSystem":
        return "Transcribing system audio...";
      case "Merging":
        return "Merging transcripts...";
      case "Complete":
        return "Complete!";
      default:
        return "Processing...";
    }
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Recording</h1>

      {error && (
        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4 text-red-700 dark:text-red-300">
          {error}
        </div>
      )}

      <div className="card p-8">
        {/* Timer Display */}
        <div className="text-center mb-8">
          <div
            className={`text-5xl font-mono font-bold tabular-nums ${
              isRecording ? "text-red-500" : "text-gray-400 dark:text-gray-500"
            }`}
          >
            {formatDuration(durationMs)}
          </div>
          {isRecording && (
            <div className="flex items-center justify-center gap-2 mt-2 text-red-500">
              <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
              <span className="text-sm font-medium">Recording</span>
            </div>
          )}
          {isProcessing && (
            <div className="flex items-center justify-center gap-2 mt-2 text-indigo-500">
              <span className="w-5 h-5 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
              <span className="text-sm font-medium">
                {processingProgress ? getStageLabel(processingProgress.stage) : "Processing..."}
              </span>
            </div>
          )}
          {meetingId && (
            <div className="text-xs text-gray-400 dark:text-gray-500 mt-2 font-mono">
              {meetingId}
            </div>
          )}
        </div>

        {/* Waveforms */}
        <div className="space-y-4 mb-8">
          <Waveform
            samples={micMetrics.samples}
            rms={micMetrics.rms}
            color="#3b82f6"
            label="You (Microphone)"
            height={80}
          />
          <Waveform
            samples={systemMetrics.samples}
            rms={systemMetrics.rms}
            color="#10b981"
            label="Others (System Audio)"
            height={80}
          />
        </div>

        {/* Level Indicators */}
        <div className="grid grid-cols-2 gap-4 mb-8">
          <div className="text-center">
            <div className="text-xs text-gray-500 dark:text-gray-400 mb-1">Mic Level</div>
            <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 transition-all duration-75"
                style={{ width: `${Math.min(100, micMetrics.rms * 200)}%` }}
              />
            </div>
          </div>
          <div className="text-center">
            <div className="text-xs text-gray-500 dark:text-gray-400 mb-1">System Level</div>
            <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                className="h-full bg-emerald-500 transition-all duration-75"
                style={{ width: `${Math.min(100, systemMetrics.rms * 200)}%` }}
              />
            </div>
          </div>
        </div>

        {/* Control Buttons / Processing UI */}
        {isProcessing ? (
          <div className="space-y-4">
            {/* Progress Bar */}
            <div>
              <div className="flex justify-between text-sm text-gray-500 dark:text-gray-400 mb-2">
                <span>{processingProgress ? getStageLabel(processingProgress.stage) : "Processing..."}</span>
                <span>{processingProgress ? `${processingProgress.percent}%` : "0%"}</span>
              </div>
              <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-indigo-500 transition-all duration-300"
                  style={{ width: `${processingProgress?.percent ?? 0}%` }}
                />
              </div>
            </div>

            {/* Skip Button */}
            <div className="flex justify-center">
              <button
                onClick={handleSkipProcessing}
                className="btn btn-secondary text-sm px-4 py-2"
              >
                Skip to Library
              </button>
            </div>
          </div>
        ) : (
          <div className="flex justify-center">
            {!isRecording ? (
              <button
                onClick={handleStartRecording}
                disabled={isLoading}
                className="btn btn-primary text-lg px-8 py-3 flex items-center gap-2"
              >
                {isLoading ? (
                  <>
                    <span className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Starting...
                  </>
                ) : (
                  <>
                    <svg
                      className="w-5 h-5"
                      fill="currentColor"
                      viewBox="0 0 20 20"
                    >
                      <circle cx="10" cy="10" r="6" />
                    </svg>
                    Start Recording
                  </>
                )}
              </button>
            ) : (
              <button
                onClick={handleStopRecording}
                disabled={isLoading}
                className="btn bg-red-500 hover:bg-red-600 text-white text-lg px-8 py-3 flex items-center gap-2"
              >
                {isLoading ? (
                  <>
                    <span className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Stopping...
                  </>
                ) : (
                  <>
                    <svg
                      className="w-5 h-5"
                      fill="currentColor"
                      viewBox="0 0 20 20"
                    >
                      <rect x="5" y="5" width="10" height="10" rx="1" />
                    </svg>
                    Stop Recording
                  </>
                )}
              </button>
            )}
          </div>
        )}
      </div>

      <p className="text-sm text-gray-500 text-center">
        Audio is saved locally to your device. System audio capture is currently Windows-only.
      </p>
    </div>
  );
}
