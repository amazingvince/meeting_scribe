/**
 * Toast notification store
 * Manages notification queue and display
 */

import { create } from 'zustand';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
  id: string;
  type: ToastType;
  title: string;
  message?: string;
  duration?: number;
  dismissible?: boolean;
}

interface ToastStore {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, 'id'>) => string;
  removeToast: (id: string) => void;
  clearAll: () => void;

  // Convenience methods
  success: (title: string, message?: string) => string;
  error: (title: string, message?: string) => string;
  warning: (title: string, message?: string) => string;
  info: (title: string, message?: string) => string;
}

function generateId(): string {
  return Math.random().toString(36).substring(2, 15);
}

const DEFAULT_DURATION = 5000;
const TOAST_DEDUPE_WINDOW_MS = 900;
const recentToastFingerprints = new Map<string, number>();

function toastFingerprint(toast: Pick<Toast, 'type' | 'title' | 'message'>): string {
  const title = toast.title.trim().toLowerCase();
  const message = (toast.message ?? '').trim().toLowerCase();
  return `${toast.type}|${title}|${message}`;
}

function pruneFingerprintCache(now: number): void {
  for (const [key, timestamp] of recentToastFingerprints.entries()) {
    if (now - timestamp > TOAST_DEDUPE_WINDOW_MS * 8) {
      recentToastFingerprints.delete(key);
    }
  }
}

export const useToastStore = create<ToastStore>((set, get) => ({
  toasts: [],

  addToast: (toast) => {
    const fingerprint = toastFingerprint(toast);
    const now = Date.now();
    const lastSeen = recentToastFingerprints.get(fingerprint);
    if (lastSeen !== undefined && now - lastSeen < TOAST_DEDUPE_WINDOW_MS) {
      const existing = get().toasts.find(
        (candidate) => toastFingerprint(candidate) === fingerprint
      );
      if (existing) {
        return existing.id;
      }
    }

    const id = generateId();
    const newToast: Toast = {
      id,
      duration: DEFAULT_DURATION,
      dismissible: true,
      ...toast,
    };
    recentToastFingerprints.set(fingerprint, now);
    pruneFingerprintCache(now);

    set((state) => ({
      toasts: [...state.toasts, newToast],
    }));

    // Auto-remove after duration
    if (newToast.duration && newToast.duration > 0) {
      setTimeout(() => {
        get().removeToast(id);
      }, newToast.duration);
    }

    return id;
  },

  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },

  clearAll: () => {
    set({ toasts: [] });
  },

  // Convenience methods
  success: (title, message) => {
    return get().addToast({ type: 'success', title, message });
  },

  error: (title, message) => {
    return get().addToast({
      type: 'error',
      title,
      message,
      duration: 8000, // Errors stay longer
    });
  },

  warning: (title, message) => {
    return get().addToast({ type: 'warning', title, message });
  },

  info: (title, message) => {
    return get().addToast({ type: 'info', title, message });
  },
}));
