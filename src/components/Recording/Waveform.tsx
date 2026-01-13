import { useEffect, useRef } from "react";

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

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();

    // Set canvas size with device pixel ratio for crisp rendering
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    const width = rect.width;
    const canvasHeight = rect.height;

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
  }, [samples, rms, color, height]);

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
