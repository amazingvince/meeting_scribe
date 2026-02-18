/**
 * Generic Tauri event subscription hook
 */

import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/**
 * Subscribe to a Tauri event with automatic cleanup
 * @param eventName The event name to listen for
 * @param handler Callback function to handle the event
 */
export function useTauriEvent<T>(
  eventName: string,
  handler: (payload: T) => void
): void {
  const handlerRef = useRef(handler);

  // Keep handler ref up to date
  useEffect(() => {
    handlerRef.current = handler;
  }, [handler]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    const setupListener = async () => {
      const dispose = await listen<T>(eventName, (event) => {
        handlerRef.current(event.payload);
      });

      if (cancelled) {
        dispose();
        return;
      }

      unlisten = dispose;
    };

    void setupListener();

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [eventName]);
}
