import { ReactNode, useEffect, useState } from 'react';
import { NavLink } from 'react-router-dom';
import { Mic, Library, MessageSquare, Settings } from 'lucide-react';
import logoMark from '../assets/branding/meeting-scribe-mark.svg';
import { motion } from 'framer-motion';
import clsx from 'clsx';
import { BackgroundTaskPill } from './ui/BackgroundTaskPill';
import { getRecordingState, onRecordingStateChanged } from '../lib/tauri';

interface LayoutProps {
  children: ReactNode;
  appInfo: { version: string; data_dir: string; platform: string } | null;
}

interface NavItemProps {
  to: string;
  icon: ReactNode;
  label: string;
  isRecording?: boolean;
}

const navItems = [
  { to: '/', Icon: Mic, label: 'Record' },
  { to: '/library', Icon: Library, label: 'Library' },
  { to: '/chat', Icon: MessageSquare, label: 'Chat' },
  { to: '/settings', Icon: Settings, label: 'Settings' },
] as const;

function MobileNavItem({ to, icon, label, isRecording = false }: NavItemProps) {
  return (
    <NavLink to={to} className="group relative flex min-h-[52px] items-center justify-center">
      {({ isActive }) => (
        <div
          className={clsx(
            'relative flex h-full w-full flex-col items-center justify-center gap-1 rounded-lg px-2 py-1 text-[11px] font-medium transition-colors duration-150',
            isActive
              ? isRecording
                ? 'text-red-600 dark:text-red-300'
                : 'text-foreground'
              : isRecording
                ? 'text-red-500 dark:text-red-300'
                : 'text-muted-foreground group-hover:text-foreground'
          )}
        >
          {isActive && (
            <motion.span
              layoutId="main-nav-active-pill"
              className="absolute inset-0 rounded-lg bg-accent shadow-sm"
              transition={{ type: 'spring', stiffness: 430, damping: 34, mass: 0.7 }}
            />
          )}

          <span
            className={clsx(
              'relative z-10 flex h-7 w-7 items-center justify-center rounded-lg transition-colors duration-150',
              isActive
                ? isRecording
                  ? 'bg-red-500/15 text-red-600 dark:bg-red-500/20 dark:text-red-300'
                  : 'text-foreground'
                : 'bg-transparent'
            )}
          >
            <span className={clsx(isRecording && label === 'Record' && 'animate-pulse')}>
              {icon}
            </span>
            {isRecording && label === 'Record' && (
              <span className="absolute -right-0.5 -top-0.5 h-2 w-2 rounded-full bg-red-500" />
            )}
          </span>

          <span className="relative z-10">{label}</span>
        </div>
      )}
    </NavLink>
  );
}

function DesktopNavItem({ to, icon, label, isRecording = false }: NavItemProps) {
  return (
    <NavLink to={to} className="group">
      {({ isActive }) => (
        <div
          className={clsx(
            'flex h-14 w-14 flex-col items-center justify-center rounded-xl text-[10px] font-medium transition-colors duration-150',
            isActive
              ? 'bg-accent text-foreground'
              : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
          )}
        >
          <div className="relative mb-0.5">
            <span
              className={clsx(
                'inline-flex h-5 w-5 items-center justify-center',
                isRecording && label === 'Record' && 'animate-pulse'
              )}
            >
              {icon}
            </span>
            {isRecording && label === 'Record' && (
              <span className="absolute -right-1 -top-1 h-2 w-2 rounded-full bg-red-500" />
            )}
          </div>
          <span>{label}</span>
        </div>
      )}
    </NavLink>
  );
}

export function Layout({ children, appInfo }: LayoutProps) {
  const [isRecordingActive, setIsRecordingActive] = useState(false);

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
    <div className="flex h-screen min-h-0 bg-background">
      <BackgroundTaskPill />

      {/* Desktop sidebar */}
      <aside
        className="hidden w-[72px] min-w-[72px] flex-col items-center border-r border-border bg-card py-4 md:flex"
        title={appInfo ? `v${appInfo.version} • ${appInfo.platform}` : undefined}
      >
        <img src={logoMark} alt="Meeting Scribe" className="mb-6 h-10 w-10 rounded-lg" />
        <nav className="flex flex-1 flex-col items-center gap-1">
          {navItems.map((item) => (
            <DesktopNavItem
              key={item.to}
              to={item.to}
              icon={<item.Icon size={18} />}
              label={item.label}
              isRecording={item.to === '/' ? isRecordingActive : false}
            />
          ))}
        </nav>
      </aside>

      {/* Main content — each page owns its own padding, scrolling, and max-width */}
      <main className="flex-1 min-h-0 min-w-0 overflow-hidden">
        {children}
      </main>

      {/* Mobile bottom nav */}
      <nav className="pointer-events-none fixed inset-x-0 bottom-3 z-40 px-3 md:hidden">
        <div
          className="pointer-events-auto mx-auto w-full max-w-lg rounded-xl border border-border bg-card/95 p-1.5 shadow-float backdrop-blur"
          title={appInfo ? `v${appInfo.version} • ${appInfo.platform}` : undefined}
        >
          <div className="grid grid-cols-4 gap-1">
            {navItems.map((item) => (
              <MobileNavItem
                key={item.to}
                to={item.to}
                icon={<item.Icon size={20} />}
                label={item.label}
                isRecording={item.to === '/' ? isRecordingActive : false}
              />
            ))}
          </div>
        </div>
      </nav>
    </div>
  );
}
