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
    <div className="border-b border-border bg-card px-4 py-3 md:px-6">
      <div className="mb-2 flex items-center gap-3">
        <Button variant="ghost" size="icon" onClick={onBack} className="h-8 w-8">
          <ArrowLeft className="w-5 h-5" />
        </Button>

        {isEditing ? (
          <div className="flex-1 flex items-center gap-2">
            <Input
              value={editedTitle}
              onChange={(e) => setEditedTitle(e.target.value)}
              onKeyDown={handleKeyDown}
              className="flex-1 text-base font-semibold"
              autoFocus
            />
            <Button
              variant="ghost"
              size="icon"
              onClick={handleEditSave}
              isLoading={isSaving}
              className="h-8 w-8"
            >
              <Check className="w-5 h-5 text-green-600" />
            </Button>
            <Button variant="ghost" size="icon" onClick={handleEditCancel} className="h-8 w-8">
              <X className="w-5 h-5 text-destructive" />
            </Button>
          </div>
        ) : (
          <div className="flex-1 flex items-center gap-2">
            <h1 className="truncate text-base font-semibold text-foreground md:text-lg">
              {meeting.title}
            </h1>
            <Button variant="ghost" size="icon" onClick={handleEditStart} className="h-8 w-8">
              <Edit2 className="w-4 h-4" />
            </Button>
          </div>
        )}

        <StatusBadge status={meeting.status} />
      </div>

      <div className="flex items-center gap-4 text-xs text-muted-foreground md:text-sm">
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
