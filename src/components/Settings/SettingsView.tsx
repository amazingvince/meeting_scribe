/**
 * Settings view with model management, audio settings, and storage stats
 */

import { useState, useEffect } from 'react';
import { Info } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { Skeleton } from '../ui/Skeleton';
import { AudioSettings } from './AudioSettings';
import { AppearanceSettings } from './AppearanceSettings';
import { ModelSettings } from './ModelSettings';
import { StorageSettings } from './StorageSettings';
import * as api from '../../lib/tauri';
import type { AppInfo } from '../../lib/tauri';

export function SettingsView() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [isLoadingAppInfo, setIsLoadingAppInfo] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setIsLoadingAppInfo(true);

    api
      .getAppInfo()
      .then((info) => {
        if (!cancelled) setAppInfo(info);
      })
      .catch((e) => {
        console.warn('Failed to load app info:', e);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingAppInfo(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="px-6 py-4 border-b border-border bg-card">
        <h2 className="text-foreground">Settings</h2>
        <p className="text-sm text-muted-foreground mt-0.5">
          Appearance, models, audio capture, and local data
        </p>
      </header>

      {/* Content */}
      <div className="no-scrollbar flex-1 min-h-0 overflow-y-auto">
        <div className="max-w-2xl mx-auto px-6 py-8 pb-20 md:pb-8 space-y-6">
          <AppearanceSettings />
          <ModelSettings />
          <AudioSettings platform={appInfo?.platform} />
          <StorageSettings />

          {/* About */}
          <Card>
            <div className="flex items-center gap-2 mb-4">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent/70">
                <Info className="w-5 h-5 text-muted-foreground" />
              </div>
              <CardTitle>About</CardTitle>
            </div>

            {isLoadingAppInfo && (
              <div className="space-y-2">
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-3/4" />
              </div>
            )}

            {!isLoadingAppInfo && appInfo && (
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Version</span>
                  <span className="text-foreground/80">{appInfo.version}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Platform</span>
                  <span className="text-foreground/80">{appInfo.platform}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Data Directory</span>
                  <span className="max-w-[250px] truncate font-mono text-xs text-foreground/80">
                    {appInfo.data_dir}
                  </span>
                </div>
              </div>
            )}

            {!isLoadingAppInfo && !appInfo && (
              <p className="text-sm text-muted-foreground">
                Unable to load application info.
              </p>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
}
