import { ReactNode } from 'react';
import { NavLink } from 'react-router-dom';
import { Mic, Library, MessageSquare, Settings } from 'lucide-react';
import clsx from 'clsx';

interface LayoutProps {
  children: ReactNode;
  appInfo: { version: string; data_dir: string; platform: string } | null;
}

export function Layout({ children, appInfo }: LayoutProps) {
  return (
    <div className="flex flex-col h-screen bg-surface-50 dark:bg-surface-950">
      {/* Main content */}
      <main className="flex-1 overflow-auto p-6">
        {children}
      </main>

      {/* Bottom navigation */}
      <nav className="border-t border-gray-200 dark:border-gray-800 bg-white dark:bg-surface-900">
        <div className="flex justify-around py-2">
          <NavItem to="/" icon={<Mic size={24} />} label="Record" />
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
}

function NavItem({ to, icon, label }: NavItemProps) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        clsx(
          'flex flex-col items-center gap-1 px-4 py-2 rounded-lg transition-colors',
          isActive
            ? 'text-primary-600 dark:text-primary-400'
            : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
        )
      }
    >
      {icon}
      <span className="text-xs font-medium">{label}</span>
    </NavLink>
  );
}
