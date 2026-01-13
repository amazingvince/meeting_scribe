/**
 * Timeline group for organizing meetings by date
 */

/* eslint-disable react-refresh/only-export-components */

import type { ReactNode } from 'react';
import type { Meeting } from '../../types';
import {
  isToday,
  isYesterday,
  isThisWeek,
  isThisMonth,
} from '../../utils/format';

interface TimelineGroupProps {
  label: string;
  children: ReactNode;
}

export function TimelineGroup({ label, children }: TimelineGroupProps) {
  return (
    <div className="space-y-2">
      <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
        {label}
      </h2>
      <div className="space-y-2">{children}</div>
    </div>
  );
}

export interface GroupedMeetings {
  today: Meeting[];
  yesterday: Meeting[];
  thisWeek: Meeting[];
  thisMonth: Meeting[];
  older: Meeting[];
}

export function groupMeetingsByDate(meetings: Meeting[]): GroupedMeetings {
  const groups: GroupedMeetings = {
    today: [],
    yesterday: [],
    thisWeek: [],
    thisMonth: [],
    older: [],
  };

  for (const meeting of meetings) {
    const date = new Date(meeting.created_at);

    if (isToday(date)) {
      groups.today.push(meeting);
    } else if (isYesterday(date)) {
      groups.yesterday.push(meeting);
    } else if (isThisWeek(date)) {
      groups.thisWeek.push(meeting);
    } else if (isThisMonth(date)) {
      groups.thisMonth.push(meeting);
    } else {
      groups.older.push(meeting);
    }
  }

  return groups;
}

export const groupLabels: Record<keyof GroupedMeetings, string> = {
  today: 'Today',
  yesterday: 'Yesterday',
  thisWeek: 'This Week',
  thisMonth: 'This Month',
  older: 'Older',
};
