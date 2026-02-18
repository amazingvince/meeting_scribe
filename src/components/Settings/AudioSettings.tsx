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
        <AudioLines className="w-5 h-5 text-teal-500" />
        <CardTitle>Audio Capture And Echo Control</CardTitle>
      </div>

      <div className="space-y-4">
        <div>
          <label
            htmlFor="echo-cancellation-backend"
            className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
          >
            Echo Cancellation Backend
          </label>
          <select
            id="echo-cancellation-backend"
            value={echoCancellationBackend}
            onChange={(e) => setEchoCancellationBackend(e.target.value as 'webrtc_aec3' | 'speex')}
            className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100"
          >
            <option value="webrtc_aec3">WebRTC AEC3 (Recommended)</option>
            <option value="speex">SpeexDSP (Legacy/Fallback)</option>
          </select>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
            Applied during transcript processing to remove speaker playback from the mic channel.
          </p>
        </div>

        <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-3 space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-sm font-medium text-gray-700 dark:text-gray-200">
                Live Transcript Preview (Experimental)
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
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
              <span className="relative w-10 h-6 bg-gray-200 dark:bg-gray-700 rounded-full peer peer-checked:bg-teal-500 transition-colors">
                <span className="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transform peer-checked:translate-x-4 transition-transform" />
              </span>
            </label>
          </div>

          {liveTranscriptionEnabled && (
            <div>
              <label
                htmlFor="live-preview-interval"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Preview Refresh Interval
              </label>
              <select
                id="live-preview-interval"
                value={liveTranscriptionIntervalSec}
                onChange={(e) => setLiveTranscriptionIntervalSec(Number(e.target.value))}
                className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100"
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
          <p className="text-sm text-gray-600 dark:text-gray-400">
            Detecting platform...
          </p>
        ) : !supportsLoopbackInputSelection ? (
          <p className="text-sm text-gray-600 dark:text-gray-400">
            System-audio input selection is currently available on macOS and Linux. Windows capture uses output-loopback device selection.
          </p>
        ) : (
          <>
            <p className="text-sm text-gray-600 dark:text-gray-400">
              Capture settings apply on the next recording start.
            </p>

            {isMac && (
              <div>
                <label
                  htmlFor="mac-system-audio-backend"
                  className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
                >
                  Capture Backend
                </label>
                <select
                  id="mac-system-audio-backend"
                  value={macSystemAudioBackend}
                  onChange={(e) => setMacSystemAudioBackend(e.target.value as 'auto' | 'process_tap' | 'loopback')}
                  className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100"
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
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Preferred System Audio Input (Optional)
              </label>
              <select
                id="mac-loopback-device"
                value={macSystemAudioDevice}
                onChange={(e) => setMacSystemAudioDevice(e.target.value)}
                className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100"
                disabled={devicesLoading}
              >
                <option value="">Auto-detect best monitor/loopback input</option>
                {sortedDevices.map((device) => (
                  <option key={device} value={device}>
                    {device}
                  </option>
                ))}
              </select>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
                {isMac
                  ? 'Choose a loopback input device if auto-detection is wrong.'
                  : 'Choose a PipeWire/Pulse monitor input device if auto-detection is wrong.'}
              </p>
              {devicesLoading && (
                <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
                  Loading input devices...
                </p>
              )}
              {devicesError && (
                <p className="text-xs text-red-600 dark:text-red-400 mt-2">
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
