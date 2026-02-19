//! LanceDB vector store
//!
//! Vector database for semantic search using embeddings.

use anyhow::{Context, Result};
use arrow_array::{
    types::Float32Type, Array, ArrayRef, FixedSizeListArray, Float32Array, Int64Array, RecordBatch,
    RecordBatchIterator, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{connect, Connection, Table};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Embedding dimension for EmbeddingGemma (768-dim vectors)
pub const EMBEDDING_DIM: usize = 768;

fn quote_lance_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Vector store for semantic search
pub struct VectorStore {
    db: Connection,
    table_name: String,
    reindex_required: AtomicBool,
    reindex_marker_path: PathBuf,
}

impl VectorStore {
    /// Connect to or create a LanceDB database
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        // Create directory if needed
        std::fs::create_dir_all(path).context("Failed to create vector store directory")?;

        let uri = path.to_string_lossy().to_string();
        let db = connect(&uri)
            .execute()
            .await
            .context("Failed to connect to LanceDB")?;

        info!("Connected to vector store at {:?}", path);

        let reindex_marker_path = path.join("reindex_required.flag");
        let reindex_required = reindex_marker_path.exists();

        Ok(Self {
            db,
            table_name: "embeddings".to_string(),
            reindex_required: AtomicBool::new(reindex_required),
            reindex_marker_path,
        })
    }

    /// Get the schema for the embeddings table
    fn embeddings_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("meeting_id", DataType::Utf8, false),
            Field::new("chunk_type", DataType::Utf8, false), // "transcript", "note", "summary"
            Field::new("text", DataType::Utf8, false),
            Field::new("start_ms", DataType::Int64, true), // For transcript chunks
            Field::new("end_ms", DataType::Int64, true),   // For transcript chunks
            Field::new("chunk_index", DataType::Int64, true),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM as i32,
                ),
                false,
            ),
        ]))
    }

    /// Initialize the embeddings table
    pub async fn initialize(&self) -> Result<()> {
        self.ensure_table_schema().await
    }

    async fn ensure_table_schema(&self) -> Result<()> {
        // Check if table exists
        let tables: Vec<String> = self.db.table_names().execute().await?;

        if !tables.contains(&self.table_name) {
            self.create_embeddings_table().await?;
            return Ok(());
        }

        let table = self.get_table().await?;
        let existing_schema = table
            .schema()
            .await
            .context("Failed to inspect embeddings schema")?;

        if Self::is_compatible_schema(existing_schema.as_ref()) {
            debug!("Embeddings table already exists with compatible schema");
            return Ok(());
        }

        warn!(
            "Embeddings table schema is incompatible with current version; rebuilding local vector index"
        );
        let namespace: Vec<String> = Vec::new();
        self.db
            .drop_table(&self.table_name, &namespace)
            .await
            .context("Failed to drop incompatible embeddings table")?;
        self.create_embeddings_table().await?;
        self.mark_reindex_required(true)?;

        Ok(())
    }

    fn mark_reindex_required(&self, required: bool) -> Result<()> {
        self.reindex_required.store(required, Ordering::SeqCst);

        if required {
            std::fs::write(
                &self.reindex_marker_path,
                b"Vector schema changed; transcript reindex required.",
            )
            .context("Failed to create vector reindex marker file")?;
        } else if self.reindex_marker_path.exists() {
            std::fs::remove_file(&self.reindex_marker_path)
                .context("Failed to clear vector reindex marker file")?;
        }

        Ok(())
    }

    pub fn is_reindex_required(&self) -> bool {
        self.reindex_required.load(Ordering::SeqCst) || self.reindex_marker_path.exists()
    }

    pub fn clear_reindex_required(&self) -> Result<()> {
        self.mark_reindex_required(false)
    }

    async fn create_embeddings_table(&self) -> Result<()> {
        let schema = Self::embeddings_schema();

        let _table: Table = self
            .db
            .create_empty_table(&self.table_name, schema)
            .execute()
            .await
            .context("Failed to create embeddings table")?;

        info!("Created embeddings table");
        Ok(())
    }

    fn is_compatible_schema(schema: &Schema) -> bool {
        const REQUIRED_FIELDS: [&str; 8] = [
            "id",
            "meeting_id",
            "chunk_type",
            "text",
            "start_ms",
            "end_ms",
            "chunk_index",
            "vector",
        ];

        REQUIRED_FIELDS
            .iter()
            .all(|field_name| schema.field_with_name(field_name).is_ok())
    }

    /// Get or open the table
    async fn get_table(&self) -> Result<Table> {
        let table: Table = self
            .db
            .open_table(&self.table_name)
            .execute()
            .await
            .context("Failed to open embeddings table")?;
        Ok(table)
    }

    /// Add embeddings to the store
    pub async fn add_embeddings(&self, records: Vec<EmbeddingRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let table = self.get_table().await?;
        let batch = self.records_to_batch(&records)?;
        let schema = batch.schema();
        let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table
            .add(Box::new(batches))
            .execute()
            .await
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
        let end_ms_vals: Vec<Option<i64>> = records.iter().map(|r| r.end_ms).collect();
        let chunk_index_vals: Vec<Option<i64>> = records.iter().map(|r| r.chunk_index).collect();

        // Create fixed-size list array for vectors
        let vectors: Vec<Option<Vec<Option<f32>>>> = records
            .iter()
            .map(|r| Some(r.vector.iter().map(|v| Some(*v)).collect()))
            .collect();

        let id_array: ArrayRef = Arc::new(StringArray::from(ids));
        let meeting_id_array: ArrayRef = Arc::new(StringArray::from(meeting_ids));
        let chunk_type_array: ArrayRef = Arc::new(StringArray::from(chunk_types));
        let text_array: ArrayRef = Arc::new(StringArray::from(texts));
        let start_ms_array: ArrayRef = Arc::new(Int64Array::from(start_ms_vals));
        let end_ms_array: ArrayRef = Arc::new(Int64Array::from(end_ms_vals));
        let chunk_index_array: ArrayRef = Arc::new(Int64Array::from(chunk_index_vals));
        let vector_array: ArrayRef = Arc::new(FixedSizeListArray::from_iter_primitive::<
            Float32Type,
            _,
            _,
        >(vectors, EMBEDDING_DIM as i32));

        RecordBatch::try_new(
            Self::embeddings_schema(),
            vec![
                id_array,
                meeting_id_array,
                chunk_type_array,
                text_array,
                start_ms_array,
                end_ms_array,
                chunk_index_array,
                vector_array,
            ],
        )
        .context("Failed to create record batch")
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

        let table = self.get_table().await?;

        let mut query = table
            .vector_search(query_vector.to_vec())
            .context("Failed to create vector search")?
            .limit(limit);

        // Apply filter if provided (e.g., "meeting_id = 'abc123'")
        if let Some(filter_expr) = filter {
            query = query.only_if(filter_expr);
        }

        let mut results = query
            .execute()
            .await
            .context("Failed to execute vector search")?;

        let mut search_results = Vec::new();

        while let Some(batch) = results.try_next().await? {
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
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let end_ms_arr = batch
                .column_by_name("end_ms")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let chunk_index_arr = batch
                .column_by_name("chunk_index")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());

            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            for i in 0..batch.num_rows() {
                let distance = distances.map(|d| d.value(i)).unwrap_or(0.0);
                // Distance metric can vary by index config; keep conversion monotonic,
                // bounded, and robust so retrieval scoring stays stable.
                let safe_distance = if distance.is_finite() && distance > 0.0 {
                    distance
                } else {
                    0.0
                };
                let similarity = 1.0 / (1.0 + safe_distance);

                search_results.push(SearchResult {
                    id: ids.value(i).to_string(),
                    meeting_id: meeting_ids.value(i).to_string(),
                    chunk_type: chunk_types.value(i).to_string(),
                    text: texts.value(i).to_string(),
                    start_ms: start_ms_arr.and_then(|arr| {
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    }),
                    end_ms: end_ms_arr.and_then(|arr| {
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    }),
                    chunk_index: chunk_index_arr.and_then(|arr| {
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    }),
                    similarity,
                    distance,
                });
            }
        }

        debug!("Vector search returned {} results", search_results.len());
        Ok(search_results)
    }

    /// Fetch transcript chunks around a chunk index.
    pub async fn transcript_chunks_near_index(
        &self,
        meeting_id: &str,
        chunk_index: i64,
        radius: i64,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let table = self.get_table().await?;
        let start = chunk_index.saturating_sub(radius.max(0));
        let end = chunk_index.saturating_add(radius.max(0));
        let filter = format!(
            "meeting_id = {} AND chunk_type = 'transcript' AND chunk_index >= {} AND chunk_index <= {}",
            quote_lance_string(meeting_id),
            start,
            end
        );

        let mut batches = table
            .query()
            .only_if(&filter)
            .select(Select::columns(&[
                "id",
                "meeting_id",
                "chunk_type",
                "text",
                "start_ms",
                "end_ms",
                "chunk_index",
            ]))
            .limit(limit.max(1))
            .execute()
            .await
            .context("Failed to query adjacent transcript chunks")?;

        let mut results = Vec::new();
        while let Some(batch) = batches.try_next().await? {
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
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let end_ms_arr = batch
                .column_by_name("end_ms")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let chunk_index_arr = batch
                .column_by_name("chunk_index")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());

            for i in 0..batch.num_rows() {
                results.push(SearchResult {
                    id: ids.value(i).to_string(),
                    meeting_id: meeting_ids.value(i).to_string(),
                    chunk_type: chunk_types.value(i).to_string(),
                    text: texts.value(i).to_string(),
                    start_ms: start_ms_arr.and_then(|arr| {
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    }),
                    end_ms: end_ms_arr.and_then(|arr| {
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    }),
                    chunk_index: chunk_index_arr.and_then(|arr| {
                        if arr.is_null(i) {
                            None
                        } else {
                            Some(arr.value(i))
                        }
                    }),
                    similarity: 0.0,
                    distance: 1.0,
                });
            }
        }

        results.sort_by(|a, b| {
            let index_cmp = a
                .chunk_index
                .unwrap_or(i64::MAX)
                .cmp(&b.chunk_index.unwrap_or(i64::MAX));
            if index_cmp == std::cmp::Ordering::Equal {
                a.start_ms
                    .unwrap_or(i64::MAX)
                    .cmp(&b.start_ms.unwrap_or(i64::MAX))
            } else {
                index_cmp
            }
        });
        Ok(results)
    }

    /// Delete embeddings for a meeting
    pub async fn delete_meeting_embeddings(&self, meeting_id: &str) -> Result<u64> {
        let table = self.get_table().await?;

        // LanceDB delete uses SQL-like filter
        table
            .delete(&format!("meeting_id = {}", quote_lance_string(meeting_id)))
            .await
            .context("Failed to delete embeddings")?;

        debug!("Deleted embeddings for meeting {}", meeting_id);
        // LanceDB doesn't return count, so we return 0
        Ok(0)
    }

    /// Delete a specific embedding by ID
    pub async fn delete_embedding(&self, id: &str) -> Result<()> {
        let table = self.get_table().await?;

        table
            .delete(&format!("id = {}", quote_lance_string(id)))
            .await
            .context("Failed to delete embedding")?;

        debug!("Deleted embedding {}", id);
        Ok(())
    }

    /// Get embedding count
    pub async fn count(&self) -> Result<u64> {
        let table = self.get_table().await?;
        let count = table
            .count_rows(None)
            .await
            .context("Failed to count embeddings")?;
        Ok(count as u64)
    }

    /// Get embedding count for a meeting
    pub async fn count_for_meeting(&self, meeting_id: &str) -> Result<u64> {
        let table = self.get_table().await?;
        let filter = format!("meeting_id = {}", quote_lance_string(meeting_id));
        let count = table
            .count_rows(Some(filter))
            .await
            .context("Failed to count embeddings")?;
        Ok(count as u64)
    }

    /// Optimize the table (compact and create index)
    pub async fn optimize(&self) -> Result<()> {
        let table = self.get_table().await?;
        table.optimize(Default::default()).await?;
        info!("Optimized vector store");
        Ok(())
    }
}

