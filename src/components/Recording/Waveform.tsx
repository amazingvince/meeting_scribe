import { useEffect, useRef } from 'react';

interface WaveformProps {
  samples: number[];
  rms: number;
  color: string;
  height?: number;
  label?: string;
}

export function Waveform({
  samples,
  rms,
  color,
  height = 80,
  label,
}: WaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);
  const sizeRef = useRef<{ width: number; height: number } | null>(null);
  const latestRef = useRef({ samples, rms, color });

  // Keep latest drawing inputs in a ref so resize events can redraw without reflow.
  useEffect(() => {
    latestRef.current = { samples, rms, color };
  }, [samples, rms, color]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctxRef.current = ctx;

    const draw = () => {
      const size = sizeRef.current;
      const ctx = ctxRef.current;
      if (!size || !ctx) return;

      const { samples, rms, color } = latestRef.current;
      const width = size.width;
      const canvasHeight = size.height;

      // Clear canvas
      ctx.clearRect(0, 0, width, canvasHeight);

      // Draw waveform bars
      const barWidth = Math.max(2, width / samples.length - 1);
      const gap = 1;
      const maxBarHeight = canvasHeight * 0.8;
      const centerY = canvasHeight / 2;

      ctx.fillStyle = color;

      samples.forEach((sample, i) => {
        const x = i * (barWidth + gap);
        const barHeight = Math.max(2, sample * maxBarHeight);

        // Draw symmetric bar (centered)
        ctx.fillRect(x, centerY - barHeight / 2, barWidth, barHeight);
      });

      // Draw RMS indicator line
      if (rms > 0) {
        const rmsHeight = rms * maxBarHeight;
        ctx.strokeStyle = color;
        ctx.lineWidth = 1;
        ctx.globalAlpha = 0.5;
        ctx.setLineDash([2, 2]);

        // Top RMS line
        ctx.beginPath();
        ctx.moveTo(0, centerY - rmsHeight / 2);
        ctx.lineTo(width, centerY - rmsHeight / 2);
        ctx.stroke();

        // Bottom RMS line
        ctx.beginPath();
        ctx.moveTo(0, centerY + rmsHeight / 2);
        ctx.lineTo(width, centerY + rmsHeight / 2);
        ctx.stroke();

        ctx.globalAlpha = 1;
        ctx.setLineDash([]);
      }
    };

    const resize = (width: number, height: number) => {
      const nextWidth = Math.max(1, Math.floor(width));
      const nextHeight = Math.max(1, Math.floor(height));
      const prev = sizeRef.current;
      if (prev && prev.width === nextWidth && prev.height === nextHeight) return;

      const dpr = window.devicePixelRatio || 1;
      sizeRef.current = { width: nextWidth, height: nextHeight };

      // Setting width/height resets the context transform.
      canvas.width = Math.floor(nextWidth * dpr);
      canvas.height = Math.floor(nextHeight * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      draw();
    };

    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      resize(entry.contentRect.width, entry.contentRect.height);
    });

    ro.observe(canvas);

    // Initial paint (ResizeObserver may not fire immediately)
    const rect = canvas.getBoundingClientRect();
    resize(rect.width, rect.height);

    return () => {
      ro.disconnect();
    };
  }, [height]);

  // Redraw when values change without forcing layout reads.
  useEffect(() => {
    const ctx = ctxRef.current;
    const size = sizeRef.current;
    if (!ctx || !size) return;

    const { samples, rms, color } = latestRef.current;
    const width = size.width;
    const canvasHeight = size.height;

    // Clear canvas
    ctx.clearRect(0, 0, width, canvasHeight);

    // Draw waveform bars
    const barWidth = Math.max(2, width / samples.length - 1);
    const gap = 1;
    const maxBarHeight = canvasHeight * 0.8;
    const centerY = canvasHeight / 2;

    ctx.fillStyle = color;

    samples.forEach((sample, i) => {
      const x = i * (barWidth + gap);
      const barHeight = Math.max(2, sample * maxBarHeight);

      // Draw symmetric bar (centered)
      ctx.fillRect(x, centerY - barHeight / 2, barWidth, barHeight);
    });

    // Draw RMS indicator line
    if (rms > 0) {
      const rmsHeight = rms * maxBarHeight;
      ctx.strokeStyle = color;
      ctx.lineWidth = 1;
      ctx.globalAlpha = 0.5;
      ctx.setLineDash([2, 2]);

      // Top RMS line
      ctx.beginPath();
      ctx.moveTo(0, centerY - rmsHeight / 2);
      ctx.lineTo(width, centerY - rmsHeight / 2);
      ctx.stroke();

      // Bottom RMS line
      ctx.beginPath();
      ctx.moveTo(0, centerY + rmsHeight / 2);
      ctx.lineTo(width, centerY + rmsHeight / 2);
      ctx.stroke();

      ctx.globalAlpha = 1;
      ctx.setLineDash([]);
    }
  }, [samples, rms, color]);

  return (
    <div className="flex flex-col gap-1">
      {label && (
        <span className="text-xs text-gray-500 dark:text-gray-400 font-medium">
          {label}
        </span>
      )}
      <canvas
        ref={canvasRef}
        className="w-full rounded bg-gray-50 dark:bg-gray-800/50"
        style={{ height }}
      />
    </div>
  );
}
