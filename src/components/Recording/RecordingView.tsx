import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { FileText, NotebookPen } from "lucide-react";
import { Waveform } from "./Waveform";
import { formatDuration } from "../../utils/format";
import * as api from "../../lib/tauri";
import { modelManager } from "../../lib/modelManager";
import { useSettingsStore, useToastStore } from "../../stores";
import { processingStageLabel } from "../../utils/stages";
import { Button } from "../ui/Button";
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

interface MeterNormalizerState {
  floor: number;
  ceiling: number;
  activeUntilMs: number;
}

interface MeterDisplay {
  metrics: ChannelMetrics;
  streaming: boolean;
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
const METER_ACTIVITY_HOLD_MS = 900;
const METER_MIN_VISUAL_LEVEL = 0.02;
const METER_RAW_ACTIVITY_THRESHOLD = 0.00028;
const METER_INITIAL_FLOOR = 0.00005;
const METER_INITIAL_CEILING = 0.012;
const METER_MIN_DYNAMIC_SPAN = 0.006;

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function createMeterNormalizerState(): MeterNormalizerState {
  return {
    floor: METER_INITIAL_FLOOR,
    ceiling: METER_INITIAL_CEILING,
    activeUntilMs: 0,
  };
}

function resetMeterNormalizerState(state: MeterNormalizerState): void {
  state.floor = METER_INITIAL_FLOOR;
  state.ceiling = METER_INITIAL_CEILING;
  state.activeUntilMs = 0;
}

function normalizeAmplitude(value: number, state: MeterNormalizerState): number {
  const span = Math.max(METER_MIN_DYNAMIC_SPAN, state.ceiling - state.floor);
  const normalized = clamp01((value - state.floor) / span);
  const boosted = 1 - Math.exp(-normalized * 3.2);
  return clamp01(Math.pow(boosted, 0.82));
}

function buildMeterDisplay(
  metrics: ChannelMetrics,
  state: MeterNormalizerState,
  isRecording: boolean,
  nowMs: number
): MeterDisplay {
  if (!isRecording) {
    resetMeterNormalizerState(state);
    return {
      metrics: {
        ...metrics,
        rms: 0,
        peak: 0,
        samples: EMPTY_SAMPLES,
      },
      streaming: false,
    };
  }

  const sourcePeak = metrics.samples.reduce(
    (max, sample) => Math.max(max, Math.abs(sample)),
    Math.abs(metrics.peak)
  );
  const sourceLevel = Math.max(Math.abs(metrics.rms), sourcePeak * 0.82);

  const floorMix = sourceLevel <= state.floor ? 0.14 : 0.02;
  state.floor = Math.max(
    METER_INITIAL_FLOOR,
    state.floor * (1 - floorMix) + sourceLevel * floorMix
  );

  const desiredCeiling = Math.max(
    sourcePeak * 1.25,
    state.floor + METER_MIN_DYNAMIC_SPAN,
    METER_INITIAL_CEILING
  );
  state.ceiling = Math.max(desiredCeiling, state.ceiling * 0.97);
  state.ceiling = Math.min(1, Math.max(state.ceiling, state.floor + METER_MIN_DYNAMIC_SPAN));

  const normalizedSamples = metrics.samples.map((sample) => {
    const level = normalizeAmplitude(Math.abs(sample), state);
    if (level <= 0) return 0;
    return Math.max(METER_MIN_VISUAL_LEVEL, level);
  });

  const normalizedRms = normalizeAmplitude(Math.abs(metrics.rms), state);
  const normalizedPeak = normalizeAmplitude(sourcePeak, state);

  const activityNow =
    sourcePeak >= METER_RAW_ACTIVITY_THRESHOLD ||
    sourceLevel >= METER_RAW_ACTIVITY_THRESHOLD ||
    normalizedPeak >= 0.06;

  if (activityNow) {
    state.activeUntilMs = nowMs + METER_ACTIVITY_HOLD_MS;
  }

  const streaming = activityNow || nowMs < state.activeUntilMs;

  return {
    metrics: {
      ...metrics,
      rms: streaming ? Math.max(METER_MIN_VISUAL_LEVEL, normalizedRms) : normalizedRms,
      peak: normalizedPeak,
      samples: normalizedSamples,
    },
    streaming,
  };
}

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
  const [isEnablingLivePreview, setIsEnablingLivePreview] = useState(false);
  const [liveNotes, setLiveNotes] = useState("");

  const livePreviewInFlightRef = useRef(false);
  const committedLiveSegmentsRef = useRef<Map<string, LiveTranscriptSegment>>(
    new Map()
  );
  const micMeterStateRef = useRef<MeterNormalizerState>(createMeterNormalizerState());
  const systemMeterStateRef = useRef<MeterNormalizerState>(createMeterNormalizerState());

  const isRecording = recordingState === "Recording";

