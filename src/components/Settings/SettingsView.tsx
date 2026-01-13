import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AppInfo {
  version: string;
  data_dir: string;
  platform: string;
}

export function SettingsView() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>('get_app_info').then(setAppInfo);
  }, []);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Settings</h1>

      {appInfo && (
        <div className="card p-6 space-y-4">
          <h2 className="font-semibold">Application Info</h2>

          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="text-gray-500">Version</div>
            <div>{appInfo.version}</div>

            <div className="text-gray-500">Platform</div>
            <div>{appInfo.platform}</div>

            <div className="text-gray-500">Data Directory</div>
            <div className="font-mono text-xs break-all">{appInfo.data_dir}</div>
          </div>
        </div>
      )}

      <div className="card p-6">
        <h2 className="font-semibold mb-4">Models</h2>
        <p className="text-gray-500 text-sm">
          Model management will be added in <code className="bg-gray-100 dark:bg-gray-800 px-1 rounded">04-transcription-engine.md</code>
        </p>
      </div>
    </div>
  );
}
