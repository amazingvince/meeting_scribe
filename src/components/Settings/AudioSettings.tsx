/**
 * Audio capture settings section
 */

import { useEffect, useMemo, useState } from 'react';
import { AudioLines } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { Select } from '../ui/Select';
import { Toggle } from '../ui/Toggle';
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

function microphoneDeviceScore(name: string): number {
  const value = name.toLowerCase();

  if (
    value.includes('blackhole') ||
    value.includes('loopback') ||
    value.includes('soundflower') ||
    value.includes('vb-cable') ||
    value.includes('vb cable') ||
    value.includes('background music') ||
    value.includes('monitor of') ||
    value.includes('.monitor') ||
    value.includes('process tap') ||
    value.includes('system audio') ||
    value.includes('virtual audio')
  ) {
    return -100;
  }

  let score = 0;
  if (value.includes('microphone') || value.includes('mic')) score += 90;
  if (value.includes('built-in') || value.includes('builtin') || value.includes('internal')) score += 35;
  if (value.includes('headset') || value.includes('headphone') || value.includes('airpods')) score += 30;
  if (value.includes('usb') || value.includes('external')) score += 20;
  return score;
}

export function AudioSettings({ platform }: AudioSettingsProps) {
  const normalizedPlatform = platform?.toLowerCase();
  const isPlatformKnown = Boolean(normalizedPlatform);
  const isMac = normalizedPlatform === 'macos' || normalizedPlatform === 'darwin';
  const isLinux = normalizedPlatform === 'linux';
  const supportsLoopbackInputSelection = isMac || isLinux;
  const supportsInputDeviceSelection = isPlatformKnown;
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
    microphoneDevice,
    setMacSystemAudioBackend,
    setMacSystemAudioDevice,
    setMicrophoneDevice,
  } = useSettingsStore();

  useEffect(() => {
    if (!supportsInputDeviceSelection) {
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
  }, [supportsInputDeviceSelection]);

  const sortedLoopbackDevices = useMemo(() => {
    return [...inputDevices].sort((a, b) => loopbackDeviceScore(b) - loopbackDeviceScore(a));
  }, [inputDevices]);

  const sortedMicrophoneDevices = useMemo(() => {
    return [...inputDevices].sort((a, b) => {
      const scoreDelta = microphoneDeviceScore(b) - microphoneDeviceScore(a);
      if (scoreDelta !== 0) {
        return scoreDelta;
      }
      return a.localeCompare(b);
    });
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
          <Select
            id="echo-cancellation-backend"
            label="Echo Cancellation Backend"
            value={echoCancellationBackend}
            onChange={(e) => setEchoCancellationBackend(e.target.value as 'webrtc_aec3' | 'speex')}
          >
            <option value="webrtc_aec3">WebRTC AEC3 (Recommended)</option>
            <option value="speex">SpeexDSP (Legacy/Fallback)</option>
          </Select>
          <p className="mt-2 text-xs text-muted-foreground">
            Applied during transcript processing to remove speaker playback from the mic channel.
          </p>
        </div>

        <div className="space-y-3 rounded-lg border border-border p-3">
          <Toggle
            checked={liveTranscriptionEnabled}
            onChange={setLiveTranscriptionEnabled}
            label="Live Transcript Preview (Experimental)"
            description="Shows rolling semi-realtime transcript snippets while recording. Uses extra CPU."
          />

          {liveTranscriptionEnabled && (
            <Select
              id="live-preview-interval"
              label="Preview Refresh Interval"
              value={liveTranscriptionIntervalSec}
              onChange={(e) => setLiveTranscriptionIntervalSec(Number(e.target.value))}
            >
              <option value={3}>Every 3 seconds</option>
              <option value={5}>Every 5 seconds</option>
              <option value={6}>Every 6 seconds</option>
              <option value={8}>Every 8 seconds</option>
              <option value={10}>Every 10 seconds</option>
            </Select>
          )}
        </div>

        {!isPlatformKnown ? (
          <p className="text-sm text-muted-foreground">
            Detecting platform...
          </p>
        ) : (
          <>
            <p className="text-sm text-muted-foreground">
              Capture settings apply on the next recording start.
            </p>

            <div>
              <Select
                id="microphone-device"
                label="Preferred Microphone Input (Optional)"
                value={microphoneDevice}
                onChange={(e) => setMicrophoneDevice(e.target.value)}
                disabled={devicesLoading}
              >
                <option value="">Auto-select microphone input</option>
                {sortedMicrophoneDevices.map((device) => (
                  <option key={device} value={device}>
                    {device}
                  </option>
                ))}
              </Select>
              <p className="mt-2 text-xs text-muted-foreground">
                Choose a microphone explicitly if auto-selection picks the wrong input device.
              </p>
            </div>

            {supportsLoopbackInputSelection ? (
              <>
                {isMac && (
                  <Select
                    id="mac-system-audio-backend"
                    label="Capture Backend"
                    value={macSystemAudioBackend}
                    onChange={(e) =>
                      setMacSystemAudioBackend(e.target.value as 'auto' | 'process_tap' | 'loopback')
                    }
                  >
                    <option value="auto">Auto (Process Tap, then loopback fallback)</option>
                    <option value="process_tap">CoreAudio Process Tap (macOS 14.2+)</option>
                    <option value="loopback">Loopback Input Device Only</option>
                  </Select>
                )}

                <div>
                  <Select
                    id="mac-loopback-device"
                    label="Preferred System Audio Input (Optional)"
                    value={macSystemAudioDevice}
                    onChange={(e) => setMacSystemAudioDevice(e.target.value)}
                    disabled={devicesLoading}
                  >
                    <option value="">Auto-detect best monitor/loopback input</option>
                    {sortedLoopbackDevices.map((device) => (
                      <option key={device} value={device}>
                        {device}
                      </option>
                    ))}
                  </Select>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {isMac
                      ? 'Choose a loopback input device if auto-detection is wrong.'
                      : 'Choose a PipeWire/Pulse monitor input device if auto-detection is wrong.'}
                  </p>
                </div>
              </>
            ) : (
              <p className="text-sm text-muted-foreground">
                System-audio input selection is currently available on macOS and Linux. Windows uses output-loopback capture selection.
              </p>
            )}

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
          </>
        )}
      </div>
    </Card>
  );
}