  useEffect(() => {
    if (!isRecording) {
      resetMeterNormalizerState(micMeterStateRef.current);
      resetMeterNormalizerState(systemMeterStateRef.current);
    }
  }, [isRecording]);

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
            // Completion is surfaced by the background-task pill; avoid extra success toasts.
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
      const microphoneDevice = settings.microphoneDevice.trim();
      const id = await api.startRecording({
        microphoneDevice: microphoneDevice.length > 0 ? microphoneDevice : undefined,
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
  }, [
    settings.macSystemAudioBackend,
    settings.macSystemAudioDevice,
    settings.microphoneDevice,
  ]);

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
          transcriptionReady = await modelManager.ensureTranscriptionReady();
        }

        if (!transcriptionReady) {
          const status = useSettingsStore.getState();
          const modelError =
            status.error ??
            (status.transcriptionDownloaded
              ? "Transcription model unavailable. Open Settings and load the model, then try again."
              : "Transcription model unavailable. Download the model in Settings to process recordings.");
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
    if (settings.liveTranscriptionEnabled) {
      settings.setLiveTranscriptionEnabled(false);
      setLivePreviewError(null);
      setLivePreviewSegments([]);
      committedLiveSegmentsRef.current.clear();
      return;
    }

    setIsEnablingLivePreview(true);
    try {
      let ready = settings.transcriptionReady;
      if (!ready) {
        ready = await modelManager.ensureTranscriptionReady();
      }

      if (!ready) {
        const status = useSettingsStore.getState();
        const detail =
          status.error ??
          (status.transcriptionDownloaded
            ? "Unable to load the transcription model. Try again in Settings."
            : "Download the transcription model in Settings to enable live transcript preview.");

        toast.warning(
          status.transcriptionDownloaded
            ? "Transcription model unavailable"
            : "Transcription model not downloaded",
          detail
        );
        return;
      }

      settings.setLiveTranscriptionEnabled(true);
      setLivePreviewError(null);
    } finally {
      setIsEnablingLivePreview(false);
    }
  }, [settings, toast]);
  const transcriptLines = livePreviewSegments;
  const livePreviewToggleBusy =
    isEnablingLivePreview || settings.isLoadingTranscription;
  const {
    micDisplayMetrics,
    systemDisplayMetrics,
    micStreaming,
    systemStreaming,
  } = useMemo(() => {
    const nowMs = Date.now();
    const micDisplay = buildMeterDisplay(
      micMetrics,
      micMeterStateRef.current,
      isRecording,
      nowMs
    );
    const systemDisplay = buildMeterDisplay(
      systemMetrics,
      systemMeterStateRef.current,
      isRecording,
      nowMs
    );
    return {
      micDisplayMetrics: micDisplay.metrics,
      systemDisplayMetrics: systemDisplay.metrics,
      micStreaming: micDisplay.streaming,
      systemStreaming: systemDisplay.streaming,
    };
  }, [micMetrics, systemMetrics, isRecording]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-border bg-card">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            {isRecording && (
              <span className="relative flex h-2.5 w-2.5">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-red-400 opacity-75" />
                <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-red-500" />
              </span>
            )}
            <h2 className="text-foreground">
              {isRecording ? "Recording" : "Ready"}
            </h2>
          </div>
          <div className="flex items-center gap-3 text-muted-foreground">
            <span className="text-sm tabular-nums font-mono">{formatDuration(durationMs)}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={handleEnableLivePreview}
            disabled={livePreviewToggleBusy}
            className="text-muted-foreground"
          >
            {livePreviewToggleBusy
              ? "Preparing..."
              : settings.liveTranscriptionEnabled
                ? "Hide Live Transcript"
                : "Enable Live Transcript"}
          </Button>
          {!isRecording ? (
            <Button
              onClick={handleStartRecording}
              disabled={isLoading}
              size="sm"
              className="min-w-[164px] justify-center text-center"
            >
              {isLoading ? (
                <>
                  <span className="h-4 w-4 rounded-full border-2 border-white border-t-transparent animate-spin" />
                  Starting...
                </>
              ) : (
                "Start Recording"
              )}
            </Button>
          ) : (
            <Button
              onClick={handleStopRecording}
              disabled={isLoading}
              variant="destructive"
              size="sm"
              className="gap-2"
            >
              {isLoading ? (
                <>
                  <span className="h-4 w-4 rounded-full border-2 border-white border-t-transparent animate-spin" />
                  Stopping...
                </>
              ) : (
                <>
                  <span className="h-2 w-2 rounded-sm bg-white" />
                  Stop
                </>
              )}
            </Button>
          )}
        </div>
      </header>

      {error && (
        <div className="border-b border-red-200 bg-red-50 px-6 py-2 text-sm text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300">
          {error}
        </div>
      )}

      {backgroundProcessing && !isRecording && (
        <div className="border-b border-brand/20 bg-brand/5 px-6 py-2.5">
          <div className="flex items-center justify-between text-xs text-brand">
            <span className="font-medium">{processingStageLabel(backgroundProcessing.stage)}</span>
            <span>{Math.round(backgroundProcessing.percent)}%</span>
          </div>
          <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-brand/10">
            <div
              className="h-full bg-brand transition-all duration-300"
              style={{ width: `${Math.min(100, Math.max(0, backgroundProcessing.percent))}%` }}
            />
          </div>
          <p className="mt-1.5 text-[11px] text-brand/70">
            {backgroundProcessing.message || "Transcript is processing in the background."}
          </p>
        </div>
      )}

      {/* Audio meters */}
      <div className="flex items-center gap-3 px-6 py-3 border-b border-border bg-card/50">
        <div className="flex-1 flex items-center gap-2">
          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground min-w-[80px]">
            <span
              className={`h-1.5 w-1.5 rounded-full ${
                micStreaming ? 'bg-success' : 'bg-muted-foreground/50'
              }`}
            />
            Mic {micStreaming ? '' : '(idle)'}
          </div>
          <div className="flex-1">
            <Waveform
              samples={micDisplayMetrics.samples}
              rms={micDisplayMetrics.rms}
              color="#3b82f6"
              height={20}
            />
          </div>
        </div>

        <div className="flex-1 flex items-center gap-2">
          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground min-w-[80px]">
            <span
              className={`h-1.5 w-1.5 rounded-full ${
                systemStreaming ? 'bg-success' : 'bg-muted-foreground/50'
              }`}
            />
            System {systemStreaming ? '' : '(idle)'}
          </div>
          <div className="flex-1">
            <Waveform
              samples={systemDisplayMetrics.samples}
              rms={systemDisplayMetrics.rms}
              color="#10b981"
              height={20}
            />
          </div>
        </div>
      </div>

      {/* Main content area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Transcript Panel */}
        {settings.liveTranscriptionEnabled && (
          <div className="flex-1 flex flex-col border-r border-border min-w-0">
            <div className="flex items-center justify-between px-5 py-3 border-b border-border bg-card/50">
              <div className="flex items-center gap-2">
                <FileText className="w-3.5 h-3.5 text-brand" />
                <span className="text-sm text-muted-foreground">Live Transcript</span>
              </div>
              <span className="text-[11px] text-muted-foreground/60">
                Every {settings.liveTranscriptionIntervalSec}s
              </span>
            </div>

            <div className="flex-1 min-h-0 overflow-y-auto p-5 space-y-4">
              {!isRecording ? (
                <p className="text-sm text-muted-foreground">Start recording to see live transcript here.</p>
              ) : !settings.transcriptionReady ? (
                <p className="text-sm text-warning">
                  Load a transcription model to show live transcript updates.
                </p>
              ) : livePreviewError ? (
                <p className="text-sm text-red-600 dark:text-red-300">{livePreviewError}</p>
              ) : transcriptLines.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  Listening and building transcript context...
                </p>
              ) : (
                transcriptLines.map((segment, index) => (
                  <div key={`${segment.start_ms}-${index}`} className="flex gap-3">
                    <span className="text-[11px] text-muted-foreground/60 tabular-nums pt-0.5 min-w-[52px]">
                      {formatDuration(segment.start_ms)}
                    </span>
                    <div className="flex-1 min-w-0">
                      <span className="text-sm text-muted-foreground">{segment.speaker}</span>
                      <p className="text-sm text-foreground/90 mt-0.5 leading-relaxed">{segment.text}</p>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}

        {/* Notes Panel */}
        <div className={`flex flex-col bg-card/30 ${settings.liveTranscriptionEnabled ? 'w-[420px] min-w-[320px]' : 'flex-1'}`}>
          <div className="flex items-center justify-between px-5 py-3 border-b border-border">
            <div className="flex items-center gap-2">
              <NotebookPen className="w-3.5 h-3.5 text-success" />
              <span className="text-sm text-muted-foreground">Meeting Notes</span>
            </div>
            <span className="text-[11px] text-muted-foreground/60">
              {meetingId ? 'Auto-saved' : 'Start recording'}
            </span>
          </div>
          <div className="flex-1 p-5">
            <div
              className={`h-full ${
                !meetingId
                  ? 'rounded-lg border border-border bg-input-background/50 opacity-50 p-3'
                  : ''
              }`}
            >
              <textarea
                value={liveNotes}
                onChange={(e) => setLiveNotes(e.target.value)}
                disabled={!meetingId}
                placeholder={
                  meetingId
                    ? "Start typing your notes..."
                    : "Hit 'Start Recording' to begin taking notes here..."
                }
                className="w-full h-full bg-transparent text-sm text-foreground/90 resize-none outline-none leading-relaxed placeholder:text-muted-foreground/40"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
