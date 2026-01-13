# 05 - Storage Layer

> **Goal:** Implement SQLite for structured data and LanceDB for vector embeddings  
> **Time Estimate:** 4-5 days  
> **Prerequisites:** [04-transcription-engine.md](./04-transcription-engine.md) completed

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Dependencies](#dependencies)
4. [SQLite Implementation](#sqlite-implementation)
5. [LanceDB Integration](#lancedb-integration)
6. [Repository Pattern](#repository-pattern)
7. [Audio File Management](#audio-file-management)
8. [Full-Text Search](#full-text-search)
9. [Tauri Integration](#tauri-integration)
10. [Frontend Data Layer](#frontend-data-layer)
11. [Migration Strategy](#migration-strategy)
12. [Performance Optimization](#performance-optimization)
13. [Testing](#testing)
14. [Troubleshooting](#troubleshooting)
15. [Acceptance Criteria](#acceptance-criteria)

---

## Overview

Meeting Scribe uses a **dual-database architecture**:

- **SQLite** - Structured data (meetings, transcripts, settings)
- **LanceDB** - Vector embeddings for semantic search (RAG)

```
                    ┌─────────────────────────────────────┐
                    │         Application Layer           │
                    └──────────────┬──────────────────────┘
                                   │
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
         ▼                         │                         ▼
┌─────────────────┐                │            ┌─────────────────┐
│     SQLite      │                │            │    LanceDB      │
│                 │                │            │                 │
│ • Meetings      │◀───────────────┼───────────▶│ • Embeddings    │
│ • Transcripts   │    Foreign     │            │ • 768-dim       │
│ • Summaries     │      Key       │            │ • Cosine sim    │
│ • Settings      │   References   │            │                 │
│ • FTS Index     │                │            │                 │
└─────────────────┘                │            └─────────────────┘
         │                         │                         │
         └─────────────────────────┴─────────────────────────┘
                                   │
                    ┌──────────────▼──────────────┐
                    │      ~/.meeting-scribe/     │
                    │                             │
                    │  ├── data/                  │
                    │  │   ├── meetings.db        │
                    │  │   └── vectors/           │
                    │  └── audio/                 │
                    └─────────────────────────────┘
```

### Why This Architecture?

| Aspect | SQLite | LanceDB |
|--------|--------|---------|
| **Purpose** | Structured queries, relationships | Semantic similarity search |
| **Query Type** | SQL, full-text search | Vector similarity (k-NN) |
| **Data Size** | Small-medium | Large embeddings |
| **Performance** | Fast reads/writes | Optimized for vectors |
| **Maturity** | Battle-tested | Modern, growing |

---

## Architecture

### Data Flow

```
Recording Complete
        │
        ▼
┌───────────────────┐
│ Create Meeting    │───▶ SQLite: meetings table
│ Record            │
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Save Transcript   │───▶ SQLite: transcript_segments
│ Segments          │───▶ LanceDB: vectors (after embedding)
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Generate Summary  │───▶ SQLite: summaries
│                   │───▶ LanceDB: summary vectors
└─────────┬─────────┘
          │
          ▼
┌───────────────────┐
│ Index for Search  │───▶ SQLite: transcript_fts (FTS5)
│                   │───▶ LanceDB: ready for RAG
└───────────────────┘
```

### File Structure

```
~/.meeting-scribe/
├── data/
│   ├── meetings.db          # SQLite database
│   └── vectors/             # LanceDB storage
│       ├── _transactions/
│       └── embeddings.lance/
├── audio/
│   ├── raw/                 # WAV files during processing
│   │   ├── {meeting_id}_you.wav
│   │   └── {meeting_id}_others.wav
│   └── archived/            # Opus files for long-term
│       └── {meeting_id}.opus
└── config.json
```

---

## Dependencies

### Update Cargo.toml

```toml
[dependencies]
# SQLite
rusqlite = { version = "0.31", features = ["bundled", "modern_sqlite"] }

# LanceDB
lancedb = "0.4"
arrow-array = "51"
arrow-schema = "51"

# Async
tokio = { version = "1.37", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Utilities
uuid = { version = "1.8", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
parking_lot = "0.12"
```

### Crate Documentation

| Crate | Purpose | Docs |
|-------|---------|------|
| **rusqlite** | SQLite bindings | [docs.rs/rusqlite](https://docs.rs/rusqlite/latest/rusqlite/) |
| **lancedb** | Vector database | [lancedb.github.io/lancedb](https://lancedb.github.io/lancedb/) |
| **arrow-array** | Arrow data format | [docs.rs/arrow-array](https://docs.rs/arrow-array/latest/arrow_array/) |

---

## SQLite Implementation

### Database Schema

Create `src-tauri/src/storage/schema.sql`:

```sql
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Core meeting data
CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    duration_ms INTEGER,
    
    -- Audio file references
    audio_path_you TEXT,
    audio_path_others TEXT,
    audio_format TEXT DEFAULT 'wav' CHECK(audio_format IN ('wav', 'opus')),
    
    -- Processing status
    status TEXT NOT NULL DEFAULT 'recording' 
        CHECK(status IN ('recording', 'processing', 'ready', 'archived', 'error')),
    error_message TEXT,
    
    -- Metadata
    tags TEXT,  -- JSON array: ["tag1", "tag2"]
    notes_count INTEGER DEFAULT 0
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
    embedding_id TEXT  -- Reference to LanceDB
);

-- Generated summaries
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

-- Model download status
CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    model_type TEXT NOT NULL CHECK(model_type IN ('transcription', 'embedding', 'llm', 'vad')),
    path TEXT,
    size_bytes INTEGER,
    status TEXT DEFAULT 'not_downloaded' 
        CHECK(status IN ('not_downloaded', 'downloading', 'ready', 'error')),
    download_progress REAL DEFAULT 0.0,
    error_message TEXT
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_meetings_created ON meetings(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_meetings_status ON meetings(status);
CREATE INDEX IF NOT EXISTS idx_segments_meeting ON transcript_segments(meeting_id);
CREATE INDEX IF NOT EXISTS idx_segments_time ON transcript_segments(meeting_id, start_ms);
CREATE INDEX IF NOT EXISTS idx_notes_meeting ON notes(meeting_id);
CREATE INDEX IF NOT EXISTS idx_summaries_meeting ON summaries(meeting_id);

-- Full-text search for transcripts
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
```

### Database Connection Manager

Create `src-tauri/src/storage/sqlite.rs`:

```rust
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
use parking_lot::Mutex;
use anyhow::{Result, Context};
use tracing::{info, debug};

/// Thread-safe SQLite connection wrapper
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create database at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create database directory")?;
        }
        
        let conn = Connection::open(path)
            .context("Failed to open SQLite database")?;
        
        // Configure connection
        conn.execute_batch(r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            PRAGMA cache_size = -64000;  -- 64MB cache
        "#)?;
        
        info!("Opened database at {:?}", path);
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
    
    /// Initialize database schema
    pub fn initialize(&self) -> Result<()> {
        let conn = self.conn.lock();
        
        // Include schema from file at compile time
        const SCHEMA: &str = include_str!("schema.sql");
        
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize database schema")?;
        
        info!("Database schema initialized");
        Ok(())
    }
    
    /// Execute a query with the connection
    pub fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock();
        f(&conn)
    }
    
    /// Execute a mutable operation with the connection
    pub fn with_conn_mut<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T>,
    {
        let mut conn = self.conn.lock();
        f(&mut conn)
    }
    
    /// Get database statistics
    pub fn stats(&self) -> Result<DatabaseStats> {
        self.with_conn(|conn| {
            let meeting_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM meetings",
                [],
                |row| row.get(0),
            )?;
            
            let segment_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM transcript_segments",
                [],
                |row| row.get(0),
            )?;
            
            let total_duration_ms: i64 = conn.query_row(
                "SELECT COALESCE(SUM(duration_ms), 0) FROM meetings",
                [],
                |row| row.get(0),
            )?;
            
            Ok(DatabaseStats {
                meeting_count: meeting_count as u64,
                segment_count: segment_count as u64,
                total_duration_ms: total_duration_ms as u64,
            })
        })
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DatabaseStats {
    pub meeting_count: u64,
    pub segment_count: u64,
    pub total_duration_ms: u64,
}
```

### Data Models

Create `src-tauri/src/storage/models.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Meeting status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeetingStatus {
    Recording,
    Processing,
    Ready,
    Archived,
    Error,
}

impl MeetingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Archived => "archived",
            Self::Error => "error",
        }
    }
}

impl std::str::FromStr for MeetingStatus {
    type Err = anyhow::Error;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recording" => Ok(Self::Recording),
            "processing" => Ok(Self::Processing),
            "ready" => Ok(Self::Ready),
            "archived" => Ok(Self::Archived),
            "error" => Ok(Self::Error),
            _ => anyhow::bail!("Invalid meeting status: {}", s),
        }
    }
}

/// Speaker identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    You,
    Others,
    Unknown,
}

impl Speaker {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Others => "others",
            Self::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for Speaker {
    type Err = anyhow::Error;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "you" => Ok(Self::You),
            "others" => Ok(Self::Others),
            "unknown" => Ok(Self::Unknown),
            _ => anyhow::bail!("Invalid speaker: {}", s),
        }
    }
}

/// Summary type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryType {
    KeyPoints,
    ActionItems,
    Full,
}

impl SummaryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::KeyPoints => "key_points",
            Self::ActionItems => "action_items",
            Self::Full => "full",
        }
    }
}

/// Meeting entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub created_at: i64,  // Unix timestamp ms
    pub updated_at: i64,
    pub duration_ms: Option<i64>,
    
    pub audio_path_you: Option<String>,
    pub audio_path_others: Option<String>,
    pub audio_format: String,
    
    pub status: MeetingStatus,
    pub error_message: Option<String>,
    
    pub tags: Vec<String>,
    pub notes_count: i32,
}

impl Meeting {
    /// Create a new meeting with generated ID
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now().timestamp_millis();
        
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            created_at: now,
            updated_at: now,
            duration_ms: None,
            audio_path_you: None,
            audio_path_others: None,
            audio_format: "wav".to_string(),
            status: MeetingStatus::Recording,
            error_message: None,
            tags: Vec::new(),
            notes_count: 0,
        }
    }
    
    /// Generate default title from timestamp
    pub fn default_title() -> String {
        let now = chrono::Local::now();
        now.format("Meeting %Y-%m-%d %H:%M").to_string()
    }
}

/// Transcript segment entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: Option<i64>,  // Auto-generated
    pub meeting_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub speaker: Speaker,
    pub confidence: Option<f64>,
    pub embedding_id: Option<String>,
}

impl TranscriptSegment {
    pub fn new(
        meeting_id: impl Into<String>,
        start_ms: i64,
        end_ms: i64,
        text: impl Into<String>,
        speaker: Speaker,
    ) -> Self {
        Self {
            id: None,
            meeting_id: meeting_id.into(),
            start_ms,
            end_ms,
            text: text.into(),
            speaker,
            confidence: None,
            embedding_id: None,
        }
    }
    
    /// Duration in milliseconds
    pub fn duration_ms(&self) -> i64 {
        self.end_ms - self.start_ms
    }
}

/// Note entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: Option<i64>,
    pub meeting_id: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub embedding_id: Option<String>,
}

impl Note {
    pub fn new(meeting_id: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Utc::now().timestamp_millis();
        
        Self {
            id: None,
            meeting_id: meeting_id.into(),
            content: content.into(),
            created_at: now,
            updated_at: now,
            embedding_id: None,
        }
    }
}

/// Summary entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub id: Option<i64>,
    pub meeting_id: String,
    pub summary_type: SummaryType,
    pub content: String,
    pub model_used: Option<String>,
    pub created_at: i64,
    pub embedding_id: Option<String>,
}

impl Summary {
    pub fn new(
        meeting_id: impl Into<String>,
        summary_type: SummaryType,
        content: impl Into<String>,
        model_used: Option<String>,
    ) -> Self {
        Self {
            id: None,
            meeting_id: meeting_id.into(),
            summary_type,
            content: content.into(),
            model_used,
            created_at: Utc::now().timestamp_millis(),
            embedding_id: None,
        }
    }
}

/// Model download status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub path: Option<String>,
    pub size_bytes: Option<i64>,
    pub status: String,
    pub download_progress: f64,
    pub error_message: Option<String>,
}
```

---

## LanceDB Integration

### Vector Store Setup

Create `src-tauri/src/storage/vectors.rs`:

```rust
use anyhow::{Result, Context};
use lancedb::connect;
use lancedb::query::ExecutableQuery;
use arrow_array::{
    ArrayRef, Float32Array, RecordBatch, RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, debug};

/// Embedding dimension (EmbeddingGemma outputs 768-dim vectors)
pub const EMBEDDING_DIM: usize = 768;

/// Vector store for semantic search
pub struct VectorStore {
    db: lancedb::Database,
    table_name: String,
}

impl VectorStore {
    /// Connect to or create a LanceDB database
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        
        // Create directory if needed
        std::fs::create_dir_all(path)
            .context("Failed to create vector store directory")?;
        
        let uri = path.to_string_lossy().to_string();
        let db = connect(&uri).execute().await
            .context("Failed to connect to LanceDB")?;
        
        info!("Connected to vector store at {:?}", path);
        
        Ok(Self {
            db,
            table_name: "embeddings".to_string(),
        })
    }
    
    /// Initialize the embeddings table
    pub async fn initialize(&self) -> Result<()> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("meeting_id", DataType::Utf8, false),
            Field::new("chunk_type", DataType::Utf8, false),  // transcript, note, summary
            Field::new("text", DataType::Utf8, false),
            Field::new("start_ms", DataType::Int64, true),    // For transcript chunks
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM as i32,
                ),
                false,
            ),
        ]));
        
        // Check if table exists
        let tables = self.db.table_names().execute().await?;
        
        if !tables.contains(&self.table_name) {
            // Create empty table with schema
            let batch = self.create_empty_batch(&schema)?;
            let batches = RecordBatchIterator::new(vec![Ok(batch)], schema.clone());
            
            self.db
                .create_table(&self.table_name, Box::new(batches))
                .execute()
                .await
                .context("Failed to create embeddings table")?;
            
            info!("Created embeddings table");
        } else {
            debug!("Embeddings table already exists");
        }
        
        Ok(())
    }
    
    /// Create an empty batch with the schema (for initialization)
    fn create_empty_batch(&self, schema: &Arc<Schema>) -> Result<RecordBatch> {
        let id: ArrayRef = Arc::new(StringArray::from(Vec::<String>::new()));
        let meeting_id: ArrayRef = Arc::new(StringArray::from(Vec::<String>::new()));
        let chunk_type: ArrayRef = Arc::new(StringArray::from(Vec::<String>::new()));
        let text: ArrayRef = Arc::new(StringArray::from(Vec::<String>::new()));
        let start_ms: ArrayRef = Arc::new(arrow_array::Int64Array::from(Vec::<i64>::new()));
        
        // Empty fixed-size list for vectors
        let values = Float32Array::from(Vec::<f32>::new());
        let vector: ArrayRef = Arc::new(
            arrow_array::FixedSizeListArray::try_new_from_values(
                values,
                EMBEDDING_DIM as i32,
            )?,
        );
        
        RecordBatch::try_new(
            schema.clone(),
            vec![id, meeting_id, chunk_type, text, start_ms, vector],
        ).context("Failed to create empty batch")
    }
    
    /// Add embeddings to the store
    pub async fn add_embeddings(&self, records: Vec<EmbeddingRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        
        let table = self.db.open_table(&self.table_name).execute().await
            .context("Failed to open embeddings table")?;
        
        let batch = self.records_to_batch(&records)?;
        let schema = batch.schema();
        let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);
        
        table.add(Box::new(batches)).execute().await
            .context("Failed to add embeddings")?;
        
        debug!("Added {} embeddings to vector store", records.len());
        Ok(())
    }
    
    /// Convert records to Arrow RecordBatch
    fn records_to_batch(&self, records: &[EmbeddingRecord]) -> Result<RecordBatch> {
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        let meeting_ids: Vec<&str> = records.iter().map(|r| r.meeting_id.as_str()).collect();
        let chunk_types: Vec<&str> = records.iter().map(|r| r.chunk_type.as_str()).collect();
        let texts: Vec<&str> = records.iter().map(|r| r.text.as_str()).collect();
        let start_ms_vals: Vec<Option<i64>> = records.iter().map(|r| r.start_ms).collect();
        
        // Flatten all vectors into a single array
        let flat_vectors: Vec<f32> = records
            .iter()
            .flat_map(|r| r.vector.iter().copied())
            .collect();
        
        let id_array: ArrayRef = Arc::new(StringArray::from(ids));
        let meeting_id_array: ArrayRef = Arc::new(StringArray::from(meeting_ids));
        let chunk_type_array: ArrayRef = Arc::new(StringArray::from(chunk_types));
        let text_array: ArrayRef = Arc::new(StringArray::from(texts));
        let start_ms_array: ArrayRef = Arc::new(arrow_array::Int64Array::from(start_ms_vals));
        
        let values = Float32Array::from(flat_vectors);
        let vector_array: ArrayRef = Arc::new(
            arrow_array::FixedSizeListArray::try_new_from_values(
                values,
                EMBEDDING_DIM as i32,
            )?,
        );
        
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("meeting_id", DataType::Utf8, false),
            Field::new("chunk_type", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("start_ms", DataType::Int64, true),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM as i32,
                ),
                false,
            ),
        ]));
        
        RecordBatch::try_new(
            schema,
            vec![
                id_array,
                meeting_id_array,
                chunk_type_array,
                text_array,
                start_ms_array,
                vector_array,
            ],
        ).context("Failed to create record batch")
    }
    
    /// Search for similar embeddings
    pub async fn search(
        &self,
        query_vector: &[f32],
        limit: usize,
        filter: Option<&str>,
    ) -> Result<Vec<SearchResult>> {
        if query_vector.len() != EMBEDDING_DIM {
            anyhow::bail!(
                "Query vector has wrong dimension: {} (expected {})",
                query_vector.len(),
                EMBEDDING_DIM
            );
        }
        
        let table = self.db.open_table(&self.table_name).execute().await
            .context("Failed to open embeddings table")?;
        
        let mut query = table
            .vector_search(query_vector.to_vec())
            .context("Failed to create vector search")?
            .limit(limit);
        
        // Apply filter if provided (e.g., "meeting_id = 'abc123'")
        if let Some(filter_expr) = filter {
            query = query.filter(filter_expr);
        }
        
        let results = query.execute().await
            .context("Failed to execute vector search")?;
        
        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .context("Failed to collect search results")?;
        
        let mut search_results = Vec::new();
        
        for batch in batches {
            let ids = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .context("Missing id column")?;
            
            let meeting_ids = batch
                .column_by_name("meeting_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .context("Missing meeting_id column")?;
            
            let chunk_types = batch
                .column_by_name("chunk_type")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .context("Missing chunk_type column")?;
            
            let texts = batch
                .column_by_name("text")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .context("Missing text column")?;
            
            let start_ms_arr = batch
                .column_by_name("start_ms")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Int64Array>());
            
            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());
            
            for i in 0..batch.num_rows() {
                let distance = distances.map(|d| d.value(i)).unwrap_or(0.0);
                let similarity = 1.0 - distance;  // Convert distance to similarity
                
                search_results.push(SearchResult {
                    id: ids.value(i).to_string(),
                    meeting_id: meeting_ids.value(i).to_string(),
                    chunk_type: chunk_types.value(i).to_string(),
                    text: texts.value(i).to_string(),
                    start_ms: start_ms_arr.and_then(|arr| {
                        if arr.is_null(i) { None } else { Some(arr.value(i)) }
                    }),
                    similarity,
                });
            }
        }
        
        Ok(search_results)
    }
    
    /// Delete embeddings for a meeting
    pub async fn delete_meeting_embeddings(&self, meeting_id: &str) -> Result<u64> {
        let table = self.db.open_table(&self.table_name).execute().await
            .context("Failed to open embeddings table")?;
        
        let deleted = table
            .delete(&format!("meeting_id = '{}'", meeting_id))
            .await
            .context("Failed to delete embeddings")?;
        
        debug!("Deleted embeddings for meeting {}", meeting_id);
        Ok(deleted as u64)
    }
    
    /// Get embedding count
    pub async fn count(&self) -> Result<u64> {
        let table = self.db.open_table(&self.table_name).execute().await
            .context("Failed to open embeddings table")?;
        
        let count = table.count_rows(None).await
            .context("Failed to count embeddings")?;
        
        Ok(count as u64)
    }
}

/// Embedding record for storage
#[derive(Debug, Clone)]
pub struct EmbeddingRecord {
    pub id: String,
    pub meeting_id: String,
    pub chunk_type: String,  // "transcript", "note", "summary"
    pub text: String,
    pub start_ms: Option<i64>,
    pub vector: Vec<f32>,
}

impl EmbeddingRecord {
    pub fn new_transcript(
        meeting_id: &str,
        text: &str,
        start_ms: i64,
        vector: Vec<f32>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: meeting_id.to_string(),
            chunk_type: "transcript".to_string(),
            text: text.to_string(),
            start_ms: Some(start_ms),
            vector,
        }
    }
    
    pub fn new_note(meeting_id: &str, text: &str, vector: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: meeting_id.to_string(),
            chunk_type: "note".to_string(),
            text: text.to_string(),
            start_ms: None,
            vector,
        }
    }
    
    pub fn new_summary(meeting_id: &str, text: &str, vector: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: meeting_id.to_string(),
            chunk_type: "summary".to_string(),
            text: text.to_string(),
            start_ms: None,
            vector,
        }
    }
}

/// Search result from vector store
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub id: String,
    pub meeting_id: String,
    pub chunk_type: String,
    pub text: String,
    pub start_ms: Option<i64>,
    pub similarity: f32,
}

// Implement async iterator collection for search results
use futures::TryStreamExt;
```

---

## Repository Pattern

### Meeting Repository

Create `src-tauri/src/storage/repositories/meetings.rs`:

```rust
use crate::storage::{Database, models::*};
use rusqlite::{params, Row};
use anyhow::{Result, Context};
use tracing::debug;

/// Repository for meeting operations
pub struct MeetingRepository {
    db: Database,
}

impl MeetingRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
    
    /// Create a new meeting
    pub fn create(&self, meeting: &Meeting) -> Result<()> {
        self.db.with_conn(|conn| {
            let tags_json = serde_json::to_string(&meeting.tags)?;
            
            conn.execute(
                r#"
                INSERT INTO meetings (
                    id, title, created_at, updated_at, duration_ms,
                    audio_path_you, audio_path_others, audio_format,
                    status, error_message, tags, notes_count
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    meeting.id,
                    meeting.title,
                    meeting.created_at,
                    meeting.updated_at,
                    meeting.duration_ms,
                    meeting.audio_path_you,
                    meeting.audio_path_others,
                    meeting.audio_format,
                    meeting.status.as_str(),
                    meeting.error_message,
                    tags_json,
                    meeting.notes_count,
                ],
            )?;
            
            debug!("Created meeting: {}", meeting.id);
            Ok(())
        })
    }
    
    /// Get meeting by ID
    pub fn get(&self, id: &str) -> Result<Option<Meeting>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT * FROM meetings WHERE id = ?"
            )?;
            
            let result = stmt.query_row([id], Self::row_to_meeting);
            
            match result {
                Ok(meeting) => Ok(Some(meeting)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }
    
    /// List meetings with pagination and filtering
    pub fn list(&self, options: ListOptions) -> Result<Vec<Meeting>> {
        self.db.with_conn(|conn| {
            let mut query = String::from("SELECT * FROM meetings WHERE 1=1");
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            
            // Apply status filter
            if let Some(status) = &options.status {
                query.push_str(" AND status = ?");
                params_vec.push(Box::new(status.as_str().to_string()));
            }
            
            // Apply search filter
            if let Some(search) = &options.search {
                query.push_str(" AND title LIKE ?");
                params_vec.push(Box::new(format!("%{}%", search)));
            }
            
            // Apply date range
            if let Some(after) = options.after {
                query.push_str(" AND created_at >= ?");
                params_vec.push(Box::new(after));
            }
            
            if let Some(before) = options.before {
                query.push_str(" AND created_at <= ?");
                params_vec.push(Box::new(before));
            }
            
            // Order and pagination
            query.push_str(" ORDER BY created_at DESC");
            query.push_str(&format!(" LIMIT {} OFFSET {}", options.limit, options.offset));
            
            let mut stmt = conn.prepare(&query)?;
            
            let params_refs: Vec<&dyn rusqlite::ToSql> = 
                params_vec.iter().map(|p| p.as_ref()).collect();
            
            let meetings = stmt
                .query_map(params_refs.as_slice(), Self::row_to_meeting)?
                .collect::<Result<Vec<_>, _>>()?;
            
            Ok(meetings)
        })
    }
    
    /// Update meeting
    pub fn update(&self, meeting: &Meeting) -> Result<()> {
        self.db.with_conn(|conn| {
            let tags_json = serde_json::to_string(&meeting.tags)?;
            
            conn.execute(
                r#"
                UPDATE meetings SET
                    title = ?,
                    updated_at = ?,
                    duration_ms = ?,
                    audio_path_you = ?,
                    audio_path_others = ?,
                    audio_format = ?,
                    status = ?,
                    error_message = ?,
                    tags = ?,
                    notes_count = ?
                WHERE id = ?
                "#,
                params![
                    meeting.title,
                    chrono::Utc::now().timestamp_millis(),
                    meeting.duration_ms,
                    meeting.audio_path_you,
                    meeting.audio_path_others,
                    meeting.audio_format,
                    meeting.status.as_str(),
                    meeting.error_message,
                    tags_json,
                    meeting.notes_count,
                    meeting.id,
                ],
            )?;
            
            debug!("Updated meeting: {}", meeting.id);
            Ok(())
        })
    }
    
    /// Update meeting status
    pub fn update_status(
        &self,
        id: &str,
        status: MeetingStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE meetings SET
                    status = ?,
                    error_message = ?,
                    updated_at = ?
                WHERE id = ?
                "#,
                params![
                    status.as_str(),
                    error_message,
                    chrono::Utc::now().timestamp_millis(),
                    id,
                ],
            )?;
            
            debug!("Updated meeting {} status to {:?}", id, status);
            Ok(())
        })
    }
    
    /// Delete meeting and cascade to related data
    pub fn delete(&self, id: &str) -> Result<bool> {
        self.db.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM meetings WHERE id = ?", [id])?;
            
            if rows > 0 {
                debug!("Deleted meeting: {}", id);
            }
            
            Ok(rows > 0)
        })
    }
    
    /// Convert database row to Meeting
    fn row_to_meeting(row: &Row) -> rusqlite::Result<Meeting> {
        let tags_json: String = row.get("tags")?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        
        let status_str: String = row.get("status")?;
        let status: MeetingStatus = status_str.parse().unwrap_or(MeetingStatus::Error);
        
        Ok(Meeting {
            id: row.get("id")?,
            title: row.get("title")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            duration_ms: row.get("duration_ms")?,
            audio_path_you: row.get("audio_path_you")?,
            audio_path_others: row.get("audio_path_others")?,
            audio_format: row.get("audio_format")?,
            status,
            error_message: row.get("error_message")?,
            tags,
            notes_count: row.get("notes_count")?,
        })
    }
}

/// Options for listing meetings
#[derive(Debug, Default)]
pub struct ListOptions {
    pub status: Option<MeetingStatus>,
    pub search: Option<String>,
    pub after: Option<i64>,
    pub before: Option<i64>,
    pub limit: u32,
    pub offset: u32,
}

impl ListOptions {
    pub fn new() -> Self {
        Self {
            limit: 50,
            offset: 0,
            ..Default::default()
        }
    }
    
    pub fn with_status(mut self, status: MeetingStatus) -> Self {
        self.status = Some(status);
        self
    }
    
    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }
    
    pub fn with_pagination(mut self, limit: u32, offset: u32) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}
```

### Transcript Repository

Create `src-tauri/src/storage/repositories/transcripts.rs`:

```rust
use crate::storage::{Database, models::*};
use rusqlite::{params, Row};
use anyhow::{Result, Context};
use tracing::debug;

/// Repository for transcript segment operations
pub struct TranscriptRepository {
    db: Database,
}

impl TranscriptRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
    
    /// Insert multiple segments in a batch
    pub fn insert_batch(&self, segments: &[TranscriptSegment]) -> Result<Vec<i64>> {
        self.db.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let mut ids = Vec::with_capacity(segments.len());
            
            {
                let mut stmt = tx.prepare(
                    r#"
                    INSERT INTO transcript_segments (
                        meeting_id, start_ms, end_ms, text,
                        speaker, confidence, embedding_id
                    ) VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#
                )?;
                
                for segment in segments {
                    stmt.execute(params![
                        segment.meeting_id,
                        segment.start_ms,
                        segment.end_ms,
                        segment.text,
                        segment.speaker.as_str(),
                        segment.confidence,
                        segment.embedding_id,
                    ])?;
                    
                    ids.push(tx.last_insert_rowid());
                }
            }
            
            tx.commit()?;
            
            debug!("Inserted {} transcript segments", segments.len());
            Ok(ids)
        })
    }
    
    /// Get all segments for a meeting
    pub fn get_by_meeting(&self, meeting_id: &str) -> Result<Vec<TranscriptSegment>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT * FROM transcript_segments
                WHERE meeting_id = ?
                ORDER BY start_ms ASC
                "#
            )?;
            
            let segments = stmt
                .query_map([meeting_id], Self::row_to_segment)?
                .collect::<Result<Vec<_>, _>>()?;
            
            Ok(segments)
        })
    }
    
    /// Get segments in a time range
    pub fn get_in_range(
        &self,
        meeting_id: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<TranscriptSegment>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT * FROM transcript_segments
                WHERE meeting_id = ?
                  AND start_ms >= ?
                  AND end_ms <= ?
                ORDER BY start_ms ASC
                "#
            )?;
            
            let segments = stmt
                .query_map(params![meeting_id, start_ms, end_ms], Self::row_to_segment)?
                .collect::<Result<Vec<_>, _>>()?;
            
            Ok(segments)
        })
    }
    
    /// Update embedding ID for a segment
    pub fn update_embedding_id(&self, id: i64, embedding_id: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                "UPDATE transcript_segments SET embedding_id = ? WHERE id = ?",
                params![embedding_id, id],
            )?;
            Ok(())
        })
    }
    
    /// Get full transcript text for a meeting
    pub fn get_full_text(&self, meeting_id: &str) -> Result<String> {
        let segments = self.get_by_meeting(meeting_id)?;
        
        let text = segments
            .iter()
            .map(|s| format!("[{}] {}", s.speaker.as_str().to_uppercase(), s.text))
            .collect::<Vec<_>>()
            .join("\n\n");
        
        Ok(text)
    }
    
    /// Delete all segments for a meeting
    pub fn delete_by_meeting(&self, meeting_id: &str) -> Result<u64> {
        self.db.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM transcript_segments WHERE meeting_id = ?",
                [meeting_id],
            )?;
            
            debug!("Deleted {} segments for meeting {}", rows, meeting_id);
            Ok(rows as u64)
        })
    }
    
    fn row_to_segment(row: &Row) -> rusqlite::Result<TranscriptSegment> {
        let speaker_str: String = row.get("speaker")?;
        let speaker: Speaker = speaker_str.parse().unwrap_or(Speaker::Unknown);
        
        Ok(TranscriptSegment {
            id: Some(row.get("id")?),
            meeting_id: row.get("meeting_id")?,
            start_ms: row.get("start_ms")?,
            end_ms: row.get("end_ms")?,
            text: row.get("text")?,
            speaker,
            confidence: row.get("confidence")?,
            embedding_id: row.get("embedding_id")?,
        })
    }
}
```

### Repository Module

Create `src-tauri/src/storage/repositories/mod.rs`:

```rust
mod meetings;
mod transcripts;

pub use meetings::{MeetingRepository, ListOptions};
pub use transcripts::TranscriptRepository;

use crate::storage::{Database, VectorStore};
use std::sync::Arc;

/// Container for all repositories
pub struct Repositories {
    pub meetings: MeetingRepository,
    pub transcripts: TranscriptRepository,
    pub vectors: Arc<VectorStore>,
}

impl Repositories {
    pub fn new(db: Database, vectors: Arc<VectorStore>) -> Self {
        Self {
            meetings: MeetingRepository::new(db.clone()),
            transcripts: TranscriptRepository::new(db),
            vectors,
        }
    }
}
```

---

## Audio File Management

Create `src-tauri/src/storage/audio_files.rs`:

```rust
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, debug, warn};

/// Manages audio file storage and archival
pub struct AudioFileManager {
    base_path: PathBuf,
    raw_dir: PathBuf,
    archived_dir: PathBuf,
}

impl AudioFileManager {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        let raw_dir = base_path.join("raw");
        let archived_dir = base_path.join("archived");
        
        // Create directories
        std::fs::create_dir_all(&raw_dir)
            .context("Failed to create raw audio directory")?;
        std::fs::create_dir_all(&archived_dir)
            .context("Failed to create archived audio directory")?;
        
        Ok(Self {
            base_path,
            raw_dir,
            archived_dir,
        })
    }
    
    /// Get path for a new raw recording
    pub fn raw_path(&self, meeting_id: &str, channel: &str) -> PathBuf {
        self.raw_dir.join(format!("{}_{}.wav", meeting_id, channel))
    }
    
    /// Get path for archived audio
    pub fn archived_path(&self, meeting_id: &str) -> PathBuf {
        self.archived_dir.join(format!("{}.opus", meeting_id))
    }
    
    /// Convert WAV to Opus for archival
    pub async fn archive_meeting(&self, meeting_id: &str) -> Result<PathBuf> {
        let you_path = self.raw_path(meeting_id, "you");
        let others_path = self.raw_path(meeting_id, "others");
        let output_path = self.archived_path(meeting_id);
        
        // Check if both files exist
        let has_you = you_path.exists();
        let has_others = others_path.exists();
        
        if !has_you && !has_others {
            anyhow::bail!("No audio files found for meeting {}", meeting_id);
        }
        
        // Merge and convert using FFmpeg
        let status = if has_you && has_others {
            // Merge both channels
            Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i", you_path.to_str().unwrap(),
                    "-i", others_path.to_str().unwrap(),
                    "-filter_complex", "[0:a][1:a]amerge=inputs=2,pan=stereo|c0<c0+c1|c1<c0+c1[a]",
                    "-map", "[a]",
                    "-c:a", "libopus",
                    "-b:a", "48k",
                    "-application", "voip",
                    output_path.to_str().unwrap(),
                ])
                .status()
                .context("Failed to run ffmpeg")?
        } else {
            // Single channel
            let input_path = if has_you { &you_path } else { &others_path };
            
            Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i", input_path.to_str().unwrap(),
                    "-c:a", "libopus",
                    "-b:a", "48k",
                    "-application", "voip",
                    output_path.to_str().unwrap(),
                ])
                .status()
                .context("Failed to run ffmpeg")?
        };
        
        if !status.success() {
            anyhow::bail!("FFmpeg conversion failed with status: {}", status);
        }
        
        info!("Archived meeting {} to {:?}", meeting_id, output_path);
        Ok(output_path)
    }
    
    /// Delete raw WAV files after archival
    pub fn cleanup_raw(&self, meeting_id: &str) -> Result<()> {
        let you_path = self.raw_path(meeting_id, "you");
        let others_path = self.raw_path(meeting_id, "others");
        
        if you_path.exists() {
            std::fs::remove_file(&you_path)
                .context("Failed to delete raw you audio")?;
            debug!("Deleted {:?}", you_path);
        }
        
        if others_path.exists() {
            std::fs::remove_file(&others_path)
                .context("Failed to delete raw others audio")?;
            debug!("Deleted {:?}", others_path);
        }
        
        Ok(())
    }
    
    /// Get total storage used
    pub fn storage_stats(&self) -> Result<StorageStats> {
        let raw_size = dir_size(&self.raw_dir)?;
        let archived_size = dir_size(&self.archived_dir)?;
        
        Ok(StorageStats {
            raw_bytes: raw_size,
            archived_bytes: archived_size,
            total_bytes: raw_size + archived_size,
        })
    }
    
    /// List meetings that should be archived (older than threshold)
    pub fn find_archivable(&self, max_age_days: u32) -> Result<Vec<String>> {
        let threshold = std::time::SystemTime::now()
            - std::time::Duration::from_secs(max_age_days as u64 * 24 * 60 * 60);
        
        let mut archivable = Vec::new();
        
        for entry in std::fs::read_dir(&self.raw_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            
            if let Ok(modified) = metadata.modified() {
                if modified < threshold {
                    if let Some(name) = entry.path().file_stem() {
                        // Extract meeting ID (format: {meeting_id}_{channel}.wav)
                        let name = name.to_string_lossy();
                        if let Some(meeting_id) = name.rsplit('_').skip(1).next() {
                            archivable.push(meeting_id.to_string());
                        }
                    }
                }
            }
        }
        
        // Deduplicate
        archivable.sort();
        archivable.dedup();
        
        Ok(archivable)
    }
}

/// Calculate directory size recursively
fn dir_size(path: &Path) -> Result<u64> {
    let mut size = 0;
    
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            
            if metadata.is_file() {
                size += metadata.len();
            } else if metadata.is_dir() {
                size += dir_size(&entry.path())?;
            }
        }
    }
    
    Ok(size)
}

#[derive(Debug, serde::Serialize)]
pub struct StorageStats {
    pub raw_bytes: u64,
    pub archived_bytes: u64,
    pub total_bytes: u64,
}
```

---

## Full-Text Search

Create `src-tauri/src/storage/search.rs`:

```rust
use crate::storage::Database;
use rusqlite::params;
use anyhow::{Result, Context};
use serde::Serialize;

/// Full-text search service
pub struct SearchService {
    db: Database,
}

impl SearchService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
    
    /// Search transcripts using FTS5
    pub fn search_transcripts(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>> {
        self.db.with_conn(|conn| {
            // Use FTS5 match syntax
            let fts_query = Self::sanitize_fts_query(query);
            
            let mut stmt = conn.prepare(
                r#"
                SELECT 
                    ts.id,
                    ts.meeting_id,
                    ts.start_ms,
                    ts.end_ms,
                    ts.text,
                    ts.speaker,
                    m.title as meeting_title,
                    m.created_at as meeting_date,
                    bm25(transcript_fts) as rank
                FROM transcript_fts
                JOIN transcript_segments ts ON transcript_fts.rowid = ts.id
                JOIN meetings m ON ts.meeting_id = m.id
                WHERE transcript_fts MATCH ?
                ORDER BY rank
                LIMIT ?
                "#
            )?;
            
            let results = stmt
                .query_map(params![fts_query, limit], |row| {
                    Ok(SearchHit {
                        segment_id: row.get("id")?,
                        meeting_id: row.get("meeting_id")?,
                        meeting_title: row.get("meeting_title")?,
                        meeting_date: row.get("meeting_date")?,
                        start_ms: row.get("start_ms")?,
                        end_ms: row.get("end_ms")?,
                        text: row.get("text")?,
                        speaker: row.get("speaker")?,
                        rank: row.get("rank")?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            
            Ok(results)
        })
    }
    
    /// Search with snippet highlighting
    pub fn search_with_snippets(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SearchHitWithSnippet>> {
        self.db.with_conn(|conn| {
            let fts_query = Self::sanitize_fts_query(query);
            
            let mut stmt = conn.prepare(
                r#"
                SELECT 
                    ts.id,
                    ts.meeting_id,
                    ts.start_ms,
                    ts.end_ms,
                    ts.speaker,
                    m.title as meeting_title,
                    m.created_at as meeting_date,
                    snippet(transcript_fts, 0, '<mark>', '</mark>', '...', 32) as snippet,
                    bm25(transcript_fts) as rank
                FROM transcript_fts
                JOIN transcript_segments ts ON transcript_fts.rowid = ts.id
                JOIN meetings m ON ts.meeting_id = m.id
                WHERE transcript_fts MATCH ?
                ORDER BY rank
                LIMIT ?
                "#
            )?;
            
            let results = stmt
                .query_map(params![fts_query, limit], |row| {
                    Ok(SearchHitWithSnippet {
                        segment_id: row.get("id")?,
                        meeting_id: row.get("meeting_id")?,
                        meeting_title: row.get("meeting_title")?,
                        meeting_date: row.get("meeting_date")?,
                        start_ms: row.get("start_ms")?,
                        end_ms: row.get("end_ms")?,
                        speaker: row.get("speaker")?,
                        snippet: row.get("snippet")?,
                        rank: row.get("rank")?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            
            Ok(results)
        })
    }
    
    /// Sanitize query for FTS5 (escape special characters)
    fn sanitize_fts_query(query: &str) -> String {
        // FTS5 query syntax: quote phrases, escape special chars
        let escaped = query
            .replace('"', "\"\"")
            .replace('*', "")
            .replace('?', "");
        
        // If query contains multiple words, wrap in quotes for phrase search
        // or add * for prefix matching
        if query.contains(' ') {
            format!("\"{}\"", escaped)
        } else {
            format!("{}*", escaped)  // Prefix search for single words
        }
    }
    
    /// Rebuild FTS index (useful after bulk imports)
    pub fn rebuild_index(&self) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute_batch(
                r#"
                INSERT INTO transcript_fts(transcript_fts) VALUES('rebuild');
                "#
            )?;
            Ok(())
        })
    }
}

/// Search result
#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub segment_id: i64,
    pub meeting_id: String,
    pub meeting_title: String,
    pub meeting_date: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub speaker: String,
    pub rank: f64,
}

/// Search result with highlighted snippet
#[derive(Debug, Serialize)]
pub struct SearchHitWithSnippet {
    pub segment_id: i64,
    pub meeting_id: String,
    pub meeting_title: String,
    pub meeting_date: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker: String,
    pub snippet: String,
    pub rank: f64,
}
```

---

## Tauri Integration

### Storage State

Create `src-tauri/src/storage/mod.rs`:

```rust
mod sqlite;
mod vectors;
mod models;
mod repositories;
mod audio_files;
mod search;

pub use sqlite::{Database, DatabaseStats};
pub use vectors::{VectorStore, EmbeddingRecord, SearchResult, EMBEDDING_DIM};
pub use models::*;
pub use repositories::*;
pub use audio_files::{AudioFileManager, StorageStats};
pub use search::{SearchService, SearchHit, SearchHitWithSnippet};

use std::path::Path;
use std::sync::Arc;
use anyhow::Result;
use tracing::info;

/// Initialize all storage components
pub async fn initialize_storage(data_dir: impl AsRef<Path>) -> Result<StorageState> {
    let data_dir = data_dir.as_ref();
    
    // SQLite database
    let db_path = data_dir.join("data").join("meetings.db");
    let db = Database::open(&db_path)?;
    db.initialize()?;
    
    // Vector store
    let vectors_path = data_dir.join("data").join("vectors");
    let vectors = VectorStore::open(&vectors_path).await?;
    vectors.initialize().await?;
    
    // Audio file manager
    let audio_path = data_dir.join("audio");
    let audio = AudioFileManager::new(&audio_path)?;
    
    // Search service
    let search = SearchService::new(db.clone());
    
    info!("Storage initialized at {:?}", data_dir);
    
    Ok(StorageState {
        db,
        vectors: Arc::new(vectors),
        audio,
        search,
    })
}

/// Combined storage state for Tauri
pub struct StorageState {
    pub db: Database,
    pub vectors: Arc<VectorStore>,
    pub audio: AudioFileManager,
    pub search: SearchService,
}

impl StorageState {
    /// Get repositories for data access
    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.db.clone(), Arc::clone(&self.vectors))
    }
}
```

### Tauri Commands

Create `src-tauri/src/commands/storage.rs`:

```rust
use crate::storage::*;
use tauri::State;
use std::sync::Arc;
use tokio::sync::Mutex;

type StorageStateHandle = Arc<Mutex<StorageState>>;

// ==================== Meeting Commands ====================

#[tauri::command]
pub async fn create_meeting(
    storage: State<'_, StorageStateHandle>,
    title: Option<String>,
) -> Result<Meeting, String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    let meeting = Meeting::new(title.unwrap_or_else(Meeting::default_title));
    
    repos.meetings
        .create(&meeting)
        .map_err(|e| e.to_string())?;
    
    Ok(meeting)
}

#[tauri::command]
pub async fn get_meeting(
    storage: State<'_, StorageStateHandle>,
    id: String,
) -> Result<Option<Meeting>, String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    repos.meetings
        .get(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_meetings(
    storage: State<'_, StorageStateHandle>,
    status: Option<String>,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Meeting>, String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    let mut options = ListOptions::new()
        .with_pagination(limit.unwrap_or(50), offset.unwrap_or(0));
    
    if let Some(status_str) = status {
        if let Ok(status) = status_str.parse() {
            options = options.with_status(status);
        }
    }
    
    if let Some(search_str) = search {
        options = options.with_search(search_str);
    }
    
    repos.meetings
        .list(options)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_meeting(
    storage: State<'_, StorageStateHandle>,
    meeting: Meeting,
) -> Result<(), String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    repos.meetings
        .update(&meeting)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_meeting(
    storage: State<'_, StorageStateHandle>,
    id: String,
) -> Result<bool, String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    // Delete from vector store first
    storage.vectors
        .delete_meeting_embeddings(&id)
        .await
        .map_err(|e| e.to_string())?;
    
    // Delete from SQLite (cascades to segments, notes, summaries)
    repos.meetings
        .delete(&id)
        .map_err(|e| e.to_string())
}

// ==================== Transcript Commands ====================

#[tauri::command]
pub async fn get_transcript(
    storage: State<'_, StorageStateHandle>,
    meeting_id: String,
) -> Result<Vec<TranscriptSegment>, String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    repos.transcripts
        .get_by_meeting(&meeting_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_full_transcript_text(
    storage: State<'_, StorageStateHandle>,
    meeting_id: String,
) -> Result<String, String> {
    let storage = storage.lock().await;
    let repos = storage.repositories();
    
    repos.transcripts
        .get_full_text(&meeting_id)
        .map_err(|e| e.to_string())
}

// ==================== Search Commands ====================

#[tauri::command]
pub async fn search_transcripts(
    storage: State<'_, StorageStateHandle>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SearchHitWithSnippet>, String> {
    let storage = storage.lock().await;
    
    storage.search
        .search_with_snippets(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vector_search(
    storage: State<'_, StorageStateHandle>,
    query_vector: Vec<f32>,
    limit: Option<usize>,
    meeting_filter: Option<String>,
) -> Result<Vec<SearchResult>, String> {
    let storage = storage.lock().await;
    
    let filter = meeting_filter
        .as_ref()
        .map(|id| format!("meeting_id = '{}'", id));
    
    storage.vectors
        .search(&query_vector, limit.unwrap_or(10), filter.as_deref())
        .await
        .map_err(|e| e.to_string())
}

// ==================== Stats Commands ====================

#[tauri::command]
pub async fn get_database_stats(
    storage: State<'_, StorageStateHandle>,
) -> Result<DatabaseStats, String> {
    let storage = storage.lock().await;
    
    storage.db
        .stats()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_storage_stats(
    storage: State<'_, StorageStateHandle>,
) -> Result<StorageStats, String> {
    let storage = storage.lock().await;
    
    storage.audio
        .storage_stats()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_vector_count(
    storage: State<'_, StorageStateHandle>,
) -> Result<u64, String> {
    let storage = storage.lock().await;
    
    storage.vectors
        .count()
        .await
        .map_err(|e| e.to_string())
}
```

### Main App Integration

Update `src-tauri/src/main.rs`:

```rust
mod storage;
mod commands;

use storage::initialize_storage;
use commands::storage::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Manager;

#[tokio::main]
async fn main() {
    // Get data directory
    let data_dir = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".meeting-scribe");
    
    // Initialize storage
    let storage_state = initialize_storage(&data_dir)
        .await
        .expect("Failed to initialize storage");
    
    let storage_handle = Arc::new(Mutex::new(storage_state));
    
    tauri::Builder::default()
        .manage(storage_handle)
        .invoke_handler(tauri::generate_handler![
            // Meeting commands
            create_meeting,
            get_meeting,
            list_meetings,
            update_meeting,
            delete_meeting,
            // Transcript commands
            get_transcript,
            get_full_transcript_text,
            // Search commands
            search_transcripts,
            vector_search,
            // Stats commands
            get_database_stats,
            get_storage_stats,
            get_vector_count,
        ])
        .run(tauri::generate_context!())
        .expect("Error running tauri application");
}
```

---

## Frontend Data Layer

### TypeScript Types

Create `src/types/storage.ts`:

```typescript
// Meeting status
export type MeetingStatus = 'recording' | 'processing' | 'ready' | 'archived' | 'error';

// Speaker identification
export type Speaker = 'you' | 'others' | 'unknown';

// Summary type
export type SummaryType = 'key_points' | 'action_items' | 'full';

// Meeting entity
export interface Meeting {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  duration_ms: number | null;
  audio_path_you: string | null;
  audio_path_others: string | null;
  audio_format: string;
  status: MeetingStatus;
  error_message: string | null;
  tags: string[];
  notes_count: number;
}

// Transcript segment
export interface TranscriptSegment {
  id: number | null;
  meeting_id: string;
  start_ms: number;
  end_ms: number;
  text: string;
  speaker: Speaker;
  confidence: number | null;
  embedding_id: string | null;
}

// Note
export interface Note {
  id: number | null;
  meeting_id: string;
  content: string;
  created_at: number;
  updated_at: number;
  embedding_id: string | null;
}

// Summary
export interface Summary {
  id: number | null;
  meeting_id: string;
  summary_type: SummaryType;
  content: string;
  model_used: string | null;
  created_at: number;
  embedding_id: string | null;
}

// Search results
export interface SearchHit {
  segment_id: number;
  meeting_id: string;
  meeting_title: string;
  meeting_date: number;
  start_ms: number;
  end_ms: number;
  text: string;
  speaker: string;
  rank: number;
}

export interface SearchHitWithSnippet {
  segment_id: number;
  meeting_id: string;
  meeting_title: string;
  meeting_date: number;
  start_ms: number;
  end_ms: number;
  speaker: string;
  snippet: string;
  rank: number;
}

export interface VectorSearchResult {
  id: string;
  meeting_id: string;
  chunk_type: string;
  text: string;
  start_ms: number | null;
  similarity: number;
}

// Stats
export interface DatabaseStats {
  meeting_count: number;
  segment_count: number;
  total_duration_ms: number;
}

export interface StorageStats {
  raw_bytes: number;
  archived_bytes: number;
  total_bytes: number;
}
```

### API Client

Create `src/lib/storage-api.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';
import type {
  Meeting,
  TranscriptSegment,
  SearchHitWithSnippet,
  VectorSearchResult,
  DatabaseStats,
  StorageStats,
  MeetingStatus,
} from '../types/storage';

// ==================== Meetings ====================

export async function createMeeting(title?: string): Promise<Meeting> {
  return invoke('create_meeting', { title });
}

export async function getMeeting(id: string): Promise<Meeting | null> {
  return invoke('get_meeting', { id });
}

export interface ListMeetingsOptions {
  status?: MeetingStatus;
  search?: string;
  limit?: number;
  offset?: number;
}

export async function listMeetings(options: ListMeetingsOptions = {}): Promise<Meeting[]> {
  return invoke('list_meetings', options);
}

export async function updateMeeting(meeting: Meeting): Promise<void> {
  return invoke('update_meeting', { meeting });
}

export async function deleteMeeting(id: string): Promise<boolean> {
  return invoke('delete_meeting', { id });
}

// ==================== Transcripts ====================

export async function getTranscript(meetingId: string): Promise<TranscriptSegment[]> {
  return invoke('get_transcript', { meetingId });
}

export async function getFullTranscriptText(meetingId: string): Promise<string> {
  return invoke('get_full_transcript_text', { meetingId });
}

// ==================== Search ====================

export async function searchTranscripts(
  query: string,
  limit?: number
): Promise<SearchHitWithSnippet[]> {
  return invoke('search_transcripts', { query, limit });
}

export async function vectorSearch(
  queryVector: number[],
  limit?: number,
  meetingFilter?: string
): Promise<VectorSearchResult[]> {
  return invoke('vector_search', { queryVector, limit, meetingFilter });
}

// ==================== Stats ====================

export async function getDatabaseStats(): Promise<DatabaseStats> {
  return invoke('get_database_stats');
}

export async function getStorageStats(): Promise<StorageStats> {
  return invoke('get_storage_stats');
}

export async function getVectorCount(): Promise<number> {
  return invoke('get_vector_count');
}

// ==================== Utilities ====================

export function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  } else {
    return `${seconds}s`;
  }
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';

  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

export function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
```

### React Hooks

Create `src/hooks/useMeetings.ts`:

```typescript
import { useState, useEffect, useCallback } from 'react';
import * as api from '../lib/storage-api';
import type { Meeting, MeetingStatus } from '../types/storage';

interface UseMeetingsOptions {
  status?: MeetingStatus;
  search?: string;
  limit?: number;
}

export function useMeetings(options: UseMeetingsOptions = {}) {
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchMeetings = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const result = await api.listMeetings(options);
      setMeetings(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load meetings');
    } finally {
      setLoading(false);
    }
  }, [options.status, options.search, options.limit]);

  useEffect(() => {
    fetchMeetings();
  }, [fetchMeetings]);

  const refresh = useCallback(() => {
    fetchMeetings();
  }, [fetchMeetings]);

  return { meetings, loading, error, refresh };
}

export function useMeeting(id: string | null) {
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) {
      setMeeting(null);
      return;
    }

    setLoading(true);
    setError(null);

    api.getMeeting(id)
      .then(setMeeting)
      .catch((err) => {
        setError(err instanceof Error ? err.message : 'Failed to load meeting');
      })
      .finally(() => setLoading(false));
  }, [id]);

  const update = useCallback(async (updates: Partial<Meeting>) => {
    if (!meeting) return;

    const updated = { ...meeting, ...updates };
    await api.updateMeeting(updated);
    setMeeting(updated);
  }, [meeting]);

  const remove = useCallback(async () => {
    if (!meeting) return false;
    return api.deleteMeeting(meeting.id);
  }, [meeting]);

  return { meeting, loading, error, update, remove };
}
```

Create `src/hooks/useSearch.ts`:

```typescript
import { useState, useCallback } from 'react';
import * as api from '../lib/storage-api';
import type { SearchHitWithSnippet } from '../types/storage';

export function useSearch() {
  const [results, setResults] = useState<SearchHitWithSnippet[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  const search = useCallback(async (searchQuery: string, limit?: number) => {
    if (!searchQuery.trim()) {
      setResults([]);
      return;
    }

    setLoading(true);
    setError(null);
    setQuery(searchQuery);

    try {
      const hits = await api.searchTranscripts(searchQuery, limit);
      setResults(hits);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Search failed');
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, []);

  const clear = useCallback(() => {
    setResults([]);
    setQuery('');
    setError(null);
  }, []);

  return { results, loading, error, query, search, clear };
}
```

---

## Migration Strategy

### Version Tracking

Create `src-tauri/src/storage/migrations.rs`:

```rust
use rusqlite::Connection;
use anyhow::{Result, Context};
use tracing::{info, debug};

/// Database version - increment when schema changes
const CURRENT_VERSION: i32 = 1;

/// Run all pending migrations
pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Create migrations table if not exists
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    
    let current_version = get_current_version(conn)?;
    
    if current_version < CURRENT_VERSION {
        info!(
            "Running migrations from version {} to {}",
            current_version, CURRENT_VERSION
        );
        
        for version in (current_version + 1)..=CURRENT_VERSION {
            run_migration(conn, version)?;
            record_migration(conn, version)?;
        }
    } else {
        debug!("Database is up to date (version {})", current_version);
    }
    
    Ok(())
}

fn get_current_version(conn: &Connection) -> Result<i32> {
    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    
    Ok(version)
}

fn record_migration(conn: &Connection, version: i32) -> Result<()> {
    conn.execute(
        "INSERT INTO _migrations (version, applied_at) VALUES (?, ?)",
        [version as i64, chrono::Utc::now().timestamp_millis()],
    )?;
    
    info!("Applied migration version {}", version);
    Ok(())
}

fn run_migration(conn: &Connection, version: i32) -> Result<()> {
    match version {
        1 => migration_v1(conn)?,
        _ => anyhow::bail!("Unknown migration version: {}", version),
    }
    
    Ok(())
}

/// Initial schema - version 1
fn migration_v1(conn: &Connection) -> Result<()> {
    // Schema is created in schema.sql during initialization
    // This migration is a placeholder for future schema changes
    debug!("Migration v1: Initial schema (no-op)");
    Ok(())
}

// Future migrations would be added here:
// fn migration_v2(conn: &Connection) -> Result<()> {
//     conn.execute("ALTER TABLE meetings ADD COLUMN participants TEXT", [])?;
//     Ok(())
// }
```

---

## Performance Optimization

### SQLite Tuning

```rust
// Apply in Database::open()
conn.execute_batch(r#"
    -- Write-ahead logging for concurrent reads
    PRAGMA journal_mode = WAL;
    
    -- Sync less often (safe with WAL)
    PRAGMA synchronous = NORMAL;
    
    -- 64MB cache
    PRAGMA cache_size = -64000;
    
    -- Store temp tables in memory
    PRAGMA temp_store = MEMORY;
    
    -- Memory-mapped I/O (256MB)
    PRAGMA mmap_size = 268435456;
    
    -- Enforce foreign keys
    PRAGMA foreign_keys = ON;
    
    -- 5 second busy timeout
    PRAGMA busy_timeout = 5000;
"#)?;
```

### LanceDB Optimization

```rust
// For large batch inserts
impl VectorStore {
    pub async fn add_embeddings_batch(
        &self,
        records: Vec<EmbeddingRecord>,
        batch_size: usize,
    ) -> Result<()> {
        for chunk in records.chunks(batch_size) {
            self.add_embeddings(chunk.to_vec()).await?;
        }
        
        // Optimize after large insert
        if records.len() > 1000 {
            self.optimize().await?;
        }
        
        Ok(())
    }
    
    pub async fn optimize(&self) -> Result<()> {
        let table = self.db.open_table(&self.table_name).execute().await?;
        table.optimize().execute().await?;
        Ok(())
    }
}
```

### Query Performance Tips

| Query Type | Optimization |
|------------|-------------|
| **List meetings** | Use indexed columns (created_at, status) |
| **Search transcripts** | Use FTS5 with MATCH, not LIKE |
| **Vector search** | Limit results, use filters to narrow scope |
| **Batch inserts** | Use transactions, batch size 100-500 |
| **Full text** | Pre-compute and cache common queries |

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_database_crud() {
        let temp = TempDir::new().unwrap();
        let db = Database::open(temp.path().join("test.db")).unwrap();
        db.initialize().unwrap();
        
        let repos = MeetingRepository::new(db);
        
        // Create
        let meeting = Meeting::new("Test Meeting");
        repos.create(&meeting).unwrap();
        
        // Read
        let loaded = repos.get(&meeting.id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().title, "Test Meeting");
        
        // Update
        let mut updated = meeting.clone();
        updated.title = "Updated Title".to_string();
        repos.update(&updated).unwrap();
        
        let loaded = repos.get(&meeting.id).unwrap().unwrap();
        assert_eq!(loaded.title, "Updated Title");
        
        // Delete
        let deleted = repos.delete(&meeting.id).unwrap();
        assert!(deleted);
        assert!(repos.get(&meeting.id).unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_vector_search() {
        let temp = TempDir::new().unwrap();
        let store = VectorStore::open(temp.path()).await.unwrap();
        store.initialize().await.unwrap();
        
        // Add test embeddings
        let records = vec![
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "Test transcript about AI",
                0,
                vec![0.1; EMBEDDING_DIM],
            ),
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "Test transcript about ML",
                1000,
                vec![0.2; EMBEDDING_DIM],
            ),
        ];
        
        store.add_embeddings(records).await.unwrap();
        
        // Search
        let query = vec![0.1; EMBEDDING_DIM];
        let results = store.search(&query, 10, None).await.unwrap();
        
        assert!(!results.is_empty());
    }
}
```

### Integration Test

```rust
#[tokio::test]
async fn test_full_workflow() {
    let temp = TempDir::new().unwrap();
    let storage = initialize_storage(temp.path()).await.unwrap();
    let repos = storage.repositories();
    
    // Create meeting
    let meeting = Meeting::new("Integration Test");
    repos.meetings.create(&meeting).unwrap();
    
    // Add transcript segments
    let segments = vec![
        TranscriptSegment::new(&meeting.id, 0, 5000, "Hello world", Speaker::You),
        TranscriptSegment::new(&meeting.id, 5000, 10000, "Hi there", Speaker::Others),
    ];
    repos.transcripts.insert_batch(&segments).unwrap();
    
    // Verify transcript
    let loaded = repos.transcripts.get_by_meeting(&meeting.id).unwrap();
    assert_eq!(loaded.len(), 2);
    
    // Search
    let results = storage.search.search_transcripts("hello", 10).unwrap();
    assert!(!results.is_empty());
    
    // Cleanup
    repos.meetings.delete(&meeting.id).unwrap();
}
```

---

## Troubleshooting

### Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| **"database is locked"** | Concurrent writes | Increase busy_timeout, use transactions |
| **"no such table"** | Schema not initialized | Call db.initialize() on startup |
| **FTS results empty** | Triggers not working | Run `rebuild_index()` |
| **Vector search slow** | Index not optimized | Call `optimize()` after bulk inserts |
| **"foreign key violation"** | Missing parent record | Create meeting before segments |

### Debug Logging

```rust
// Enable SQL logging
std::env::set_var("RUST_LOG", "rusqlite=debug");

// Enable LanceDB logging
std::env::set_var("RUST_LOG", "lancedb=debug");
```

### Database Inspection

```bash
# Open SQLite database
sqlite3 ~/.meeting-scribe/data/meetings.db

# Check schema
.schema

# Count records
SELECT COUNT(*) FROM meetings;
SELECT COUNT(*) FROM transcript_segments;

# Check FTS index
SELECT * FROM transcript_fts WHERE transcript_fts MATCH 'test';
```

---

## Acceptance Criteria

### Required

- [ ] SQLite database creates and initializes correctly
- [ ] CRUD operations work for meetings
- [ ] Transcript segments insert and retrieve correctly
- [ ] Full-text search returns relevant results
- [ ] LanceDB vector store initializes
- [ ] Vector search returns similar embeddings
- [ ] Foreign key cascades delete related data
- [ ] Storage stats report correctly

### Nice to Have

- [ ] Migrations run automatically
- [ ] WAL mode enabled for performance
- [ ] Batch operations use transactions
- [ ] Search highlights work correctly
- [ ] Audio archival compresses files

---

## Next Steps

After completing the storage layer:

1. **[06-embedding-engine.md](./06-embedding-engine.md)** - Generate embeddings for transcripts
2. **[07-llm-engine.md](./07-llm-engine.md)** - Summarization with llama-cpp
3. **[09-rag-implementation.md](./09-rag-implementation.md)** - Connect search + LLM for RAG

---

## References

### Documentation

- [rusqlite Guide](https://docs.rs/rusqlite/latest/rusqlite/)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)
- [LanceDB Rust](https://lancedb.github.io/lancedb/basic/)
- [Arrow Rust](https://arrow.apache.org/rust/arrow/index.html)

### Examples

- [rusqlite examples](https://github.com/rusqlite/rusqlite/tree/master/examples)
- [LanceDB examples](https://github.com/lancedb/lancedb/tree/main/rust/lancedb/examples)

### Performance

- [SQLite Performance Tips](https://www.sqlite.org/optoverview.html)
- [WAL Mode](https://www.sqlite.org/wal.html)
- [LanceDB Optimization](https://lancedb.github.io/lancedb/guides/optimization/)
