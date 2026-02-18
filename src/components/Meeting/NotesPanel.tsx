/**
 * Notes panel for user notes
 * Fetches and saves notes via Tauri API
 */

import { useState, useCallback, useEffect } from 'react';
import { Save, FileText, Loader2, Check } from 'lucide-react';
import { Button } from '../ui/Button';
import { Textarea } from '../ui/Input';
import { useToastStore } from '../../stores';
import * as api from '../../lib/tauri';

interface NotesPanelProps {
  meetingId: string;
}

export function NotesPanel({ meetingId }: NotesPanelProps) {
  const toast = useToastStore();
  const [notes, setNotes] = useState('');
  const [originalNotes, setOriginalNotes] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  const hasChanges = notes !== originalNotes;

  // Fetch notes on mount
  useEffect(() => {
    const fetchNotes = async () => {
      setIsLoading(true);
      try {
        const note = await api.getNote(meetingId);
        const content = note?.content ?? '';
        setNotes(content);
        setOriginalNotes(content);
      } catch (e) {
        console.error('Failed to fetch notes:', e);
        // Don't show error toast for missing notes - just start with empty
      } finally {
        setIsLoading(false);
      }
    };

    fetchNotes();
  }, [meetingId]);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setNotes(e.target.value);
    },
    []
  );

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    try {
      await api.saveNote(meetingId, notes);
      setOriginalNotes(notes);
      toast.success('Notes saved');
    } catch (e) {
      toast.error(
        'Failed to save notes',
        e instanceof Error ? e.message : String(e)
      );
    } finally {
      setIsSaving(false);
    }
  }, [meetingId, notes, toast]);

  return (
    <div className="p-4 h-full flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h3 className="font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
          <FileText className="w-5 h-5 text-blue-500" />
          Notes
        </h3>
        <Button
          variant="secondary"
          size="sm"
          onClick={handleSave}
          isLoading={isSaving}
          disabled={!hasChanges || isLoading}
        >
          <Save className="w-4 h-4" />
          Save
        </Button>
      </div>

      <Textarea
        value={notes}
        onChange={handleChange}
        placeholder={
          isLoading
            ? 'Loading notes...'
            : 'Add your notes about this meeting...'
        }
        className="flex-1 min-h-[300px] resize-none"
        disabled={isLoading}
      />

      <div className="mt-2 min-h-4">
        {isSaving ? (
          <p className="text-xs text-indigo-600 dark:text-indigo-400 flex items-center gap-1">
            <Loader2 className="w-3 h-3 animate-spin" />
            Saving changes...
          </p>
        ) : hasChanges ? (
          <p className="text-xs text-amber-600 dark:text-amber-400">
            You have unsaved changes
          </p>
        ) : !isLoading ? (
          <p className="text-xs text-green-600 dark:text-green-400 flex items-center gap-1">
            <Check className="w-3 h-3" />
            All changes saved
          </p>
        ) : null}
      </div>
    </div>
  );
}
