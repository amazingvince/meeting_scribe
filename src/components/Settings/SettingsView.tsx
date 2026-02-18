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
    <div className="space-y-6 pb-8">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Settings
      </h1>

      <AppearanceSettings />
      <ModelSettings />
      <AudioSettings platform={appInfo?.platform} />
      <StorageSettings />

      {/* App Info */}
      <section>
        <Card>
          <div className="flex items-center gap-2 mb-4">
            <Info className="w-5 h-5 text-gray-500" />
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
                <span className="text-gray-500 dark:text-gray-400">Version</span>
                <span className="text-gray-900 dark:text-gray-100">
                  {appInfo.version}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">Platform</span>
                <span className="text-gray-900 dark:text-gray-100">
                  {appInfo.platform}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">
                  Data Directory
                </span>
                <span className="text-gray-900 dark:text-gray-100 font-mono text-xs truncate max-w-[250px]">
                  {appInfo.data_dir}
                </span>
              </div>
            </div>
          )}

          {!isLoadingAppInfo && !appInfo && (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Unable to load application info.
            </p>
          )}
        </Card>
      </section>
    </div>
  );
}
