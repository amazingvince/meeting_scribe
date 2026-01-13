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
        <HardDrive className="w-5 h-5 text-orange-500" />
        <CardTitle>Storage Usage</CardTitle>
      </div>

      {storageStats && (
        <div className="space-y-4">
          {/* Total usage */}
          <div>
            <div className="flex justify-between text-sm mb-1">
              <span className="text-gray-500 dark:text-gray-400">
                Total Storage
              </span>
              <span className="font-medium text-gray-900 dark:text-gray-100">
                {formatBytes(storageStats.total_bytes)}
              </span>
            </div>
            <ProgressBar value={storageStats.total_bytes} max={10 * 1024 * 1024 * 1024} size="sm" />
          </div>

          {/* Breakdown */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-gray-600 dark:text-gray-400">
                <Database className="w-4 h-4" />
                Database
              </div>
              <span className="text-gray-900 dark:text-gray-100">
                {formatBytes(storageStats.database_bytes)}
              </span>
            </div>

            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-gray-600 dark:text-gray-400">
                <FolderOpen className="w-4 h-4" />
                Vector Store
              </div>
              <span className="text-gray-900 dark:text-gray-100">
                {formatBytes(storageStats.vectors_bytes)}
              </span>
            </div>

            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-gray-600 dark:text-gray-400">
                <FileAudio className="w-4 h-4" />
                Audio Files
              </div>
              <span className="text-gray-900 dark:text-gray-100">
                {formatBytes(storageStats.audio_bytes)}
              </span>
            </div>

            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2 text-gray-600 dark:text-gray-400">
                <Cpu className="w-4 h-4" />
                ML Models
              </div>
              <span className="text-gray-900 dark:text-gray-100">
                {formatBytes(storageStats.models_bytes)}
              </span>
            </div>
          </div>
        </div>
      )}

      {dbStats && (
        <div className="mt-6 pt-4 border-t border-gray-200 dark:border-gray-700">
          <h4 className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-3">
            Database Statistics
          </h4>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <span className="text-gray-500 dark:text-gray-400">Meetings</span>
              <p className="font-medium text-gray-900 dark:text-gray-100">
                {dbStats.meeting_count}
              </p>
            </div>
            <div>
              <span className="text-gray-500 dark:text-gray-400">
                Transcript Segments
              </span>
              <p className="font-medium text-gray-900 dark:text-gray-100">
                {dbStats.segment_count.toLocaleString()}
              </p>
            </div>
            <div>
              <span className="text-gray-500 dark:text-gray-400">Notes</span>
              <p className="font-medium text-gray-900 dark:text-gray-100">
                {dbStats.note_count}
              </p>
            </div>
            <div>
              <span className="text-gray-500 dark:text-gray-400">
                Total Duration
              </span>
              <p className="font-medium text-gray-900 dark:text-gray-100">
                {Math.round(dbStats.total_duration_ms / 60000)} min
              </p>
            </div>
          </div>
        </div>
      )}
    </Card>
  );
}
