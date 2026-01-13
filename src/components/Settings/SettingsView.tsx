/**
 * Settings view with model management, audio settings, and storage stats
 */

import { useState, useEffect } from 'react';
import { Info } from 'lucide-react';
import { Card, CardTitle } from '../ui/Card';
import { ModelSettings } from './ModelSettings';
import { StorageSettings } from './StorageSettings';
import * as api from '../../lib/tauri';
import type { AppInfo } from '../../lib/tauri';

export function SettingsView() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    api.getAppInfo().then(setAppInfo);
  }, []);

  return (
    <div className="space-y-6 pb-8">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Settings
      </h1>

      {/* Model Settings */}
      <section>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
          Models
        </h2>
        <ModelSettings />
      </section>

      {/* Storage */}
      <section>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
          Storage
        </h2>
        <StorageSettings />
      </section>

      {/* App Info */}
      <section>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
          About
        </h2>
        <Card>
          <div className="flex items-center gap-2 mb-4">
            <Info className="w-5 h-5 text-gray-500" />
            <CardTitle>Application Info</CardTitle>
          </div>

          {appInfo && (
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
        </Card>
      </section>
    </div>
  );
}
