import { ReactNode, useEffect, useState } from 'react';
import { NavLink, useLocation } from 'react-router-dom';
import { Mic, Library, MessageSquare, Settings } from 'lucide-react';
import clsx from 'clsx';
import { BackgroundTaskPill } from './ui/BackgroundTaskPill';
import { getRecordingState, onRecordingStateChanged } from '../lib/tauri';

interface LayoutProps {
  children: ReactNode;
  appInfo: { version: string; data_dir: string; platform: string } | null;
}

export function Layout({ children, appInfo }: LayoutProps) {
  const [isRecordingActive, setIsRecordingActive] = useState(false);
  const location = useLocation();
  const isRecordRoute = location.pathname === '/';

  useEffect(() => {
    let cancelled = false;
    let intervalId: number | null = null;
    let unlisten: (() => void) | null = null;

    const refreshState = async () => {
      try {
        const state = await getRecordingState();
        if (!cancelled) {
          setIsRecordingActive(state.state === 'Recording');
        }
      } catch {
        // Ignore transient IPC errors.
      }
    };

    void refreshState();
    intervalId = window.setInterval(() => {
      void refreshState();
    }, 2500);

    void onRecordingStateChanged((event) => {
      if (!cancelled) {
        setIsRecordingActive(event.state === 'Recording');
      }
    }).then((dispose) => {
      if (cancelled) {
        dispose();
      } else {
        unlisten = dispose;
      }
    });

    return () => {
      cancelled = true;
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return (
    <div className="flex flex-col h-screen bg-surface-50 dark:bg-surface-950">
      <BackgroundTaskPill />

      {/* Main content */}
      <main
        className={clsx(
          'flex-1 p-6',
          isRecordRoute ? 'overflow-hidden' : 'overflow-auto'
        )}
      >
        {children}
      </main>

      {/* Bottom navigation */}
      <nav className="border-t border-gray-200 dark:border-gray-800 bg-white dark:bg-surface-900">
        <div className="flex justify-around py-2">
          <NavItem to="/" icon={<Mic size={24} />} label="Record" isRecording={isRecordingActive} />
          <NavItem to="/library" icon={<Library size={24} />} label="Library" />
          <NavItem to="/chat" icon={<MessageSquare size={24} />} label="Chat" />
          <NavItem to="/settings" icon={<Settings size={24} />} label="Settings" />
        </div>

        {/* Version info (dev only) */}
        {appInfo && (
          <div className="text-center text-xs text-gray-400 pb-2">
            v{appInfo.version} | {appInfo.platform}
          </div>
        )}
      </nav>
    </div>
  );
}

interface NavItemProps {
  to: string;
  icon: ReactNode;
  label: string;
  isRecording?: boolean;
}

function NavItem({ to, icon, label, isRecording = false }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        clsx(
          'flex flex-col items-center gap-1 px-4 py-2 rounded-lg transition-colors',
          isActive
            ? isRecording
              ? 'text-red-600 dark:text-red-400'
              : 'text-primary-600 dark:text-primary-400'
            : isRecording
              ? 'text-red-500 hover:text-red-600 dark:text-red-400 dark:hover:text-red-300'
              : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
        )
      }
    >
      <div className="relative">
        <span className={clsx(isRecording && 'animate-pulse')}>{icon}</span>
        {isRecording && (
          <span className="absolute -top-1 -right-1 w-2 h-2 rounded-full bg-red-500" />
        )}
      </div>
      <span className="text-xs font-medium">
        {label}
        {isRecording && label === 'Record' ? ' Live' : ''}
      </span>
    </NavLink>
  );
}
