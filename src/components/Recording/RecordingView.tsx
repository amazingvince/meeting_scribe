import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { FileText, NotebookPen } from "lucide-react";
import { Waveform } from "./Waveform";
import { formatDuration } from "../../utils/format";
import * as api from "../../lib/tauri";
import { useSettingsStore, useToastStore } from "../../stores";
import type {
  LiveTranscriptSegment,
  MeetingProcessingFinishedEvent,
} from "../../types";

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
const LIVE_PREVIEW_STABILIZATION_MS = 2800;
const MAX_RENDERED_TRANSCRIPT_SEGMENTS = 300;
const MAX_COMMITTED_TRANSCRIPT_SEGMENTS = 1800;

function normalizeText(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, " ");
}

function segmentAnchor(segment: LiveTranscriptSegment): string {
  // Bucketing by start+speaker lets us update partial ASR revisions in-place.
  const bucket = Math.round(segment.start_ms / 300);
  return `${segment.speaker}|${bucket}`;
}

function preferSegment(
  current: LiveTranscriptSegment,
  incoming: LiveTranscriptSegment
): LiveTranscriptSegment {
  const currentText = normalizeText(current.text);
  const incomingText = normalizeText(incoming.text);

  if (incomingText === currentText) {
    return incoming.end_ms >= current.end_ms ? incoming : current;
  }

  if (incomingText.startsWith(currentText)) {
    return incoming;
  }
  if (currentText.startsWith(incomingText)) {
    return current;
  }

  if (incomingText.length !== currentText.length) {
    return incomingText.length > currentText.length ? incoming : current;
  }

  return incoming.end_ms >= current.end_ms ? incoming : current;
}

function collapseTranscriptRevisions(
  segments: LiveTranscriptSegment[]
): LiveTranscriptSegment[] {
  const sorted = [...segments].sort((a, b) => a.start_ms - b.start_ms);
  const collapsed: LiveTranscriptSegment[] = [];

  for (const segment of sorted) {
    const normalized = normalizeText(segment.text);
    if (normalized.length === 0) {
      continue;
    }

    const previous = collapsed[collapsed.length - 1];
    if (!previous) {
      collapsed.push(segment);
      continue;
    }

    const prevText = normalizeText(previous.text);
    const nearInTime = segment.start_ms <= previous.end_ms + 2200;
    const sameSpeaker = segment.speaker === previous.speaker;
    const looksLikeRevision =
      sameSpeaker &&
      nearInTime &&
      (normalized.startsWith(prevText) || prevText.startsWith(normalized));

    if (looksLikeRevision) {
      collapsed[collapsed.length - 1] = preferSegment(previous, segment);
      continue;
    }

    collapsed.push(segment);
  }

  return collapsed;
}

function trimCommittedMap(
  committed: Map<string, LiveTranscriptSegment>
): void {
  if (committed.size <= MAX_COMMITTED_TRANSCRIPT_SEGMENTS) {
    return;
  }

  const keep = [...committed.values()]
    .sort((a, b) => a.start_ms - b.start_ms)
    .slice(-MAX_COMMITTED_TRANSCRIPT_SEGMENTS);

  committed.clear();
  for (const segment of keep) {
    committed.set(segmentAnchor(segment), segment);
  }
}

function mergeLivePreviewIncremental(
  committed: Map<string, LiveTranscriptSegment>,
  incoming: LiveTranscriptSegment[],
  durationMs: number
): LiveTranscriptSegment[] {
  const ordered = incoming
    .filter((segment) => normalizeText(segment.text).length > 0)
    .sort((a, b) => a.start_ms - b.start_ms);

  const stableCutoffMs = Math.max(0, durationMs - LIVE_PREVIEW_STABILIZATION_MS);

  for (const segment of ordered) {
    if (segment.end_ms > stableCutoffMs) {
      continue;
    }

    const key = segmentAnchor(segment);
    const existing = committed.get(key);
    committed.set(key, existing ? preferSegment(existing, segment) : segment);
  }

  trimCommittedMap(committed);

  const display = new Map<string, LiveTranscriptSegment>();
  for (const segment of committed.values()) {
    display.set(segmentAnchor(segment), segment);
  }

  for (const segment of ordered) {
    if (segment.end_ms <= stableCutoffMs) {
      continue;
    }

    const key = segmentAnchor(segment);
    const existing = display.get(key);
    display.set(key, existing ? preferSegment(existing, segment) : segment);
  }

  return collapseTranscriptRevisions([...display.values()]).slice(
    -MAX_RENDERED_TRANSCRIPT_SEGMENTS
  );
}

