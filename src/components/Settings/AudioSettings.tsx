/**
 * Audio capture settings section
 */

import { useEffect, useMemo, useState } from 'react';
import { AudioLines } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { useSettingsStore } from '../../stores';
import * as api from '../../lib/tauri';

interface AudioSettingsProps {
  platform?: string;
}

function loopbackDeviceScore(name: string): number {
  const value = name.toLowerCase();
  let score = 0;

  if (value.includes('monitor of')) score = Math.max(score, 130);
  if (value.includes('.monitor')) score = Math.max(score, 120);
  if (value.includes('monitor')) score = Math.max(score, 110);
  if (value.includes('blackhole')) score = Math.max(score, 100);
  if (value.includes('loopback')) score = Math.max(score, 95);
  if (value.includes('soundflower')) score = Math.max(score, 90);
  if (value.includes('background music')) score = Math.max(score, 85);
  if (value.includes('vb-cable') || value.includes('vb cable')) score = Math.max(score, 80);
  if (value.includes('stereo mix')) score = Math.max(score, 75);
  if (value.includes('what u hear')) score = Math.max(score, 75);
  if (value.includes('system audio')) score = Math.max(score, 70);
  if (value.includes('microphone') || value.includes('mic')) score = Math.max(0, score - 50);

  return score;
}

