/**
 * Meeting header with title, date, duration, and actions
 */

import { useState, useCallback } from 'react';
import { ArrowLeft, Edit2, Check, X, Calendar, Clock } from 'lucide-react';
import type { Meeting } from '../../types';
import { Button } from '../ui/Button';
import { Input } from '../ui/Input';
import { StatusBadge } from '../ui/Badge';
import { formatDate, formatDuration, formatTime } from '../../utils/format';

interface MeetingHeaderProps {
  meeting: Meeting;
  onBack: () => void;
  onUpdateTitle: (title: string) => Promise<void>;
}

export function MeetingHeader({
  meeting,
  onBack,
  onUpdateTitle,
}: MeetingHeaderProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editedTitle, setEditedTitle] = useState(meeting.title);
  const [isSaving, setIsSaving] = useState(false);

  const handleEditStart = useCallback(() => {
    setEditedTitle(meeting.title);
    setIsEditing(true);
  }, [meeting.title]);

  const handleEditCancel = useCallback(() => {
    setEditedTitle(meeting.title);
    setIsEditing(false);
  }, [meeting.title]);

  const handleEditSave = useCallback(async () => {
    if (editedTitle.trim() === '') return;
    if (editedTitle === meeting.title) {
      setIsEditing(false);
      return;
    }

    setIsSaving(true);
    await onUpdateTitle(editedTitle.trim());
    setIsSaving(false);
    setIsEditing(false);
  }, [editedTitle, meeting.title, onUpdateTitle]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        handleEditSave();
      } else if (e.key === 'Escape') {
        handleEditCancel();
      }
    },
    [handleEditSave, handleEditCancel]
  );

  return (
    <div className="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-6 py-4">
      <div className="flex items-center gap-4 mb-4">
        <Button variant="ghost" size="icon" onClick={onBack}>
          <ArrowLeft className="w-5 h-5" />
        </Button>

        {isEditing ? (
          <div className="flex-1 flex items-center gap-2">
            <Input
              value={editedTitle}
              onChange={(e) => setEditedTitle(e.target.value)}
              onKeyDown={handleKeyDown}
              className="flex-1 text-xl font-bold"
              autoFocus
            />
            <Button
              variant="ghost"
              size="icon"
              onClick={handleEditSave}
              isLoading={isSaving}
            >
              <Check className="w-5 h-5 text-green-600" />
            </Button>
            <Button variant="ghost" size="icon" onClick={handleEditCancel}>
              <X className="w-5 h-5 text-red-600" />
            </Button>
          </div>
        ) : (
          <div className="flex-1 flex items-center gap-2">
            <h1 className="text-xl font-bold text-gray-900 dark:text-gray-100 truncate">
              {meeting.title}
            </h1>
            <Button variant="ghost" size="icon" onClick={handleEditStart}>
              <Edit2 className="w-4 h-4" />
            </Button>
          </div>
        )}

        <StatusBadge status={meeting.status} />
      </div>

      <div className="flex items-center gap-6 text-sm text-gray-500 dark:text-gray-400">
        <span className="flex items-center gap-2">
          <Calendar className="w-4 h-4" />
          {formatDate(new Date(meeting.created_at))} at{' '}
          {formatTime(new Date(meeting.created_at))}
        </span>
        {meeting.duration_ms && (
          <span className="flex items-center gap-2">
            <Clock className="w-4 h-4" />
            {formatDuration(meeting.duration_ms)}
          </span>
        )}
      </div>
    </div>
  );
}
