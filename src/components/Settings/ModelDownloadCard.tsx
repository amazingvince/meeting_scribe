/**
 * Model download card with progress bar
 */

import { Download, Check, Loader2, Trash2, AlertCircle, RefreshCw, Play } from 'lucide-react';
import { Button } from '../ui/Button';
import { ProgressBar } from '../ui/Progress';
import { Badge } from '../ui/Badge';
import { formatBytes } from '../../utils/format';

interface DownloadDetails {
  stage?: string | null;
  message?: string | null;
  downloadedBytes?: number | null;
  totalBytes?: number | null;
  speedBps?: number | null;
  file?: string | null;
}

interface ModelDownloadCardProps {
  name: string;
  description: string;
  size: string;
  downloaded: boolean;
  isDownloading: boolean;
  downloadProgress: number;
  onDownload: () => void;
  onDelete?: () => void;
  error?: string | null;
  onClearError?: () => void;
  // Optional load functionality for LLM models
  isLoaded?: boolean;
  isLoadingModel?: boolean;
  onLoad?: () => void;
  // Optional default selection
  isDefault?: boolean;
  onSetDefault?: () => void;
  downloadDetails?: DownloadDetails;
}

export function ModelDownloadCard({
  name,
  description,
  size,
  downloaded,
  isDownloading,
  downloadProgress,
  onDownload,
  onDelete,
  error,
  onClearError,
  isLoaded,
  isLoadingModel,
  onLoad,
  isDefault,
  onSetDefault,
  downloadDetails,
}: ModelDownloadCardProps) {
  const canLoad = downloaded && Boolean(onLoad) && !isLoaded;

  const handleRetry = () => {
    onClearError?.();
    if (canLoad && onLoad) {
      onLoad();
      return;
    }
    onDownload();
  };

  const transferSummary =
    downloadDetails?.downloadedBytes !== undefined &&
    downloadDetails?.downloadedBytes !== null
      ? downloadDetails.totalBytes
        ? `${formatBytes(downloadDetails.downloadedBytes)} / ${formatBytes(downloadDetails.totalBytes)}`
        : `${formatBytes(downloadDetails.downloadedBytes)} downloaded`
      : null;

  const speedSummary =
    downloadDetails?.speedBps && downloadDetails.speedBps > 0
      ? `${formatBytes(downloadDetails.speedBps)}/s`
      : null;

  const etaSummary =
    downloadDetails?.totalBytes &&
    downloadDetails?.downloadedBytes !== null &&
    downloadDetails?.downloadedBytes !== undefined &&
    downloadDetails?.speedBps &&
    downloadDetails.speedBps > 0
      ? (() => {
          const remaining = Math.max(
            0,
            downloadDetails.totalBytes - downloadDetails.downloadedBytes
          );
          const etaSeconds = Math.round(remaining / downloadDetails.speedBps);
          if (!Number.isFinite(etaSeconds) || etaSeconds <= 0) {
            return null;
          }
          if (etaSeconds < 60) {
            return `${etaSeconds}s left`;
          }
          const minutes = Math.floor(etaSeconds / 60);
          const seconds = etaSeconds % 60;
          return `${minutes}m ${seconds}s left`;
        })()
      : null;

  const normalizedStage = downloadDetails?.stage
    ? downloadDetails.stage
        .replace(/([a-z])([A-Z])/g, '$1 $2')
        .replace(/_/g, ' ')
        .trim()
    : 'Downloading';

  return (
    <div
      className={`p-4 border rounded-lg ${
        error
          ? 'border-destructive/50 bg-destructive/5'
          : 'border-border'
      }`}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="font-medium text-foreground">
              {name}
            </h3>
            {downloaded && !isLoaded && (
              <Badge variant="default" size="sm">
                <Check className="w-3 h-3 mr-1" />
                Downloaded
              </Badge>
            )}
            {isLoaded && (
              <Badge variant="success" size="sm">
                <Check className="w-3 h-3 mr-1" />
                Loaded
              </Badge>
            )}
            {error && (
              <Badge variant="error" size="sm">
                <AlertCircle className="w-3 h-3 mr-1" />
                Failed
              </Badge>
            )}
          </div>
          <p className="mt-1 text-sm text-muted-foreground">
            {description}
          </p>
          <p className="mt-1 text-xs text-muted-foreground/70">
            Size: {size}
          </p>
          {downloaded && onSetDefault && (
            <label className="flex items-center gap-2 mt-2 cursor-pointer">
              <input
                type="radio"
                checked={isDefault}
                onChange={onSetDefault}
                className="h-4 w-4 border-border text-primary focus:ring-ring"
              />
              <span className="text-sm text-muted-foreground">
                Default
              </span>
            </label>
          )}
        </div>

        <div className="flex items-center gap-2">
          {downloaded && onDelete && !isLoaded && (
            <Button variant="ghost" size="icon" onClick={onDelete}>
              <Trash2 className="w-4 h-4 text-destructive" />
            </Button>
          )}
          {canLoad && onLoad && (
            <Button
              size="sm"
              onClick={onLoad}
              disabled={isLoadingModel}
            >
              {isLoadingModel ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Play className="w-4 h-4" />
              )}
              {isLoadingModel ? 'Loading...' : error ? 'Retry Load' : 'Load'}
            </Button>
          )}
          {!downloaded && !error && (
            <Button
              variant="secondary"
              size="sm"
              onClick={onDownload}
              disabled={isDownloading}
            >
              {isDownloading ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Download className="w-4 h-4" />
              )}
              {isDownloading ? 'Downloading...' : 'Download'}
            </Button>
          )}
          {error && !canLoad && (
            <Button variant="secondary" size="sm" onClick={handleRetry}>
              <RefreshCw className="w-4 h-4" />
              {canLoad ? 'Retry Load' : 'Retry Download'}
            </Button>
          )}
        </div>
      </div>

      {isDownloading && (
        <div className="mt-3 space-y-2">
          <ProgressBar
            value={downloadProgress}
            label={normalizedStage}
            showLabel
            size="sm"
          />
          <p className="text-xs text-muted-foreground">
            {downloadDetails?.message ?? 'Downloading model files...'}
          </p>
          {(downloadDetails?.file || transferSummary || speedSummary || etaSummary) && (
            <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
              {downloadDetails?.file && <span>File: {downloadDetails.file}</span>}
              {transferSummary && <span>{transferSummary}</span>}
              {speedSummary && <span>{speedSummary}</span>}
              {etaSummary && <span>{etaSummary}</span>}
            </div>
          )}
        </div>
      )}

      {error && (
        <div className="mt-3 text-sm text-destructive flex items-start gap-2">
          <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
          <span>{error}</span>
        </div>
      )}
    </div>
  );
}