export function AudioSettings({ platform }: AudioSettingsProps) {
  const normalizedPlatform = platform?.toLowerCase();
  const isPlatformKnown = Boolean(normalizedPlatform);
  const isMac = normalizedPlatform === 'macos' || normalizedPlatform === 'darwin';
  const isLinux = normalizedPlatform === 'linux';
  const supportsLoopbackInputSelection = isMac || isLinux;
  const [inputDevices, setInputDevices] = useState<string[]>([]);
  const [devicesLoading, setDevicesLoading] = useState(false);
  const [devicesError, setDevicesError] = useState<string | null>(null);

  const {
    echoCancellationBackend,
    setEchoCancellationBackend,
    liveTranscriptionEnabled,
    liveTranscriptionIntervalSec,
    setLiveTranscriptionEnabled,
    setLiveTranscriptionIntervalSec,
    macSystemAudioBackend,
    macSystemAudioDevice,
    setMacSystemAudioBackend,
    setMacSystemAudioDevice,
  } = useSettingsStore();

  useEffect(() => {
    if (!supportsLoopbackInputSelection) {
      setInputDevices([]);
      setDevicesError(null);
      return;
    }

    let cancelled = false;
    setDevicesLoading(true);
    setDevicesError(null);

    api
      .listAudioDevices()
      .then((devices) => {
        if (cancelled) return;
        setInputDevices(devices.input_devices);
      })
      .catch((e) => {
        if (cancelled) return;
        setDevicesError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (!cancelled) setDevicesLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [supportsLoopbackInputSelection]);

  const sortedDevices = useMemo(() => {
    return [...inputDevices].sort((a, b) => loopbackDeviceScore(b) - loopbackDeviceScore(a));
  }, [inputDevices]);

  return (
    <Card>
      <div className="flex items-center gap-2 mb-4">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent/70">
          <AudioLines className="w-5 h-5 text-muted-foreground" />
        </div>
        <CardTitle>Audio Capture And Echo Control</CardTitle>
      </div>

      <div className="space-y-4">
        <div>
          <label
            htmlFor="echo-cancellation-backend"
            className="mb-2 block text-sm font-medium text-foreground"
          >
            Echo Cancellation Backend
          </label>
          <select
            id="echo-cancellation-backend"
            value={echoCancellationBackend}
            onChange={(e) => setEchoCancellationBackend(e.target.value as 'webrtc_aec3' | 'speex')}
            className="w-full rounded-md border border-input bg-input-background px-3 py-2 text-sm text-foreground"
          >
            <option value="webrtc_aec3">WebRTC AEC3 (Recommended)</option>
            <option value="speex">SpeexDSP (Legacy/Fallback)</option>
          </select>
          <p className="mt-2 text-xs text-muted-foreground">
            Applied during transcript processing to remove speaker playback from the mic channel.
          </p>
        </div>

        <div className="space-y-3 rounded-lg border border-border p-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-sm font-medium text-foreground">
                Live Transcript Preview (Experimental)
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                Shows rolling semi-realtime transcript snippets while recording. Uses extra CPU.
              </p>
            </div>
            <label className="inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                checked={liveTranscriptionEnabled}
                onChange={(e) => setLiveTranscriptionEnabled(e.target.checked)}
                className="sr-only peer"
              />
              <span className="relative h-6 w-10 rounded-full bg-muted transition-colors peer peer-checked:bg-primary">
                <span className="absolute top-0.5 left-0.5 w-5 h-5 bg-card rounded-full shadow transform peer-checked:translate-x-4 transition-transform" />
              </span>
            </label>
          </div>

          {liveTranscriptionEnabled && (
            <div>
              <label
                htmlFor="live-preview-interval"
                className="mb-2 block text-sm font-medium text-foreground"
              >
                Preview Refresh Interval
              </label>
              <select
                id="live-preview-interval"
                value={liveTranscriptionIntervalSec}
                onChange={(e) => setLiveTranscriptionIntervalSec(Number(e.target.value))}
                className="w-full rounded-md border border-input bg-input-background px-3 py-2 text-sm text-foreground"
              >
                <option value={3}>Every 3 seconds</option>
                <option value={5}>Every 5 seconds</option>
                <option value={6}>Every 6 seconds</option>
                <option value={8}>Every 8 seconds</option>
                <option value={10}>Every 10 seconds</option>
              </select>
            </div>
          )}
        </div>

        {!isPlatformKnown ? (
          <p className="text-sm text-muted-foreground">
            Detecting platform...
          </p>
        ) : !supportsLoopbackInputSelection ? (
          <p className="text-sm text-muted-foreground">
            System-audio input selection is currently available on macOS and Linux. Windows capture uses output-loopback device selection.
          </p>
        ) : (
          <>
            <p className="text-sm text-muted-foreground">
              Capture settings apply on the next recording start.
            </p>

            {isMac && (
              <div>
                <label
                  htmlFor="mac-system-audio-backend"
                  className="mb-2 block text-sm font-medium text-foreground"
                >
                  Capture Backend
                </label>
                <select
                  id="mac-system-audio-backend"
                  value={macSystemAudioBackend}
                  onChange={(e) => setMacSystemAudioBackend(e.target.value as 'auto' | 'process_tap' | 'loopback')}
                  className="w-full rounded-md border border-input bg-input-background px-3 py-2 text-sm text-foreground"
                >
                  <option value="auto">Auto (Process Tap, then loopback fallback)</option>
                  <option value="process_tap">CoreAudio Process Tap (macOS 14.2+)</option>
                  <option value="loopback">Loopback Input Device Only</option>
                </select>
              </div>
            )}

            <div>
              <label
                htmlFor="mac-loopback-device"
                className="mb-2 block text-sm font-medium text-foreground"
              >
                Preferred System Audio Input (Optional)
              </label>
              <select
                id="mac-loopback-device"
                value={macSystemAudioDevice}
                onChange={(e) => setMacSystemAudioDevice(e.target.value)}
                className="w-full rounded-md border border-input bg-input-background px-3 py-2 text-sm text-foreground"
                disabled={devicesLoading}
              >
                <option value="">Auto-detect best monitor/loopback input</option>
                {sortedDevices.map((device) => (
                  <option key={device} value={device}>
                    {device}
                  </option>
                ))}
              </select>
              <p className="mt-2 text-xs text-muted-foreground">
                {isMac
                  ? 'Choose a loopback input device if auto-detection is wrong.'
                  : 'Choose a PipeWire/Pulse monitor input device if auto-detection is wrong.'}
              </p>
              {devicesLoading && (
                <p className="mt-2 text-xs text-muted-foreground">
                  Loading input devices...
                </p>
              )}
              {devicesError && (
                <p className="text-xs text-destructive mt-2">
                  Failed to list devices: {devicesError}
                </p>
              )}
            </div>
          </>
        )}
      </div>
    </Card>
  );
}
