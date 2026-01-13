-- Meeting Scribe Database Schema
-- SQLite with FTS5 for full-text search

PRAGMA foreign_keys = ON;

-- ============================================
-- CORE TABLES
-- ============================================

-- Core meeting data
CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    duration_ms INTEGER,
    audio_path_you TEXT,
    audio_path_others TEXT,
    status TEXT NOT NULL DEFAULT 'recording'
        CHECK(status IN ('recording', 'processing', 'ready', 'archived', 'error')),
    error_message TEXT,
    tags TEXT  -- JSON array: ["tag1", "tag2"]
);

-- Transcript segments with speaker labels
CREATE TABLE IF NOT EXISTS transcript_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    text TEXT NOT NULL,
    speaker TEXT CHECK(speaker IN ('you', 'others', 'unknown')),
    confidence REAL,
    embedding_id TEXT  -- Reference to LanceDB
);

-- User notes attached to meetings
CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    embedding_id TEXT
);

-- Generated summaries (for Phase 7 LLM integration)
CREATE TABLE IF NOT EXISTS summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    summary_type TEXT NOT NULL CHECK(summary_type IN ('key_points', 'action_items', 'full')),
    content TEXT NOT NULL,
    model_used TEXT,
    created_at INTEGER NOT NULL,
    embedding_id TEXT
);

-- Application settings (key-value store)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ============================================
-- INDEXES
-- ============================================

CREATE INDEX IF NOT EXISTS idx_meetings_created ON meetings(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_meetings_status ON meetings(status);
CREATE INDEX IF NOT EXISTS idx_segments_meeting ON transcript_segments(meeting_id);
CREATE INDEX IF NOT EXISTS idx_segments_time ON transcript_segments(meeting_id, start_ms);
CREATE INDEX IF NOT EXISTS idx_notes_meeting ON notes(meeting_id);
CREATE INDEX IF NOT EXISTS idx_summaries_meeting ON summaries(meeting_id);

-- ============================================
-- FULL-TEXT SEARCH (FTS5)
-- ============================================

-- FTS5 virtual table for transcript search
CREATE VIRTUAL TABLE IF NOT EXISTS transcript_fts USING fts5(
    text,
    content='transcript_segments',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- FTS triggers to keep index in sync
CREATE TRIGGER IF NOT EXISTS transcript_ai AFTER INSERT ON transcript_segments BEGIN
    INSERT INTO transcript_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE TRIGGER IF NOT EXISTS transcript_ad AFTER DELETE ON transcript_segments BEGIN
    INSERT INTO transcript_fts(transcript_fts, rowid, text) VALUES('delete', old.id, old.text);
END;

CREATE TRIGGER IF NOT EXISTS transcript_au AFTER UPDATE ON transcript_segments BEGIN
    INSERT INTO transcript_fts(transcript_fts, rowid, text) VALUES('delete', old.id, old.text);
    INSERT INTO transcript_fts(rowid, text) VALUES (new.id, new.text);
END;
