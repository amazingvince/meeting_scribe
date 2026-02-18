/**
 * Toast notification component
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  CheckCircle,
  XCircle,
  AlertTriangle,
  Info,
  X,
} from 'lucide-react';
import { AnimatePresence, motion } from 'framer-motion';
import { useToastStore, type Toast as ToastType, type ToastType as ToastVariant } from '../../stores/toastStore';

const icons: Record<ToastVariant, typeof CheckCircle> = {
  success: CheckCircle,
  error: XCircle,
  warning: AlertTriangle,
  info: Info,
};

const styles: Record<ToastVariant, string> = {
  success: 'bg-success/10 border-success/20',
  error: 'bg-destructive/10 border-destructive/20',
  warning: 'bg-warning/10 border-warning/20',
  info: 'bg-info/10 border-info/20',
};

const iconStyles: Record<ToastVariant, string> = {
  success: 'text-success',
  error: 'text-destructive',
  warning: 'text-warning',
  info: 'text-info',
};

const countdownStyles: Record<ToastVariant, string> = {
  success: 'bg-success',
  error: 'bg-destructive',
  warning: 'bg-warning',
  info: 'bg-info',
};

interface ToastItemProps {
  toast: ToastType;
  onDismiss: () => void;
}

function ToastItem({ toast, onDismiss }: ToastItemProps) {
  const Icon = icons[toast.type];
  const duration = toast.duration ?? 5000;

  const [paused, setPaused] = useState(false);
  const remainingRef = useRef(duration);
  const startTimeRef = useRef(Date.now());
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const startTimer = useCallback(() => {
    clearTimer();
    if (remainingRef.current <= 0) return;
    startTimeRef.current = Date.now();
    timerRef.current = setTimeout(() => onDismissRef.current(), remainingRef.current);
  }, [clearTimer]);

  useEffect(() => {
    if (duration > 0) {
      startTimer();
    }
    return clearTimer;
  }, [duration, startTimer, clearTimer]);

  const handleMouseEnter = useCallback(() => {
    if (duration <= 0) return;
    setPaused(true);
    clearTimer();
    remainingRef.current = Math.max(
      0,
      remainingRef.current - (Date.now() - startTimeRef.current)
    );
  }, [duration, clearTimer]);

  const handleMouseLeave = useCallback(() => {
    if (duration <= 0) return;
    setPaused(false);
    startTimer();
  }, [duration, startTimer]);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, x: 16, scale: 0.985 }}
      animate={{ opacity: 1, x: 0, scale: 1, transition: { duration: 0.2, ease: [0.22, 1, 0.36, 1] } }}
      exit={{ opacity: 0, x: 16, scale: 0.985, transition: { duration: 0.1, ease: [0.22, 1, 0.36, 1] } }}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      className={`
        relative flex items-start gap-3 p-4 overflow-hidden
        border rounded-lg shadow-float backdrop-blur-sm
        ${styles[toast.type]}
      `}
    >
      <Icon className={`w-5 h-5 flex-shrink-0 ${iconStyles[toast.type]}`} />
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-foreground">
          {toast.title}
        </p>
        {toast.message && (
          <p className="mt-1 text-sm text-muted-foreground">
            {toast.message}
          </p>
        )}
      </div>
      {toast.dismissible && (
        <button
          onClick={onDismiss}
          className="flex-shrink-0 text-muted-foreground hover:text-foreground"
        >
          <X className="w-4 h-4" />
        </button>
      )}

      {duration > 0 && (
        <div
          className={`absolute bottom-0 left-0 right-0 h-0.5 origin-left animate-countdown ${countdownStyles[toast.type]}`}
          style={{
            animationDuration: `${duration}ms`,
            animationPlayState: paused ? 'paused' : 'running',
          }}
        />
      )}
    </motion.div>
  );
}

export function ToastContainer() {
  const { toasts, removeToast } = useToastStore();

  if (toasts.length === 0) return null;

  return (
    <div
      role="log"
      aria-live="polite"
      aria-label="Notifications"
      className="fixed top-4 right-4 z-50 flex flex-col gap-2 w-full max-w-sm"
    >
      <AnimatePresence initial={false}>
        {toasts.map((toast) => (
          <ToastItem
            key={toast.id}
            toast={toast}
            onDismiss={() => removeToast(toast.id)}
          />
        ))}
      </AnimatePresence>
    </div>
  );
}
