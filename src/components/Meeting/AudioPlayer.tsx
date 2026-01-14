/**
 * Audio player component with dual track support
 * Plays meeting recordings with playback controls and timestamp sync
 */

import {
  useState,
  useRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  forwardRef,
} from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Play, Pause, Volume2, VolumeX } from 'lucide-react';
import { formatDuration } from '../../utils/format';

export interface AudioPlayerHandle {
  seekTo: (ms: number) => void;
  play: () => void;
  pause: () => void;
}

interface AudioPlayerProps {
  /** Path to "you" audio file (mic) */
  micPath: string | null;
  /** Path to "others" audio file (system) */
  systemPath: string | null;
  /** Total duration in milliseconds */
  durationMs: number | null;
  /** Callback when playback time changes */
  onTimeUpdate?: (ms: number) => void;
}

export const AudioPlayer = forwardRef<AudioPlayerHandle, AudioPlayerProps>(
  function AudioPlayer({ micPath, systemPath, durationMs, onTimeUpdate }, ref) {
    const micAudioRef = useRef<HTMLAudioElement>(null);
    const systemAudioRef = useRef<HTMLAudioElement>(null);

    const [isPlaying, setIsPlaying] = useState(false);
    const [currentTime, setCurrentTime] = useState(0);
    const [duration, setDuration] = useState(durationMs ?? 0);
    const [volume, setVolume] = useState(1);
    const [isMuted, setIsMuted] = useState(false);
    const [activeTrack, setActiveTrack] = useState<'mic' | 'system' | 'both'>(
      'both'
    );
    const [isLoaded, setIsLoaded] = useState(false);

    // Convert file paths to playable URLs
    const micUrl = micPath ? convertFileSrc(micPath) : null;
    const systemUrl = systemPath ? convertFileSrc(systemPath) : null;

    // Expose methods via ref
    useImperativeHandle(ref, () => ({
      seekTo: (ms: number) => {
        const seconds = ms / 1000;
        if (micAudioRef.current) {
          micAudioRef.current.currentTime = seconds;
        }
        if (systemAudioRef.current) {
          systemAudioRef.current.currentTime = seconds;
        }
        setCurrentTime(ms);
      },
      play: () => {
        void playAudio();
      },
      pause: () => {
        pauseAudio();
      },
    }));

    const playAudio = useCallback(async (trackOverride?: typeof activeTrack) => {
      try {
        const track = trackOverride ?? activeTrack;
        const promises: Promise<void>[] = [];

        if (micAudioRef.current && (track === 'mic' || track === 'both')) {
          promises.push(micAudioRef.current.play());
        } else if (micAudioRef.current) {
          micAudioRef.current.pause();
        }

        if (systemAudioRef.current && (track === 'system' || track === 'both')) {
          promises.push(systemAudioRef.current.play());
        } else if (systemAudioRef.current) {
          systemAudioRef.current.pause();
        }

        await Promise.all(promises);
        setIsPlaying(true);
      } catch (error) {
        console.error('Failed to play audio:', error);
      }
    }, [activeTrack]);

    const pauseAudio = useCallback(() => {
      if (micAudioRef.current) {
        micAudioRef.current.pause();
      }
      if (systemAudioRef.current) {
        systemAudioRef.current.pause();
      }
      setIsPlaying(false);
    }, []);

    const togglePlayPause = useCallback(() => {
      if (isPlaying) {
        pauseAudio();
      } else {
        void playAudio();
      }
    }, [isPlaying, pauseAudio, playAudio]);

    const handleSeek = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        const newTime = Number(e.target.value);
        const seconds = newTime / 1000;

        if (micAudioRef.current) {
          micAudioRef.current.currentTime = seconds;
        }
        if (systemAudioRef.current) {
          systemAudioRef.current.currentTime = seconds;
        }

        setCurrentTime(newTime);
        onTimeUpdate?.(newTime);
      },
      [onTimeUpdate]
    );

    const handleVolumeChange = useCallback(
      (e: React.ChangeEvent<HTMLInputElement>) => {
        const newVolume = Number(e.target.value);
        setVolume(newVolume);
        setIsMuted(newVolume === 0);

        if (micAudioRef.current) {
          micAudioRef.current.volume = newVolume;
        }
        if (systemAudioRef.current) {
          systemAudioRef.current.volume = newVolume;
        }
      },
      []
    );

    const toggleMute = useCallback(() => {
      const newMuted = !isMuted;
      setIsMuted(newMuted);

      const newVolume = newMuted ? 0 : volume || 1;

      if (micAudioRef.current) {
        micAudioRef.current.volume = newVolume;
      }
      if (systemAudioRef.current) {
        systemAudioRef.current.volume = newVolume;
      }

      if (!newMuted && volume === 0) {
        setVolume(1);
      }
    }, [isMuted, volume]);

    // Handle time update from the active (master) audio element
    const handleTimeUpdate = useCallback(
      (e: React.SyntheticEvent<HTMLAudioElement>) => {
        const master =
          activeTrack === 'mic'
            ? micAudioRef.current
            : activeTrack === 'system'
              ? systemAudioRef.current
              : micAudioRef.current ?? systemAudioRef.current;

        if (!master || e.currentTarget !== master) return;

        const timeMs = master.currentTime * 1000;
        setCurrentTime(timeMs);
        onTimeUpdate?.(timeMs);
      },
      [activeTrack, onTimeUpdate]
    );

    // Handle metadata loaded
    const handleLoadedMetadata = useCallback(
      (e: React.SyntheticEvent<HTMLAudioElement>) => {
        const audio = e.currentTarget;

        if (!durationMs && Number.isFinite(audio.duration)) {
          setDuration((prev) => Math.max(prev, audio.duration * 1000));
        }

        setIsLoaded(true);
      },
      [durationMs]
    );

    // Handle audio ended
    const handleEnded = useCallback(
      (e: React.SyntheticEvent<HTMLAudioElement>) => {
        const master =
          activeTrack === 'mic'
            ? micAudioRef.current
            : activeTrack === 'system'
              ? systemAudioRef.current
              : micAudioRef.current ?? systemAudioRef.current;

        if (!master || e.currentTarget !== master) return;

        if (micAudioRef.current) {
          micAudioRef.current.pause();
        }
        if (systemAudioRef.current) {
          systemAudioRef.current.pause();
        }

        setIsPlaying(false);
        setCurrentTime(0);
        if (micAudioRef.current) {
          micAudioRef.current.currentTime = 0;
        }
        if (systemAudioRef.current) {
          systemAudioRef.current.currentTime = 0;
        }
      },
      [activeTrack]
    );

    // Sync volume when track changes
    useEffect(() => {
      const vol = isMuted ? 0 : volume;
      if (micAudioRef.current) {
        micAudioRef.current.volume = activeTrack === 'system' ? 0 : vol;
      }
      if (systemAudioRef.current) {
        systemAudioRef.current.volume = activeTrack === 'mic' ? 0 : vol;
      }
    }, [activeTrack, volume, isMuted]);

    // Update duration from prop
    useEffect(() => {
      if (durationMs) {
        setDuration(durationMs);
      }
    }, [durationMs]);

    // No audio files available
    if (!micUrl && !systemUrl) {
      return (
        <div className="bg-gray-100 dark:bg-gray-800 rounded-lg p-4 text-center text-gray-500 dark:text-gray-400">
          No audio available for this meeting
        </div>
      );
    }

    return (
      <div className="bg-gray-100 dark:bg-gray-800 rounded-lg p-4 space-y-3">
        {/* Hidden audio elements */}
        {micUrl && (
          <audio
            ref={micAudioRef}
            src={micUrl}
            onTimeUpdate={handleTimeUpdate}
            onLoadedMetadata={handleLoadedMetadata}
            onEnded={handleEnded}
            preload="metadata"
          />
        )}
        {systemUrl && (
          <audio
            ref={systemAudioRef}
            src={systemUrl}
            onTimeUpdate={handleTimeUpdate}
            onLoadedMetadata={handleLoadedMetadata}
            onEnded={handleEnded}
            preload="metadata"
          />
        )}

        {/* Main controls row */}
        <div className="flex items-center gap-4">
          {/* Play/Pause button */}
          <button
            onClick={togglePlayPause}
            disabled={!isLoaded}
            className="w-10 h-10 flex items-center justify-center rounded-full bg-indigo-500 hover:bg-indigo-600 text-white disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {isPlaying ? <Pause className="w-5 h-5" /> : <Play className="w-5 h-5 ml-0.5" />}
          </button>

          {/* Time display */}
          <div className="text-sm font-mono text-gray-600 dark:text-gray-400 w-32 text-center">
            {formatDuration(currentTime)} / {formatDuration(duration)}
          </div>

          {/* Seek bar */}
          <div className="flex-1">
            <input
              type="range"
              min={0}
              max={duration}
              value={currentTime}
              onChange={handleSeek}
              disabled={!isLoaded}
              className="w-full h-2 bg-gray-300 dark:bg-gray-600 rounded-lg appearance-none cursor-pointer disabled:cursor-not-allowed
                [&::-webkit-slider-thumb]:appearance-none
                [&::-webkit-slider-thumb]:w-4
                [&::-webkit-slider-thumb]:h-4
                [&::-webkit-slider-thumb]:rounded-full
                [&::-webkit-slider-thumb]:bg-indigo-500
                [&::-webkit-slider-thumb]:hover:bg-indigo-600
                [&::-webkit-slider-thumb]:cursor-pointer"
            />
          </div>

          {/* Volume controls */}
          <div className="flex items-center gap-2">
            <button
              onClick={toggleMute}
              className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
            >
              {isMuted || volume === 0 ? (
                <VolumeX className="w-5 h-5" />
              ) : (
                <Volume2 className="w-5 h-5" />
              )}
            </button>
            <input
              type="range"
              min={0}
              max={1}
              step={0.1}
              value={isMuted ? 0 : volume}
              onChange={handleVolumeChange}
              className="w-20 h-1.5 bg-gray-300 dark:bg-gray-600 rounded-lg appearance-none cursor-pointer
                [&::-webkit-slider-thumb]:appearance-none
                [&::-webkit-slider-thumb]:w-3
                [&::-webkit-slider-thumb]:h-3
                [&::-webkit-slider-thumb]:rounded-full
                [&::-webkit-slider-thumb]:bg-gray-500
                [&::-webkit-slider-thumb]:hover:bg-gray-600"
            />
          </div>
        </div>

        {/* Track selector (only if both tracks available) */}
        {micUrl && systemUrl && (
          <div className="flex items-center justify-center gap-2 pt-2 border-t border-gray-200 dark:border-gray-700">
            <span className="text-xs text-gray-500 dark:text-gray-400">Track:</span>
            <div className="flex gap-1">
              {(['mic', 'system', 'both'] as const).map((track) => (
                <button
                  key={track}
                  onClick={() => {
                    setActiveTrack(track);
                    if (isPlaying) {
                      const seconds = currentTime / 1000;
                      if (micAudioRef.current) {
                        micAudioRef.current.currentTime = seconds;
                      }
                      if (systemAudioRef.current) {
                        systemAudioRef.current.currentTime = seconds;
                      }
                      void playAudio(track);
                    }
                  }}
                  className={`
                    text-xs px-2 py-1 rounded transition-colors
                    ${
                      activeTrack === track
                        ? 'bg-indigo-500 text-white'
                        : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-300 dark:hover:bg-gray-600'
                    }
                  `}
                >
                  {track === 'mic' ? 'You' : track === 'system' ? 'Others' : 'Both'}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    );
  }
);