function getStageLabel(stage: string): string {
  switch (stage) {
    case "TranscribingMic":
      return "Transcribing microphone audio";
    case "TranscribingSystem":
      return "Transcribing system audio";
    case "Merging":
      return "Merging transcript channels";
    case "Complete":
      return "Transcript ready";
    default:
      return "Processing transcript";
  }
}

function notesDraftKey(meetingId: string): string {
  return `meeting-scribe-live-notes:${meetingId}`;
}

export function RecordingView() {
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
  const [backgroundProcessing, setBackgroundProcessing] =
    useState<ProcessingProgress | null>(null);
  const [livePreviewSegments, setLivePreviewSegments] = useState<
    LiveTranscriptSegment[]
  >([]);
  const [livePreviewError, setLivePreviewError] = useState<string | null>(null);
  const [liveNotes, setLiveNotes] = useState("");

  const livePreviewInFlightRef = useRef(false);
  const committedLiveSegmentsRef = useRef<Map<string, LiveTranscriptSegment>>(
    new Map()
  );

  const isRecording = recordingState === "Recording";

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

    void fetchState();
  }, []);

  // Load notes draft when meeting ID changes
  useEffect(() => {
    if (!meetingId) {
      setLiveNotes("");
      return;
    }

    const savedDraft = localStorage.getItem(notesDraftKey(meetingId));
    if (savedDraft !== null) {
      setLiveNotes(savedDraft);
    } else {
      setLiveNotes("");
    }
  }, [meetingId]);

  // Persist notes draft while recording
  useEffect(() => {
    if (!meetingId || !isRecording) {
      return;
    }
    localStorage.setItem(notesDraftKey(meetingId), liveNotes);
  }, [liveNotes, meetingId, isRecording]);

  // Listen for waveform updates
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    const setupListener = async () => {
      const dispose = await listen<WaveformUpdate>("waveform-update", (event) => {
        const { mic, system, duration_ms } = event.payload;
        setMicMetrics(mic);
        setSystemMetrics(system);
        setDurationMs(duration_ms);
      });

      if (cancelled) {
        dispose();
        return;
      }

      unlisten = dispose;
    };

    void setupListener();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Listen for background processing progress and completion
  useEffect(() => {
    let unlistenProgress: UnlistenFn | undefined;
    let unlistenFinished: UnlistenFn | undefined;
    let cancelled = false;

    const setupListeners = async () => {
      const progressDispose = await listen<ProcessingProgress>(
        "meeting-processing-progress",
        (event) => {
          setBackgroundProcessing(event.payload);
        }
      );

      const finishedDispose = await listen<MeetingProcessingFinishedEvent>(
        "meeting-processing-finished",
        (event) => {
          const payload = event.payload;
          setBackgroundProcessing((current) =>
            current?.meeting_id === payload.meeting_id ? null : current
          );

          if (payload.success) {
            const segmentCount = payload.segment_count ?? 0;
            toast.success(
              "Transcript complete",
              `Meeting processed (${segmentCount} segments).`
            );
          } else {
            toast.error(
              "Transcript failed",
              payload.error_message ?? "Background processing failed."
            );
          }
        }
      );

      if (cancelled) {
        progressDispose();
        finishedDispose();
        return;
      }

      unlistenProgress = progressDispose;
      unlistenFinished = finishedDispose;
    };

    void setupListeners();

    return () => {
      cancelled = true;
      unlistenProgress?.();
      unlistenFinished?.();
    };
  }, [toast]);

  // Poll semi-realtime transcript preview while recording
  useEffect(() => {
    if (!isRecording || !meetingId || !settings.liveTranscriptionEnabled) {
      committedLiveSegmentsRef.current.clear();
      setLivePreviewSegments([]);
      setLivePreviewError(null);
      return;
    }

    if (!settings.transcriptionReady) {
      setLivePreviewError("Load a transcription model to use live preview.");
      return;
    }

    let cancelled = false;
    const intervalMs = Math.max(2, settings.liveTranscriptionIntervalSec) * 1000;

    const pollPreview = async () => {
      if (cancelled || livePreviewInFlightRef.current) {
        return;
      }
      livePreviewInFlightRef.current = true;

      try {
        const preview = await api.getLiveTranscriptionPreview(meetingId, {
          windowSeconds: Math.max(10, settings.liveTranscriptionIntervalSec * 3),
          includeSystemAudio: true,
        });

        if (cancelled) return;

        setLivePreviewSegments(() =>
          mergeLivePreviewIncremental(
            committedLiveSegmentsRef.current,
            preview.segments,
            preview.duration_ms
          )
        );
        setLivePreviewError(null);
      } catch (e) {
        if (cancelled) return;
        const errorMsg = e instanceof Error ? e.message : String(e);
        if (!errorMsg.toLowerCase().includes("only available while recording")) {
          setLivePreviewError(errorMsg);
        }
      } finally {
        livePreviewInFlightRef.current = false;
      }
    };

    void pollPreview();
    const timer = window.setInterval(() => {
      void pollPreview();
    }, intervalMs);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
      livePreviewInFlightRef.current = false;
    };
  }, [
    isRecording,
    meetingId,
    settings.liveTranscriptionEnabled,
    settings.liveTranscriptionIntervalSec,
    settings.transcriptionReady,
  ]);

  const handleStartRecording = useCallback(async () => {
    setError(null);
    setIsLoading(true);

    try {
      const loopbackDevice = settings.macSystemAudioDevice.trim();
      const id = await api.startRecording({
        macSystemAudio: {
          backend: settings.macSystemAudioBackend,
          loopbackDevice: loopbackDevice.length > 0 ? loopbackDevice : undefined,
        },
      });

      setMeetingId(id);
      setRecordingState("Recording");
      setDurationMs(0);
      committedLiveSegmentsRef.current.clear();
      setLivePreviewSegments([]);
      setLivePreviewError(null);
      setLiveNotes(localStorage.getItem(notesDraftKey(id)) ?? "");
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      console.error("Failed to start recording:", e);
    } finally {
      setIsLoading(false);
    }
  }, [settings.macSystemAudioBackend, settings.macSystemAudioDevice]);

  const handleStopRecording = useCallback(async () => {
    setError(null);
    setIsLoading(true);

    try {
      const result = await invoke<RecordingResult>("stop_recording");

      setMicMetrics({ rms: 0, peak: 0, samples: EMPTY_SAMPLES, speech_probability: null });
      setSystemMetrics({ rms: 0, peak: 0, samples: EMPTY_SAMPLES, speech_probability: null });
      setLivePreviewSegments([]);
      setLivePreviewError(null);

      const now = Date.now();
      await api.createMeetingWithId({
        id: result.meeting_id,
        title: `Meeting ${new Date().toLocaleString()}`,
        created_at: now,
        updated_at: now,
        duration_ms: result.duration_ms,
        audio_path_you: result.mic_path ?? null,
        audio_path_others: result.system_path ?? null,
        status: settings.autoProcessMeetings ? "processing" : "ready",
        error_message: null,
        tags: [],
      });

      const finalNotes = liveNotes.trim();
      if (finalNotes.length > 0) {
        try {
          await api.saveNote(result.meeting_id, finalNotes);
          toast.success("Notes saved", "Your meeting notes were attached.");
        } catch (noteError) {
          const noteErrorMsg = noteError instanceof Error ? noteError.message : String(noteError);
          toast.warning("Notes could not be saved", noteErrorMsg);
        }
      }
      localStorage.removeItem(notesDraftKey(result.meeting_id));

      if (!settings.autoProcessMeetings) {
        toast.info(
          "Recording saved",
          "Auto-processing is disabled. Generate transcript from the meeting view when needed."
        );
      } else {
        let transcriptionReady = settings.transcriptionReady;
        if (!transcriptionReady) {
          toast.info("Loading transcription model...");
          transcriptionReady = await settings.initializeTranscription();
        }

        if (!transcriptionReady) {
          const modelError =
            "Transcription model unavailable. Download and load a model to process this recording.";
          await api.updateMeetingStatus(result.meeting_id, "error", modelError);
          toast.warning("Transcription not available", modelError);
        } else {
          await api.startMeetingProcessing(
            result.meeting_id,
            result.mic_path ?? undefined,
            result.system_path ?? undefined,
            { echoBackend: settings.echoCancellationBackend }
          );

          setBackgroundProcessing({
            meeting_id: result.meeting_id,
            stage: "TranscribingMic",
            percent: 0,
            message: "Queued for background processing",
          });

          toast.info(
            "Processing in background",
            "You can immediately start your next meeting."
          );
        }
      }

      setRecordingState("Idle");
      setMeetingId(null);
      setDurationMs(0);
      committedLiveSegmentsRef.current.clear();
      setLiveNotes("");
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      console.error("Failed to stop recording:", e);
      setRecordingState("Idle");
    } finally {
      setIsLoading(false);
    }
  }, [settings, toast, liveNotes]);

  const handleEnableLivePreview = useCallback(async () => {
    settings.setLiveTranscriptionEnabled(true);
    if (!settings.transcriptionReady) {
      toast.info("Loading transcription model...");
      const ready = await settings.initializeTranscription();
      if (!ready) {
        toast.warning(
          "Transcription model unavailable",
          "Download a model in Settings to enable live transcript preview."
        );
      }
    }
  }, [settings, toast]);
  const transcriptLines = livePreviewSegments;

  return (
    <div className="h-full min-h-0 flex flex-col gap-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">Live Meeting Notepad</h1>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
            Notes stay visible; live transcript appears when enabled.
          </p>
        </div>
        {!settings.liveTranscriptionEnabled && (
          <button onClick={handleEnableLivePreview} className="btn btn-secondary text-xs px-3 py-1.5">
            Enable Live Transcript
          </button>
        )}
      </div>

      {error && (
        <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-4 text-red-700 dark:text-red-300">
          {error}
        </div>
      )}

      {backgroundProcessing && !isRecording && (
        <div className="bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 rounded-lg p-4">
          <div className="flex items-center justify-between text-sm text-indigo-700 dark:text-indigo-300">
            <span className="font-medium">{getStageLabel(backgroundProcessing.stage)}</span>
            <span>{Math.round(backgroundProcessing.percent)}%</span>
          </div>
          <div className="mt-2 h-2 bg-indigo-100 dark:bg-indigo-900/40 rounded-full overflow-hidden">
            <div
              className="h-full bg-indigo-500 transition-all duration-300"
              style={{ width: `${Math.min(100, Math.max(0, backgroundProcessing.percent))}%` }}
            />
          </div>
          <p className="mt-2 text-xs text-indigo-600 dark:text-indigo-400">
            {backgroundProcessing.message || "Transcript is processing in the background."}
          </p>
        </div>
      )}

      <section className="rounded-xl border border-sky-100 dark:border-slate-700 bg-gradient-to-br from-white via-sky-50/35 to-emerald-50/20 dark:from-surface-900 dark:via-surface-900 dark:to-surface-900 p-3 shadow-sm">
        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
          <div>
            <div className="text-3xl font-mono font-semibold tabular-nums text-slate-900 dark:text-slate-100">
              {formatDuration(durationMs)}
            </div>
            <div className="mt-0.5 text-sm text-gray-500 dark:text-gray-400">
              {isRecording ? "Recording in progress" : "Ready for next meeting"}
            </div>
          </div>

          <div className="flex items-center gap-3">
            {!isRecording ? (
              <button
                onClick={handleStartRecording}
                disabled={isLoading}
                className="btn btn-primary text-sm px-5 py-2.5 flex items-center gap-2"
              >
                {isLoading ? (
                  <>
                    <span className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Starting...
                  </>
                ) : (
                  <>
                    <span className="w-2.5 h-2.5 rounded-full bg-white" />
                    Start Meeting
                  </>
                )}
              </button>
            ) : (
              <button
                onClick={handleStopRecording}
                disabled={isLoading}
                className="btn bg-red-500 hover:bg-red-600 text-white text-sm px-5 py-2.5 flex items-center gap-2"
              >
                {isLoading ? (
                  <>
                    <span className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Stopping...
                  </>
                ) : (
                  <>
                    <span className="w-2.5 h-2.5 rounded-sm bg-white" />
                    Stop Meeting
                  </>
                )}
              </button>
            )}
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-2.5 mt-2.5">
          <Waveform
            samples={micMetrics.samples}
            rms={micMetrics.rms}
            color="#3b82f6"
            label="You (Microphone)"
            height={46}
          />
          <Waveform
            samples={systemMetrics.samples}
            rms={systemMetrics.rms}
            color="#10b981"
            label="Others (System Audio)"
            height={46}
          />
        </div>
      </section>

      <div
        className={`grid flex-1 min-h-0 gap-4 ${
          settings.liveTranscriptionEnabled ? 'grid-cols-1 lg:grid-cols-2' : 'grid-cols-1'
        }`}
      >
        {settings.liveTranscriptionEnabled && (
          <section className="card min-h-0 p-0 overflow-hidden flex flex-col">
          <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <FileText className="w-4 h-4 text-indigo-500" />
              <h2 className="text-sm font-semibold text-gray-800 dark:text-gray-100">Live Transcript</h2>
            </div>
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {settings.liveTranscriptionEnabled
                ? `Every ${settings.liveTranscriptionIntervalSec}s`
                : "Disabled"}
            </span>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3 space-y-3 bg-white/50 dark:bg-surface-900/20">
            {!isRecording ? (
              <p className="text-sm text-gray-500 dark:text-gray-400">Start recording to see live transcript context here.</p>
            ) : !settings.transcriptionReady ? (
              <p className="text-sm text-amber-700 dark:text-amber-300">
                Load a transcription model to show live transcript updates.
              </p>
            ) : livePreviewError ? (
              <p className="text-sm text-red-600 dark:text-red-300">{livePreviewError}</p>
            ) : transcriptLines.length === 0 ? (
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Listening and building transcript context...
              </p>
            ) : (
              transcriptLines.map((segment, index) => (
                <div key={`${segment.start_ms}-${index}`} className="rounded-lg border border-gray-100 dark:border-gray-800 px-3 py-2">
                  <div className="text-[11px] uppercase tracking-wide text-gray-500 dark:text-gray-400">
                    {formatDuration(segment.start_ms)} • {segment.speaker}
                  </div>
                  <div className="text-sm text-gray-800 dark:text-gray-200 mt-0.5">{segment.text}</div>
                </div>
              ))
            )}
          </div>
          </section>
        )}

        <section className="card min-h-0 p-0 overflow-hidden flex flex-col">
          <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <NotebookPen className="w-4 h-4 text-emerald-500" />
              <h2 className="text-sm font-semibold text-gray-800 dark:text-gray-100">My Meeting Notes</h2>
            </div>
            <span className="text-xs text-gray-500 dark:text-gray-400">{liveNotes.length} chars</span>
          </div>

          <div className="flex-1 min-h-0 p-3 bg-white/70 dark:bg-surface-900/20">
            <textarea
              value={liveNotes}
              onChange={(e) => setLiveNotes(e.target.value)}
              disabled={!meetingId}
              placeholder={
                meetingId
                  ? "Write notes, decisions, and action items while the meeting is running..."
                  : "Start recording to begin this meeting notepad."
              }
              className="w-full h-full min-h-0 resize-none rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-surface-900 px-3 py-3 text-sm leading-6 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-emerald-400"
            />
          </div>

          <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700 text-xs text-gray-500 dark:text-gray-400">
            Notes are auto-saved locally during recording and attached to the meeting when you stop.
          </div>
        </section>
      </div>
    </div>
  );
}
