import { useState, useEffect } from 'react';
import { HashRouter, Routes, Route } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { Layout } from './components/Layout';
import { RecordingView } from './components/Recording/RecordingView';
import { LibraryView } from './components/Library/LibraryView';
import { MeetingDetailView } from './components/Meeting/MeetingDetailView';
import { ChatView } from './components/Chat/ChatView';
import { SettingsView } from './components/Settings/SettingsView';
import { ToastContainer } from './components/ui/Toast';

interface AppInfo {
  version: string;
  data_dir: string;
  platform: string;
}

function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    // Test IPC connection on mount
    invoke<AppInfo>('get_app_info').then(setAppInfo);
  }, []);

  return (
    <HashRouter>
      <Layout appInfo={appInfo}>
        <Routes>
          <Route path="/" element={<RecordingView />} />
          <Route path="/library" element={<LibraryView />} />
          <Route path="/meeting/:id" element={<MeetingDetailView />} />
          <Route path="/chat" element={<ChatView />} />
          <Route path="/settings" element={<SettingsView />} />
        </Routes>
      </Layout>
      <ToastContainer />
    </HashRouter>
  );
}

export default App;
