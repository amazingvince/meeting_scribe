import { useState, useEffect } from 'react';
import { HashRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Layout';
import { RecordingView } from './components/Recording/RecordingView';
import { LibraryView } from './components/Library/LibraryView';
import { MeetingDetailView } from './components/Meeting/MeetingDetailView';
import { ChatView } from './components/Chat/ChatView';
import { SettingsView } from './components/Settings/SettingsView';
import { ToastContainer } from './components/ui/Toast';
import { getAppInfo, type AppInfo } from './lib/tauri';
import { useTheme } from './hooks/useTheme';

function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  useTheme();

  useEffect(() => {
    // Test IPC connection on mount
    let cancelled = false;

    getAppInfo()
      .then((info) => {
        if (!cancelled) setAppInfo(info);
      })
      .catch((e) => {
        console.warn('Failed to get app info:', e);
        if (!cancelled) setAppInfo(null);
      });

    return () => {
      cancelled = true;
    };
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
