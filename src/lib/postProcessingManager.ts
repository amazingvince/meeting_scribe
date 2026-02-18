import * as api from './tauri';
import { modelManager } from './modelManager';
import { useSettingsStore } from '../stores';

export interface PostProcessingNotifications {
  onSummaryStarted: (meetingId: string) => void;
  onSummaryDeferred: (meetingId: string) => void;
  onSummaryFailed: (meetingId: string, error: string) => void;
  onEmbeddingDeferred: (meetingId: string) => void;
  onEmbeddingCompleted: (meetingId: string) => void;
  onEmbeddingFailed: (meetingId: string, error: string) => void;
}

interface PostProcessingJob {
  meetingId: string;
  summaryQueued: boolean;
  summaryDeferredNotified: boolean;
  embeddingDone: boolean;
  embeddingDeferredNotified: boolean;
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export class PostProcessingManager {
  private readonly jobs = new Map<string, PostProcessingJob>();
  private processingPromise: Promise<void> | null = null;
  private rerunRequested = false;

  enqueueMeeting(meetingId: string): void {
    if (!this.jobs.has(meetingId)) {
      this.jobs.set(meetingId, {
        meetingId,
        summaryQueued: false,
        summaryDeferredNotified: false,
        embeddingDone: false,
        embeddingDeferredNotified: false,
      });
    }

    if (this.processingPromise) {
      this.rerunRequested = true;
    }
  }

  processPending(notifications: PostProcessingNotifications): Promise<void> {
    if (this.processingPromise) {
      this.rerunRequested = true;
      return this.processingPromise;
    }

    this.processingPromise = (async () => {
      do {
        this.rerunRequested = false;
        const jobs = [...this.jobs.values()];
        for (const job of jobs) {
          await this.processJob(job, notifications);
          const current = this.jobs.get(job.meetingId);
          if (!current) {
            continue;
          }

          const autoEmbed = useSettingsStore.getState().autoEmbedTranscripts;
          if (current.summaryQueued && (!autoEmbed || current.embeddingDone)) {
            this.jobs.delete(job.meetingId);
          }
        }
      } while (this.rerunRequested);
    })().finally(() => {
      this.processingPromise = null;
    });

    return this.processingPromise;
  }

  private async processJob(
    job: PostProcessingJob,
    notifications: PostProcessingNotifications
  ): Promise<void> {
    if (!job.summaryQueued) {
      const llmReady = await modelManager.ensureLlmReady();
      if (!llmReady) {
        if (!job.summaryDeferredNotified) {
          notifications.onSummaryDeferred(job.meetingId);
          job.summaryDeferredNotified = true;
        }
        return;
      }

      job.summaryDeferredNotified = false;
      try {
        await api.startSummaryGeneration(job.meetingId, 'full');
        job.summaryQueued = true;
        notifications.onSummaryStarted(job.meetingId);
      } catch (error) {
        notifications.onSummaryFailed(job.meetingId, getErrorMessage(error));
        this.jobs.delete(job.meetingId);
        return;
      }
    }

    const autoEmbed = useSettingsStore.getState().autoEmbedTranscripts;
    if (!autoEmbed || job.embeddingDone) {
      return;
    }

    const embeddingReady = await modelManager.ensureEmbeddingReady();
    if (!embeddingReady) {
      if (!job.embeddingDeferredNotified) {
        notifications.onEmbeddingDeferred(job.meetingId);
        job.embeddingDeferredNotified = true;
      }
      return;
    }

    job.embeddingDeferredNotified = false;
    try {
      await api.embedMeetingTranscript(job.meetingId);
      await useSettingsStore.getState().refreshUnembeddedCount();
      job.embeddingDone = true;
      notifications.onEmbeddingCompleted(job.meetingId);
    } catch (error) {
      notifications.onEmbeddingFailed(job.meetingId, getErrorMessage(error));
      this.jobs.delete(job.meetingId);
    }
  }
}

export const postProcessingManager = new PostProcessingManager();
