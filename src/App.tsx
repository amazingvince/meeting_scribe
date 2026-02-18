import { lazy, Suspense, useState, useEffect } from 'react';
import { HashRouter, Routes, Route, useLocation } from 'react-router-dom';
import { AnimatePresence, motion } from 'framer-motion';
import { Layout } from './components/Layout';
import { ToastContainer } from './components/ui/Toast';
import { getAppInfo, type AppInfo } from './lib/tauri';
import { useTheme } from './hooks/useTheme';
import { usePostProcessingCoordinator } from './hooks';
import { modelManager } from './lib/modelManager';

const RecordingView = lazy(() =>
  import('./components/Recording/RecordingView').then((module) => ({
    default: module.RecordingView,
  }))
);
const LibraryView = lazy(() =>
  import('./components/Library/LibraryView').then((module) => ({
    default: module.LibraryView,
  }))
);
const MeetingDetailView = lazy(() =>
  import('./components/Meeting/MeetingDetailView').then((module) => ({
    default: module.MeetingDetailView,
  }))
);
const ChatView = lazy(() =>
  import('./components/Chat/ChatView').then((module) => ({
    default: module.ChatView,
  }))
);
const SettingsView = lazy(() =>
  import('./components/Settings/SettingsView').then((module) => ({
    default: module.SettingsView,
  }))
);

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
        <Suspense
          fallback={
            <div className="h-full min-h-0 flex items-center justify-center text-sm text-muted-foreground">
              Loading view...
            </div>
          }
        >
          <Routes location={location}>
            <Route path="/" element={<RecordingView />} />
            <Route path="/library" element={<LibraryView />} />
            <Route path="/meeting/:id" element={<MeetingDetailView />} />
            <Route path="/chat" element={<ChatView />} />
            <Route path="/settings" element={<SettingsView />} />
          </Routes>
        </Suspense>
      </motion.div>
    </AnimatePresence>
  );
}

function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  useTheme();
  usePostProcessingCoordinator();

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

  useEffect(() => {
    void modelManager.ensureWarmup();
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