/// Embedding record for storage
#[derive(Debug, Clone)]
pub struct EmbeddingRecord {
    /// Unique ID for this embedding
    pub id: String,
    /// Meeting this embedding belongs to
    pub meeting_id: String,
    /// Type of content: "transcript", "note", "summary"
    pub chunk_type: String,
    /// Original text that was embedded
    pub text: String,
    /// Start time for transcript chunks
    pub start_ms: Option<i64>,
    /// End time for transcript chunks
    pub end_ms: Option<i64>,
    /// Chunk index within the source document/transcript
    pub chunk_index: Option<i64>,
    /// Embedding vector
    pub vector: Vec<f32>,
}

impl EmbeddingRecord {
    /// Create a new embedding record for transcript text
    pub fn new_transcript(
        meeting_id: &str,
        text: &str,
        start_ms: i64,
        end_ms: i64,
        chunk_index: usize,
        vector: Vec<f32>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: meeting_id.to_string(),
            chunk_type: "transcript".to_string(),
            text: text.to_string(),
            start_ms: Some(start_ms),
            end_ms: Some(end_ms),
            chunk_index: Some(chunk_index as i64),
            vector,
        }
    }

    /// Create a new embedding record for a note
    pub fn new_note(meeting_id: &str, text: &str, vector: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: meeting_id.to_string(),
            chunk_type: "note".to_string(),
            text: text.to_string(),
            start_ms: None,
            end_ms: None,
            chunk_index: None,
            vector,
        }
    }

    /// Create a new embedding record for a summary
    pub fn new_summary(meeting_id: &str, text: &str, vector: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            meeting_id: meeting_id.to_string(),
            chunk_type: "summary".to_string(),
            text: text.to_string(),
            start_ms: None,
            end_ms: None,
            chunk_index: None,
            vector,
        }
    }
}

