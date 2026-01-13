/**
 * Transcript panel with scrollable transcript segments
 */

import { useMemo } from 'react';
import type { TranscriptSegment } from '../../types';
import { NoTranscriptEmpty } from '../ui/EmptyState';
import { SkeletonText } from '../ui/Skeleton';
import { formatDuration } from '../../utils/format';

interface TranscriptPanelProps {
  segments: TranscriptSegment[];
  isLoading: boolean;
  onTimestampClick?: (ms: number) => void;
}

export function TranscriptPanel({
  segments,
  isLoading,
  onTimestampClick,
}: TranscriptPanelProps) {
  // Group consecutive segments by speaker
  const groupedSegments = useMemo(() => {
    if (segments.length === 0) return [];

    const groups: {
      speaker: string;
      startMs: number;
      segments: TranscriptSegment[];
    }[] = [];

    let currentGroup: (typeof groups)[0] | null = null;

    for (const segment of segments) {
      if (!currentGroup || currentGroup.speaker !== segment.speaker) {
        if (currentGroup) {
          groups.push(currentGroup);
        }
        currentGroup = {
          speaker: segment.speaker,
          startMs: segment.start_ms,
          segments: [segment],
        };
      } else {
        currentGroup.segments.push(segment);
      }
    }

    if (currentGroup) {
      groups.push(currentGroup);
    }

    return groups;
  }, [segments]);

  if (isLoading) {
    return (
      <div className="p-4 space-y-6">
        <SkeletonText lines={4} />
        <SkeletonText lines={3} />
        <SkeletonText lines={5} />
      </div>
    );
  }

  if (segments.length === 0) {
    return <NoTranscriptEmpty />;
  }

  return (
    <div className="p-4 space-y-6 overflow-y-auto">
      {groupedSegments.map((group, idx) => (
        <div key={idx} className="space-y-2">
          <div className="flex items-center gap-3">
            <span
              className={`
                text-sm font-medium px-2 py-0.5 rounded
                ${
                  group.speaker === 'You'
                    ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-400'
                    : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300'
                }
              `}
            >
              {group.speaker}
            </span>
            <button
              className="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 font-mono"
              onClick={() => onTimestampClick?.(group.startMs)}
            >
              {formatDuration(group.startMs)}
            </button>
          </div>
          <div className="text-gray-700 dark:text-gray-300 leading-relaxed">
            {group.segments.map((segment, segIdx) => (
              <span
                key={segIdx}
                className="hover:bg-yellow-100 dark:hover:bg-yellow-900/20 cursor-pointer rounded px-0.5"
                onClick={() => onTimestampClick?.(segment.start_ms)}
              >
                {segment.text}{' '}
              </span>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
