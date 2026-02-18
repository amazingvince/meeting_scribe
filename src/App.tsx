import { useState, useEffect } from 'react';
import { HashRouter, Routes, Route, useLocation } from 'react-router-dom';
import { AnimatePresence, motion } from 'framer-motion';
import { Layout } from './components/Layout';
import { RecordingView } from './components/Recording/RecordingView';
import { LibraryView } from './components/Library/LibraryView';
import { MeetingDetailView } from './components/Meeting/MeetingDetailView';
import { ChatView } from './components/Chat/ChatView';
import { SettingsView } from './components/Settings/SettingsView';
import { ToastContainer } from './components/ui/Toast';
import { getAppInfo, type AppInfo } from './lib/tauri';
import { useTheme } from './hooks/useTheme';

function AnimatedRoutes() {
  const location = useLocation();

  return (
    <AnimatePresence mode="sync" initial={false}>
      <motion.div
        key={location.pathname}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.08 }}
        className="h-full min-h-0"
      >
        <Routes location={location}>
          <Route path="/" element={<RecordingView />} />
          <Route path="/library" element={<LibraryView />} />
          <Route path="/meeting/:id" element={<MeetingDetailView />} />
          <Route path="/chat" element={<ChatView />} />
          <Route path="/settings" element={<SettingsView />} />
        </Routes>
      </motion.div>
    </AnimatePresence>
  );
}

function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  useTheme();

  useEffect(() => {
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
        <AnimatedRoutes />
      </Layout>
      <ToastContainer />
    </HashRouter>
  );
}

export default App;