/// Search result from vector store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Embedding ID
    pub id: String,
    /// Meeting this embedding belongs to
    pub meeting_id: String,
    /// Type of content
    pub chunk_type: String,
    /// Original text
    pub text: String,
    /// Start time for transcripts
    pub start_ms: Option<i64>,
    /// End time for transcripts
    pub end_ms: Option<i64>,
    /// Chunk index within source
    pub chunk_index: Option<i64>,
    /// Similarity score (0-1, higher is more similar)
    pub similarity: f32,
    /// Raw distance from query
    pub distance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_test_store() -> (TempDir, VectorStore) {
        let temp: TempDir = TempDir::new().unwrap();
        let store: VectorStore = VectorStore::open(temp.path()).await.unwrap();
        store.initialize().await.unwrap();
        (temp, store)
    }

    fn make_test_vector(seed: f32) -> Vec<f32> {
        (0..EMBEDDING_DIM)
            .map(|i| (i as f32 * seed) % 1.0)
            .collect()
    }

    #[tokio::test]
    async fn test_vector_store_initialization() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        // Should be able to count (table exists)
        let count: u64 = store.count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_add_and_count_embeddings() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        let records = vec![
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "Hello world",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "How are you",
                5000,
                6000,
                1,
                make_test_vector(0.2),
            ),
        ];

        store.add_embeddings(records).await.unwrap();

        let count: u64 = store.count().await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_vector_search() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        // Add some embeddings
        let records = vec![
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "AI and machine learning discussion",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "Database design patterns",
                5000,
                6000,
                1,
                make_test_vector(0.5),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-2",
                "Rust programming tips",
                0,
                1000,
                0,
                make_test_vector(0.9),
            ),
        ];

        store.add_embeddings(records).await.unwrap();

        // Search with a similar vector
        let query = make_test_vector(0.1);
        let results: Vec<SearchResult> = store.search(&query, 10, None).await.unwrap();

        assert!(!results.is_empty());
        // First result should be most similar to our query
        assert!(results[0].similarity > 0.0);
    }

    #[tokio::test]
    async fn test_search_with_filter() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        let records = vec![
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "First meeting",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-2",
                "Second meeting",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
        ];

        store.add_embeddings(records).await.unwrap();

        // Search only in meeting-1
        let query = make_test_vector(0.1);
        let results: Vec<SearchResult> = store
            .search(&query, 10, Some("meeting_id = 'meeting-1'"))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].meeting_id, "meeting-1");
    }

    #[tokio::test]
    async fn test_delete_meeting_embeddings() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        let records = vec![
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "First",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "Second",
                5000,
                6000,
                1,
                make_test_vector(0.2),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-2",
                "Other",
                0,
                1000,
                0,
                make_test_vector(0.3),
            ),
        ];

        store.add_embeddings(records).await.unwrap();

        // Delete meeting-1 embeddings
        store.delete_meeting_embeddings("meeting-1").await.unwrap();

        // Check counts
        let count: u64 = store.count().await.unwrap();
        assert_eq!(count, 1);

        let meeting_2_count: u64 = store.count_for_meeting("meeting-2").await.unwrap();
        assert_eq!(meeting_2_count, 1);
    }

    #[tokio::test]
    async fn test_filters_escape_single_quotes_in_meeting_id() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;
        let meeting_id = "meeting-o'hare";

        let records = vec![
            EmbeddingRecord::new_transcript(
                meeting_id,
                "Quoted ID content",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
            EmbeddingRecord::new_transcript(
                "meeting-2",
                "Other content",
                0,
                1000,
                0,
                make_test_vector(0.2),
            ),
        ];

        store.add_embeddings(records).await.unwrap();

        let quoted_count: u64 = store.count_for_meeting(meeting_id).await.unwrap();
        assert_eq!(quoted_count, 1);

        store.delete_meeting_embeddings(meeting_id).await.unwrap();

        let remaining_quoted: u64 = store.count_for_meeting(meeting_id).await.unwrap();
        assert_eq!(remaining_quoted, 0);
        let remaining_other: u64 = store.count_for_meeting("meeting-2").await.unwrap();
        assert_eq!(remaining_other, 1);
    }

    #[tokio::test]
    async fn test_different_chunk_types() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        let records = vec![
            EmbeddingRecord::new_transcript(
                "meeting-1",
                "Transcript text",
                0,
                1000,
                0,
                make_test_vector(0.1),
            ),
            EmbeddingRecord::new_note("meeting-1", "Note content", make_test_vector(0.2)),
            EmbeddingRecord::new_summary("meeting-1", "Summary content", make_test_vector(0.3)),
        ];

        store.add_embeddings(records).await.unwrap();

        let count: u64 = store.count().await.unwrap();
        assert_eq!(count, 3);

        // Search and check types
        let results: Vec<SearchResult> = store
            .search(&make_test_vector(0.2), 10, None)
            .await
            .unwrap();

        let types: Vec<&str> = results.iter().map(|r| r.chunk_type.as_str()).collect();
        assert!(types.contains(&"transcript"));
        assert!(types.contains(&"note"));
        assert!(types.contains(&"summary"));
    }

    #[tokio::test]
    async fn test_wrong_dimension_error() {
        let (_temp, store): (TempDir, VectorStore) = setup_test_store().await;

        // Try to search with wrong dimension
        let wrong_dim_query = vec![0.1f32; 128]; // Wrong dimension
        let result: Result<Vec<SearchResult>> = store.search(&wrong_dim_query, 10, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("wrong dimension"));
    }
}
