/**
 * Storage settings section
 */

import { useState, useEffect } from 'react';
import { HardDrive, Database, FileAudio, FolderOpen, Cpu } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { ProgressBar } from '../ui/Progress';
import { Skeleton } from '../ui/Skeleton';
import { formatBytes } from '../../utils/format';
import * as api from '../../lib/tauri';
import type { DatabaseStats, StorageStats } from '../../types';

export function StorageSettings() {
  const [dbStats, setDbStats] = useState<DatabaseStats | null>(null);
  const [storageStats, setStorageStats] = useState<StorageStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadStats = async () => {
      try {
        const [db, storage] = await Promise.all([
          api.getDatabaseStats(),
          api.getStorageStats(),
        ]);
        setDbStats(db);
        setStorageStats(storage);
      } catch (e) {
        console.error('Failed to load stats:', e);
      } finally {
        setIsLoading(false);
      }
    };

    loadStats();
  }, []);

  if (isLoading) {
    return (
      <Card>
        <CardTitle>Storage</CardTitle>
        <div className="mt-4 space-y-3">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-4 w-1/2" />
        </div>
      </Card>
    );
  }

  return (
    <Card>
      <div className="flex items-center gap-2 mb-4">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent/70">
          <HardDrive className="w-5 h-5 text-muted-foreground" />
        </div>
        <CardTitle>Storage Usage</CardTitle>
      </div>

      {storageStats && (
        <div className="space-y-4">
          {/* Total usage */}
          <div>
            <div className="flex justify-between text-sm mb-1">
              <span className="text-muted-foreground">
                Total Storage
              </span>
              <span className="font-medium text-foreground">
                {formatBytes(storageStats.total_bytes)}
              </span>
            </div>
            <ProgressBar value={storageStats.total_bytes} max={10 * 1024 * 1024 * 1024} size="sm" />
          </div>

          {/* Breakdown */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-muted-foreground">
                <Database className="w-4 h-4" />
                Database
              </div>
              <span className="text-foreground">
                {formatBytes(storageStats.database_bytes)}
              </span>
            </div>

            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-muted-foreground">
                <FolderOpen className="w-4 h-4" />
                Vector Store
              </div>
              <span className="text-foreground">
                {formatBytes(storageStats.vectors_bytes)}
              </span>
            </div>

            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-muted-foreground">
                <FileAudio className="w-4 h-4" />
                Audio Files
              </div>
              <span className="text-foreground">
                {formatBytes(storageStats.audio_bytes)}
              </span>
            </div>

            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-muted-foreground">
                <Cpu className="w-4 h-4" />
                ML Models
              </div>
              <span className="text-foreground">
                {formatBytes(storageStats.models_bytes)}
              </span>
            </div>
          </div>
        </div>
      )}

      {dbStats && (
        <div className="mt-6 border-t border-border pt-4">
          <h4 className="mb-3 text-sm font-medium text-foreground">
            Database Statistics
          </h4>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">Meetings</span>
              <p className="font-medium text-foreground">
                {dbStats.meeting_count}
              </p>
            </div>
            <div>
              <span className="text-muted-foreground">
                Transcript Segments
              </span>
              <p className="font-medium text-foreground">
                {dbStats.segment_count.toLocaleString()}
              </p>
            </div>
            <div>
              <span className="text-muted-foreground">Notes</span>
              <p className="font-medium text-foreground">
                {dbStats.note_count}
              </p>
            </div>
            <div>
              <span className="text-muted-foreground">
                Total Duration
              </span>
              <p className="font-medium text-foreground">
                {Math.round(dbStats.total_duration_ms / 60000)} min
              </p>
            </div>
          </div>
        </div>
      )}
    </Card>
  );
}
