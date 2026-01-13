# 09 - RAG Implementation: Retrieval-Augmented Generation for Meeting Chat

## Goal
Implement a complete RAG (Retrieval-Augmented Generation) system that allows users to chat with their meeting history. Combine vector similarity search with LLM generation to provide accurate, contextual answers grounded in actual meeting content.

**Estimated Time:** 4-5 days

## Prerequisites
- Document 05 (Storage Layer) completed - SQLite and LanceDB ready
- Document 06 (Embedding Engine) completed - Text embeddings working
- Document 07 (LLM Engine) completed - llama-cpp-2 integration ready
- Document 08 (Frontend UI) completed - Chat interface available

## Technology Overview

### RAG Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           RAG Pipeline                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  User Query                                                                  │
│      │                                                                       │
│      ▼                                                                       │
│  ┌────────────────┐    ┌────────────────┐    ┌────────────────────────┐    │
│  │  Query         │    │  Vector        │    │  Context               │    │
│  │  Embedding     │───▶│  Search        │───▶│  Building              │    │
│  │                │    │  (LanceDB)     │    │                        │    │
│  └────────────────┘    └────────────────┘    └────────────────────────┘    │
│                                                       │                      │
│                                                       ▼                      │
│                              ┌────────────────────────────────────────┐     │
│                              │           LLM Generation               │     │
│                              │  (System prompt + Context + Query)     │     │
│                              └────────────────────────────────────────┘     │
│                                                       │                      │
│                                                       ▼                      │
│                              ┌────────────────────────────────────────┐     │
│                              │         Response + Sources             │     │
│                              └────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Components

| Component | Purpose | Technology |
|-----------|---------|------------|
| **Query Embedding** | Convert query to vector | EmbeddingGemma (ONNX) |
| **Vector Search** | Find similar chunks | LanceDB |
| **Context Builder** | Assemble relevant context | Custom Rust |
| **LLM Generation** | Generate response | llama-cpp-2 |
| **Source Tracking** | Cite meeting sources | Custom metadata |

### References

- [LangChain RAG Conceptual Guide](https://python.langchain.com/docs/concepts/rag)
- [LanceDB Vector Search](https://lancedb.github.io/lancedb/search/)
- [Pinecone RAG Best Practices](https://www.pinecone.io/learn/retrieval-augmented-generation/)
- [NVIDIA RAG 101](https://developer.nvidia.com/blog/rag-101-retrieval-augmented-generation-questions-answered/)

## Architecture

### Component Structure

```
src-tauri/src/
├── rag/
│   ├── mod.rs              # Module exports
│   ├── retriever.rs        # Vector search logic
│   ├── context.rs          # Context building
│   ├── generator.rs        # LLM response generation
│   ├── pipeline.rs         # Complete RAG pipeline
│   └── prompts.rs          # RAG-specific prompts
├── commands/
│   └── chat.rs             # Tauri commands for chat
```

### Data Flow

```
1. User sends query: "What did we decide about the API timeline?"
                │
                ▼
2. Embed query using EmbeddingGemma with query_prompt()
                │
                ▼
3. Search LanceDB for top-k similar chunks (k=5 default)
                │
                ▼
4. Build context with retrieved chunks + meeting metadata
                │
                ▼
5. Generate prompt: System + Context + Query + History
                │
                ▼
6. Stream LLM response with source citations
                │
                ▼
7. Return response + source references to frontend
```

## Implementation

### Step 1: Retriever Module

**File: `src-tauri/src/rag/retriever.rs`**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::inference::embedding::EmbeddingService;
use crate::storage::vectors::VectorStore;
use crate::storage::sqlite::Database;

/// A retrieved chunk with relevance metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    /// Unique chunk identifier
    pub id: String,
    /// Meeting this chunk belongs to
    pub meeting_id: String,
    /// Meeting title for display
    pub meeting_title: String,
    /// Meeting date
    pub meeting_date: i64,
    /// Type of chunk: "transcript", "note", "summary"
    pub chunk_type: String,
    /// The actual text content
    pub text: String,
    /// Start timestamp in milliseconds (for transcript chunks)
    pub start_ms: Option<i64>,
    /// End timestamp (for transcript chunks)
    pub end_ms: Option<i64>,
    /// Speaker if available
    pub speaker: Option<String>,
    /// Cosine similarity score (0-1)
    pub similarity: f32,
}

/// Configuration for retrieval
#[derive(Debug, Clone)]
pub struct RetrieverConfig {
    /// Number of chunks to retrieve
    pub top_k: usize,
    /// Minimum similarity threshold (0-1)
    pub min_similarity: f32,
    /// Whether to include notes
    pub include_notes: bool,
    /// Whether to include summaries
    pub include_summaries: bool,
    /// Filter to specific meeting IDs
    pub meeting_filter: Option<Vec<String>>,
    /// Maximum total context length (in chars)
    pub max_context_chars: usize,
}

impl Default for RetrieverConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            min_similarity: 0.3,
            include_notes: true,
            include_summaries: true,
            meeting_filter: None,
            max_context_chars: 8000,
        }
    }
}

/// Retriever handles vector search and chunk retrieval
pub struct Retriever {
    embedding_service: Arc<EmbeddingService>,
    vector_store: Arc<VectorStore>,
    database: Arc<Database>,
}

impl Retriever {
    pub fn new(
        embedding_service: Arc<EmbeddingService>,
        vector_store: Arc<VectorStore>,
        database: Arc<Database>,
    ) -> Self {
        Self {
            embedding_service,
            vector_store,
            database,
        }
    }
    
    /// Retrieve relevant chunks for a query
    pub async fn retrieve(
        &self,
        query: &str,
        config: &RetrieverConfig,
    ) -> Result<Vec<RetrievedChunk>> {
        // 1. Generate query embedding with query-specific prompt
        let query_embedding = self.embedding_service
            .embed_for_query(query)
            .await?;
        
        // 2. Build filter conditions
        let mut filters = Vec::new();
        
        // Chunk type filter
        let mut chunk_types = vec!["transcript".to_string()];
        if config.include_notes {
            chunk_types.push("note".to_string());
        }
        if config.include_summaries {
            chunk_types.push("summary".to_string());
        }
        
        // Meeting ID filter
        if let Some(ref meeting_ids) = config.meeting_filter {
            filters.push(format!(
                "meeting_id IN ({})",
                meeting_ids.iter()
                    .map(|id| format!("'{}'", id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        
        // 3. Vector search
        let search_results = self.vector_store
            .search(
                &query_embedding,
                config.top_k * 2, // Fetch extra for filtering
                Some(chunk_types),
                config.meeting_filter.clone(),
            )
            .await?;
        
        // 4. Filter by similarity and enrich with meeting metadata
        let mut chunks = Vec::new();
        let mut total_chars = 0;
        
        for result in search_results {
            // Skip low similarity
            if result.similarity < config.min_similarity {
                continue;
            }
            
            // Get meeting metadata
            let meeting = self.database
                .get_meeting(&result.meeting_id)
                .await?;
            
            let chunk = RetrievedChunk {
                id: result.id,
                meeting_id: result.meeting_id,
                meeting_title: meeting.as_ref()
                    .map(|m| m.title.clone())
                    .unwrap_or_else(|| "Unknown Meeting".to_string()),
                meeting_date: meeting.as_ref()
                    .map(|m| m.created_at)
                    .unwrap_or(0),
                chunk_type: result.chunk_type,
                text: result.text.clone(),
                start_ms: result.start_ms,
                end_ms: result.end_ms,
                speaker: result.speaker,
                similarity: result.similarity,
            };
            
            // Check context length limit
            total_chars += chunk.text.len();
            if total_chars > config.max_context_chars {
                break;
            }
            
            chunks.push(chunk);
            
            if chunks.len() >= config.top_k {
                break;
            }
        }
        
        // 5. Sort by meeting date (most recent first) then by similarity
        chunks.sort_by(|a, b| {
            b.meeting_date.cmp(&a.meeting_date)
                .then(b.similarity.partial_cmp(&a.similarity).unwrap())
        });
        
        Ok(chunks)
    }
    
    /// Retrieve chunks relevant to a specific meeting (for meeting-scoped questions)
    pub async fn retrieve_from_meeting(
        &self,
        query: &str,
        meeting_id: &str,
        top_k: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let config = RetrieverConfig {
            top_k,
            meeting_filter: Some(vec![meeting_id.to_string()]),
            ..Default::default()
        };
        
        self.retrieve(query, &config).await
    }
    
    /// Hybrid search: combine vector search with full-text search
    pub async fn hybrid_retrieve(
        &self,
        query: &str,
        config: &RetrieverConfig,
    ) -> Result<Vec<RetrievedChunk>> {
        // Get vector search results
        let vector_results = self.retrieve(query, config).await?;
        
        // Get FTS results
        let fts_results = self.database
            .search_transcripts(query, config.top_k)
            .await?;
        
        // Merge and deduplicate
        let mut merged = vector_results;
        let existing_ids: std::collections::HashSet<_> = 
            merged.iter().map(|c| c.id.clone()).collect();
        
        for fts in fts_results {
            if !existing_ids.contains(&fts.segment_id.to_string()) {
                // Get full segment data
                if let Ok(Some(segment)) = self.database
                    .get_transcript_segment(fts.segment_id)
                    .await
                {
                    let meeting = self.database
                        .get_meeting(&fts.meeting_id)
                        .await?;
                    
                    merged.push(RetrievedChunk {
                        id: fts.segment_id.to_string(),
                        meeting_id: fts.meeting_id,
                        meeting_title: meeting.as_ref()
                            .map(|m| m.title.clone())
                            .unwrap_or_default(),
                        meeting_date: meeting.as_ref()
                            .map(|m| m.created_at)
                            .unwrap_or(0),
                        chunk_type: "transcript".to_string(),
                        text: segment.text,
                        start_ms: Some(segment.start_ms),
                        end_ms: Some(segment.end_ms),
                        speaker: Some(segment.speaker),
                        // FTS results get lower similarity boost
                        similarity: 0.5 + (fts.rank as f32 * 0.1),
                    });
                }
            }
        }
        
        // Re-sort
        merged.sort_by(|a, b| {
            b.similarity.partial_cmp(&a.similarity).unwrap()
        });
        
        // Limit to top_k
        merged.truncate(config.top_k);
        
        Ok(merged)
    }
}
```

### Step 2: Context Builder

**File: `src-tauri/src/rag/context.rs`**

```rust
use crate::rag::retriever::RetrievedChunk;
use chrono::{DateTime, Utc, TimeZone};

/// Formats retrieved chunks into a context string for the LLM
pub struct ContextBuilder {
    /// Maximum context length in characters
    max_length: usize,
    /// Whether to include timestamps
    include_timestamps: bool,
    /// Whether to include similarity scores (for debugging)
    include_scores: bool,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self {
            max_length: 8000,
            include_timestamps: true,
            include_scores: false,
        }
    }
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_max_length(mut self, length: usize) -> Self {
        self.max_length = length;
        self
    }
    
    pub fn with_timestamps(mut self, include: bool) -> Self {
        self.include_timestamps = include;
        self
    }
    
    /// Build context string from retrieved chunks
    pub fn build(&self, chunks: &[RetrievedChunk]) -> String {
        if chunks.is_empty() {
            return "No relevant meeting content found.".to_string();
        }
        
        let mut context = String::new();
        let mut current_meeting_id = String::new();
        let mut total_length = 0;
        
        for (index, chunk) in chunks.iter().enumerate() {
            // Start new meeting section if meeting changed
            if chunk.meeting_id != current_meeting_id {
                if !current_meeting_id.is_empty() {
                    context.push_str("\n\n");
                }
                
                // Meeting header
                let date = format_timestamp(chunk.meeting_date);
                let header = format!(
                    "=== Meeting: {} ({}) ===\n",
                    chunk.meeting_title, date
                );
                context.push_str(&header);
                current_meeting_id = chunk.meeting_id.clone();
            }
            
            // Format chunk based on type
            let formatted = self.format_chunk(chunk, index);
            
            // Check length limit
            if total_length + formatted.len() > self.max_length {
                context.push_str("\n[Context truncated due to length]");
                break;
            }
            
            context.push_str(&formatted);
            total_length += formatted.len();
        }
        
        context
    }
    
    fn format_chunk(&self, chunk: &RetrievedChunk, index: usize) -> String {
        let mut formatted = String::new();
        
        match chunk.chunk_type.as_str() {
            "transcript" => {
                // Transcript segment
                let timestamp = if self.include_timestamps {
                    if let Some(start_ms) = chunk.start_ms {
                        format!("[{}] ", format_duration_ms(start_ms))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                
                let speaker = chunk.speaker.as_ref()
                    .map(|s| format!("{}: ", capitalize(s)))
                    .unwrap_or_default();
                
                formatted.push_str(&format!(
                    "\n{}{}{}", 
                    timestamp, 
                    speaker, 
                    chunk.text
                ));
            }
            "note" => {
                formatted.push_str(&format!("\n[User Notes]\n{}", chunk.text));
            }
            "summary" => {
                formatted.push_str(&format!("\n[Meeting Summary]\n{}", chunk.text));
            }
            _ => {
                formatted.push_str(&format!("\n{}", chunk.text));
            }
        }
        
        if self.include_scores {
            formatted.push_str(&format!(" (score: {:.2})", chunk.similarity));
        }
        
        formatted
    }
    
    /// Build a structured context with source references
    pub fn build_with_references(&self, chunks: &[RetrievedChunk]) -> (String, Vec<SourceReference>) {
        let mut context = String::new();
        let mut references = Vec::new();
        let mut current_meeting_id = String::new();
        
        for (index, chunk) in chunks.iter().enumerate() {
            let ref_id = index + 1;
            
            // Meeting header on change
            if chunk.meeting_id != current_meeting_id {
                if !current_meeting_id.is_empty() {
                    context.push_str("\n\n");
                }
                let date = format_timestamp(chunk.meeting_date);
                context.push_str(&format!("From \"{}\" ({}):\n", chunk.meeting_title, date));
                current_meeting_id = chunk.meeting_id.clone();
            }
            
            // Add chunk with reference marker
            let text = &chunk.text;
            context.push_str(&format!("[{}] {}\n", ref_id, text));
            
            // Track reference
            references.push(SourceReference {
                id: ref_id,
                meeting_id: chunk.meeting_id.clone(),
                meeting_title: chunk.meeting_title.clone(),
                chunk_type: chunk.chunk_type.clone(),
                text: chunk.text.clone(),
                start_ms: chunk.start_ms,
            });
        }
        
        (context, references)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceReference {
    pub id: usize,
    pub meeting_id: String,
    pub meeting_title: String,
    pub chunk_type: String,
    pub text: String,
    pub start_ms: Option<i64>,
}

fn format_timestamp(epoch_ms: i64) -> String {
    let dt = Utc.timestamp_millis_opt(epoch_ms).unwrap();
    dt.format("%B %d, %Y").to_string()
}

fn format_duration_ms(ms: i64) -> String {
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes % 60, seconds % 60)
    } else {
        format!("{:02}:{:02}", minutes, seconds % 60)
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_context_builder() {
        let chunks = vec![
            RetrievedChunk {
                id: "1".into(),
                meeting_id: "m1".into(),
                meeting_title: "Sprint Planning".into(),
                meeting_date: 1704067200000, // Jan 1, 2024
                chunk_type: "transcript".into(),
                text: "Let's discuss the API timeline.".into(),
                start_ms: Some(60000),
                end_ms: Some(65000),
                speaker: Some("you".into()),
                similarity: 0.85,
            },
            RetrievedChunk {
                id: "2".into(),
                meeting_id: "m1".into(),
                meeting_title: "Sprint Planning".into(),
                meeting_date: 1704067200000,
                chunk_type: "transcript".into(),
                text: "I think we should aim for two weeks.".into(),
                start_ms: Some(66000),
                end_ms: Some(70000),
                speaker: Some("others".into()),
                similarity: 0.78,
            },
        ];
        
        let builder = ContextBuilder::new();
        let context = builder.build(&chunks);
        
        assert!(context.contains("Sprint Planning"));
        assert!(context.contains("[01:00]"));
        assert!(context.contains("You:"));
        assert!(context.contains("API timeline"));
    }
}
```

### Step 3: RAG Prompts

**File: `src-tauri/src/rag/prompts.rs`**

```rust
use crate::rag::context::SourceReference;

/// System prompt for RAG chat
pub fn rag_system_prompt() -> &'static str {
    r#"You are a helpful assistant that answers questions about the user's meetings.

Your knowledge comes from the meeting transcripts, notes, and summaries provided in the context.

Guidelines:
1. Answer based ONLY on the provided meeting context
2. If the information isn't in the context, say "I don't have information about that in your meetings"
3. Reference specific meetings and timestamps when possible
4. Be concise but thorough
5. If asked about decisions, try to include who made them and when
6. For action items, include any deadlines or assignees mentioned

Do not make up information that isn't in the meeting context."#
}

/// Build the complete RAG prompt
pub fn build_rag_prompt(
    context: &str,
    query: &str,
    chat_history: &[(String, String)], // (role, content)
) -> String {
    let mut prompt = String::new();
    
    // System instruction
    prompt.push_str(rag_system_prompt());
    prompt.push_str("\n\n");
    
    // Meeting context
    prompt.push_str("## Meeting Context\n\n");
    prompt.push_str(context);
    prompt.push_str("\n\n");
    
    // Chat history (last few turns for context)
    if !chat_history.is_empty() {
        prompt.push_str("## Previous Conversation\n\n");
        for (role, content) in chat_history.iter().take(6) {
            let prefix = if role == "user" { "User" } else { "Assistant" };
            prompt.push_str(&format!("{}: {}\n", prefix, content));
        }
        prompt.push_str("\n");
    }
    
    // Current query
    prompt.push_str("## Current Question\n\n");
    prompt.push_str(&format!("User: {}\n\n", query));
    prompt.push_str("Assistant:");
    
    prompt
}

/// Build a prompt for follow-up questions (uses more history)
pub fn build_followup_prompt(
    context: &str,
    query: &str,
    chat_history: &[(String, String)],
) -> String {
    let mut prompt = String::new();
    
    prompt.push_str(rag_system_prompt());
    prompt.push_str("\n\n");
    
    prompt.push_str("## Meeting Context\n\n");
    prompt.push_str(context);
    prompt.push_str("\n\n");
    
    // Include more history for follow-ups
    if !chat_history.is_empty() {
        prompt.push_str("## Conversation\n\n");
        for (role, content) in chat_history.iter() {
            let prefix = if role == "user" { "User" } else { "Assistant" };
            prompt.push_str(&format!("{}: {}\n\n", prefix, content));
        }
    }
    
    prompt.push_str(&format!("User: {}\n\n", query));
    prompt.push_str("Assistant:");
    
    prompt
}

/// Prompt for generating suggested follow-up questions
pub fn suggested_questions_prompt(context: &str, last_response: &str) -> String {
    format!(
        r#"Based on the following meeting context and conversation, suggest 3 short follow-up questions the user might want to ask.

Meeting Context:
{context}

Last Assistant Response:
{last_response}

Generate exactly 3 questions, one per line, without numbering or bullets:"#
    )
}

/// Prompt for meeting-specific questions (when chatting within a meeting view)
pub fn meeting_scoped_prompt(meeting_title: &str) -> String {
    format!(
        r#"You are a helpful assistant answering questions specifically about the meeting "{meeting_title}".

Guidelines:
1. Focus only on content from this specific meeting
2. Reference timestamps when relevant
3. Be specific about who said what
4. If asked about something not discussed in this meeting, say so

Meeting Transcript and Notes:"#
    )
}

/// Generate a query expansion for better retrieval
pub fn expand_query(query: &str) -> String {
    // Simple query expansion - could be enhanced with LLM in future
    let mut expanded = query.to_string();
    
    // Add common synonyms for meeting-related terms
    let expansions = [
        ("decide", "decision decided agreed"),
        ("action", "action item task todo"),
        ("timeline", "timeline schedule deadline date"),
        ("discuss", "discuss talk mention say"),
        ("agree", "agree decided consensus"),
    ];
    
    for (term, expansion) in expansions {
        if query.to_lowercase().contains(term) {
            expanded.push_str(&format!(" {}", expansion));
        }
    }
    
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rag_prompt_building() {
        let context = "Meeting content here";
        let query = "What was decided?";
        let history = vec![
            ("user".to_string(), "Hello".to_string()),
            ("assistant".to_string(), "Hi! How can I help?".to_string()),
        ];
        
        let prompt = build_rag_prompt(context, query, &history);
        
        assert!(prompt.contains("Meeting content here"));
        assert!(prompt.contains("What was decided?"));
        assert!(prompt.contains("Hello"));
        assert!(prompt.contains("Assistant:"));
    }
}
```

### Step 4: RAG Generator

**File: `src-tauri/src/rag/generator.rs`**

```rust
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::inference::llm::{LLMService, GenerationConfig};
use crate::rag::context::SourceReference;
use crate::rag::prompts::{build_rag_prompt, build_followup_prompt};

/// Response from RAG generation
#[derive(Debug, Clone)]
pub struct RagResponse {
    /// The generated text response
    pub text: String,
    /// Sources used to generate the response
    pub sources: Vec<SourceReference>,
    /// Whether the response was truncated
    pub truncated: bool,
}

/// Streaming token with metadata
#[derive(Debug, Clone)]
pub enum StreamToken {
    /// A token of generated text
    Token(String),
    /// Generation complete with final response
    Done(RagResponse),
    /// An error occurred
    Error(String),
}

/// RAG Generator handles LLM response generation
pub struct RagGenerator {
    llm_service: Arc<LLMService>,
}

impl RagGenerator {
    pub fn new(llm_service: Arc<LLMService>) -> Self {
        Self { llm_service }
    }
    
    /// Generate a response given context and query (non-streaming)
    pub async fn generate(
        &self,
        context: &str,
        sources: Vec<SourceReference>,
        query: &str,
        chat_history: &[(String, String)],
    ) -> Result<RagResponse> {
        let prompt = if chat_history.is_empty() {
            build_rag_prompt(context, query, &[])
        } else {
            build_followup_prompt(context, query, chat_history)
        };
        
        let config = GenerationConfig {
            max_tokens: 1024,
            temperature: 0.7,
            top_p: 0.9,
            stop_sequences: vec![
                "User:".to_string(),
                "\n## ".to_string(),
            ],
            ..Default::default()
        };
        
        let response = self.llm_service
            .generate(&prompt, config)
            .await?;
        
        Ok(RagResponse {
            text: response.text.trim().to_string(),
            sources,
            truncated: response.truncated,
        })
    }
    
    /// Generate a streaming response
    pub async fn generate_stream(
        &self,
        context: &str,
        sources: Vec<SourceReference>,
        query: &str,
        chat_history: &[(String, String)],
    ) -> Result<mpsc::Receiver<StreamToken>> {
        let prompt = if chat_history.is_empty() {
            build_rag_prompt(context, query, &[])
        } else {
            build_followup_prompt(context, query, chat_history)
        };
        
        let config = GenerationConfig {
            max_tokens: 1024,
            temperature: 0.7,
            top_p: 0.9,
            stop_sequences: vec![
                "User:".to_string(),
                "\n## ".to_string(),
            ],
            ..Default::default()
        };
        
        let (tx, rx) = mpsc::channel(100);
        let llm_service = self.llm_service.clone();
        let sources_clone = sources.clone();
        
        tokio::spawn(async move {
            let mut full_response = String::new();
            let mut truncated = false;
            
            match llm_service.generate_stream(&prompt, config).await {
                Ok(mut stream_rx) => {
                    while let Some(token) = stream_rx.recv().await {
                        match token {
                            crate::inference::llm::StreamEvent::Token(t) => {
                                full_response.push_str(&t);
                                let _ = tx.send(StreamToken::Token(t)).await;
                            }
                            crate::inference::llm::StreamEvent::Done { truncated: t } => {
                                truncated = t;
                            }
                            crate::inference::llm::StreamEvent::Error(e) => {
                                let _ = tx.send(StreamToken::Error(e)).await;
                                return;
                            }
                        }
                    }
                    
                    let _ = tx.send(StreamToken::Done(RagResponse {
                        text: full_response.trim().to_string(),
                        sources: sources_clone,
                        truncated,
                    })).await;
                }
                Err(e) => {
                    let _ = tx.send(StreamToken::Error(e.to_string())).await;
                }
            }
        });
        
        Ok(rx)
    }
    
    /// Generate suggested follow-up questions
    pub async fn generate_suggestions(
        &self,
        context: &str,
        last_response: &str,
    ) -> Result<Vec<String>> {
        let prompt = crate::rag::prompts::suggested_questions_prompt(context, last_response);
        
        let config = GenerationConfig {
            max_tokens: 150,
            temperature: 0.8,
            ..Default::default()
        };
        
        let response = self.llm_service
            .generate(&prompt, config)
            .await?;
        
        let questions: Vec<String> = response.text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && l.len() < 100)
            .take(3)
            .map(|s| s.to_string())
            .collect();
        
        Ok(questions)
    }
}
```

### Step 5: Complete RAG Pipeline

**File: `src-tauri/src/rag/pipeline.rs`**

```rust
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, instrument};

use crate::inference::embedding::EmbeddingService;
use crate::inference::llm::LLMService;
use crate::storage::vectors::VectorStore;
use crate::storage::sqlite::Database;

use super::retriever::{Retriever, RetrieverConfig, RetrievedChunk};
use super::context::ContextBuilder;
use super::generator::{RagGenerator, RagResponse, StreamToken};
use super::prompts::expand_query;

/// Configuration for the RAG pipeline
#[derive(Debug, Clone)]
pub struct RagConfig {
    /// Retriever configuration
    pub retriever: RetrieverConfig,
    /// Whether to use hybrid search (vector + FTS)
    pub use_hybrid_search: bool,
    /// Whether to expand queries for better retrieval
    pub expand_queries: bool,
    /// Whether to generate follow-up suggestions
    pub generate_suggestions: bool,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            retriever: RetrieverConfig::default(),
            use_hybrid_search: true,
            expand_queries: true,
            generate_suggestions: true,
        }
    }
}

/// Complete response from the RAG pipeline
#[derive(Debug, Clone, serde::Serialize)]
pub struct RagPipelineResponse {
    /// Generated answer
    pub answer: String,
    /// Source references
    pub sources: Vec<SourceInfo>,
    /// Suggested follow-up questions
    pub suggestions: Vec<String>,
    /// Whether response was truncated
    pub truncated: bool,
}

/// Simplified source info for frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceInfo {
    pub meeting_id: String,
    pub meeting_title: String,
    pub chunk_type: String,
    pub text: String,
    pub start_ms: Option<i64>,
    pub similarity: f32,
}

impl From<&RetrievedChunk> for SourceInfo {
    fn from(chunk: &RetrievedChunk) -> Self {
        Self {
            meeting_id: chunk.meeting_id.clone(),
            meeting_title: chunk.meeting_title.clone(),
            chunk_type: chunk.chunk_type.clone(),
            text: if chunk.text.len() > 200 {
                format!("{}...", &chunk.text[..200])
            } else {
                chunk.text.clone()
            },
            start_ms: chunk.start_ms,
            similarity: chunk.similarity,
        }
    }
}

/// The main RAG pipeline
pub struct RagPipeline {
    retriever: Retriever,
    generator: RagGenerator,
    context_builder: ContextBuilder,
    config: RagConfig,
}

impl RagPipeline {
    pub fn new(
        embedding_service: Arc<EmbeddingService>,
        llm_service: Arc<LLMService>,
        vector_store: Arc<VectorStore>,
        database: Arc<Database>,
    ) -> Self {
        Self {
            retriever: Retriever::new(
                embedding_service,
                vector_store,
                database,
            ),
            generator: RagGenerator::new(llm_service),
            context_builder: ContextBuilder::new(),
            config: RagConfig::default(),
        }
    }
    
    pub fn with_config(mut self, config: RagConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Process a query through the complete RAG pipeline
    #[instrument(skip(self, chat_history))]
    pub async fn query(
        &self,
        query: &str,
        chat_history: &[(String, String)],
    ) -> Result<RagPipelineResponse> {
        info!("Processing RAG query: {}", query);
        
        // 1. Optionally expand query
        let search_query = if self.config.expand_queries {
            expand_query(query)
        } else {
            query.to_string()
        };
        
        // 2. Retrieve relevant chunks
        let chunks = if self.config.use_hybrid_search {
            self.retriever
                .hybrid_retrieve(&search_query, &self.config.retriever)
                .await?
        } else {
            self.retriever
                .retrieve(&search_query, &self.config.retriever)
                .await?
        };
        
        info!("Retrieved {} chunks", chunks.len());
        
        // 3. Build context
        let (context, references) = self.context_builder
            .build_with_references(&chunks);
        
        // 4. Generate response
        let response = self.generator
            .generate(
                &context,
                references,
                query,
                chat_history,
            )
            .await?;
        
        // 5. Optionally generate suggestions
        let suggestions = if self.config.generate_suggestions && chunks.len() > 0 {
            self.generator
                .generate_suggestions(&context, &response.text)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        
        // 6. Build final response
        Ok(RagPipelineResponse {
            answer: response.text,
            sources: chunks.iter().map(SourceInfo::from).collect(),
            suggestions,
            truncated: response.truncated,
        })
    }
    
    /// Process a query with streaming response
    pub async fn query_stream(
        &self,
        query: &str,
        chat_history: &[(String, String)],
    ) -> Result<(mpsc::Receiver<StreamToken>, Vec<SourceInfo>)> {
        // 1. Retrieve
        let search_query = if self.config.expand_queries {
            expand_query(query)
        } else {
            query.to_string()
        };
        
        let chunks = if self.config.use_hybrid_search {
            self.retriever
                .hybrid_retrieve(&search_query, &self.config.retriever)
                .await?
        } else {
            self.retriever
                .retrieve(&search_query, &self.config.retriever)
                .await?
        };
        
        let sources: Vec<SourceInfo> = chunks.iter().map(SourceInfo::from).collect();
        
        // 2. Build context
        let (context, references) = self.context_builder
            .build_with_references(&chunks);
        
        // 3. Start streaming generation
        let stream = self.generator
            .generate_stream(
                &context,
                references,
                query,
                chat_history,
            )
            .await?;
        
        Ok((stream, sources))
    }
    
    /// Query within a specific meeting context
    pub async fn query_meeting(
        &self,
        meeting_id: &str,
        query: &str,
    ) -> Result<RagPipelineResponse> {
        let chunks = self.retriever
            .retrieve_from_meeting(query, meeting_id, 10)
            .await?;
        
        let (context, references) = self.context_builder
            .build_with_references(&chunks);
        
        let response = self.generator
            .generate(&context, references, query, &[])
            .await?;
        
        Ok(RagPipelineResponse {
            answer: response.text,
            sources: chunks.iter().map(SourceInfo::from).collect(),
            suggestions: Vec::new(),
            truncated: response.truncated,
        })
    }
    
    /// Get just the relevant sources without generating a response
    pub async fn find_sources(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<SourceInfo>> {
        let config = RetrieverConfig {
            top_k,
            ..self.config.retriever.clone()
        };
        
        let chunks = self.retriever
            .retrieve(query, &config)
            .await?;
        
        Ok(chunks.iter().map(SourceInfo::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Integration tests would require mocked services
    // See tests/rag_integration.rs for full tests
}
```

### Step 6: Module Exports

**File: `src-tauri/src/rag/mod.rs`**

```rust
//! RAG (Retrieval-Augmented Generation) module for meeting chat
//! 
//! This module provides the complete pipeline for:
//! - Retrieving relevant meeting content via vector search
//! - Building context from multiple sources
//! - Generating responses with LLM
//! - Tracking and citing sources

mod retriever;
mod context;
mod generator;
mod pipeline;
mod prompts;

pub use retriever::{Retriever, RetrieverConfig, RetrievedChunk};
pub use context::{ContextBuilder, SourceReference};
pub use generator::{RagGenerator, RagResponse, StreamToken};
pub use pipeline::{RagPipeline, RagConfig, RagPipelineResponse, SourceInfo};
pub use prompts::{rag_system_prompt, build_rag_prompt};
```

### Step 7: Tauri Chat Commands

**File: `src-tauri/src/commands/chat.rs`**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::rag::{RagPipeline, RagPipelineResponse, SourceInfo, StreamToken};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub answer: String,
    pub sources: Vec<SourceInfo>,
    pub suggestions: Vec<String>,
}

/// Send a message and get a response (non-streaming)
#[tauri::command]
pub async fn chat_with_meetings(
    state: State<'_, AppState>,
    message: String,
    history: Vec<ChatMessage>,
) -> Result<ChatResponse, String> {
    let pipeline = state.rag_pipeline.lock().await;
    let pipeline = pipeline.as_ref()
        .ok_or("RAG pipeline not initialized")?;
    
    // Convert history to tuple format
    let history_tuples: Vec<(String, String)> = history
        .into_iter()
        .map(|m| (m.role, m.content))
        .collect();
    
    let response = pipeline
        .query(&message, &history_tuples)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(ChatResponse {
        answer: response.answer,
        sources: response.sources,
        suggestions: response.suggestions,
    })
}

/// Start a streaming chat response
#[tauri::command]
pub async fn stream_chat_response(
    app: AppHandle,
    state: State<'_, AppState>,
    message: String,
    history: Vec<ChatMessage>,
) -> Result<String, String> {
    let pipeline = state.rag_pipeline.lock().await;
    let pipeline = pipeline.as_ref()
        .ok_or("RAG pipeline not initialized")?;
    
    let history_tuples: Vec<(String, String)> = history
        .into_iter()
        .map(|m| (m.role, m.content))
        .collect();
    
    // Generate unique stream ID
    let stream_id = Uuid::new_v4().to_string();
    let stream_id_clone = stream_id.clone();
    
    let (mut rx, sources) = pipeline
        .query_stream(&message, &history_tuples)
        .await
        .map_err(|e| e.to_string())?;
    
    // Spawn task to forward tokens to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(token) = rx.recv().await {
            match token {
                StreamToken::Token(t) => {
                    let _ = app_clone.emit(
                        &format!("chat-token-{}", stream_id_clone),
                        t,
                    );
                }
                StreamToken::Done(response) => {
                    let _ = app_clone.emit(
                        &format!("chat-complete-{}", stream_id_clone),
                        response.sources,
                    );
                    break;
                }
                StreamToken::Error(e) => {
                    let _ = app_clone.emit(
                        &format!("chat-error-{}", stream_id_clone),
                        e,
                    );
                    break;
                }
            }
        }
    });
    
    Ok(stream_id)
}

/// Chat within a specific meeting context
#[tauri::command]
pub async fn chat_about_meeting(
    state: State<'_, AppState>,
    meeting_id: String,
    message: String,
) -> Result<ChatResponse, String> {
    let pipeline = state.rag_pipeline.lock().await;
    let pipeline = pipeline.as_ref()
        .ok_or("RAG pipeline not initialized")?;
    
    let response = pipeline
        .query_meeting(&meeting_id, &message)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(ChatResponse {
        answer: response.answer,
        sources: response.sources,
        suggestions: Vec::new(),
    })
}

/// Semantic search across meetings
#[tauri::command]
pub async fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SourceInfo>, String> {
    let pipeline = state.rag_pipeline.lock().await;
    let pipeline = pipeline.as_ref()
        .ok_or("RAG pipeline not initialized")?;
    
    pipeline
        .find_sources(&query, limit.unwrap_or(10))
        .await
        .map_err(|e| e.to_string())
}

/// Get suggested questions based on recent meetings
#[tauri::command]
pub async fn get_chat_suggestions(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = &state.database;
    
    // Get recent meetings
    let meetings = db.list_meetings(Some(3), None, None)
        .await
        .map_err(|e| e.to_string())?;
    
    if meetings.is_empty() {
        return Ok(vec![
            "What meetings have I recorded?".to_string(),
        ]);
    }
    
    // Generate suggestions based on meeting titles
    let mut suggestions = vec![
        "What were the key decisions from my recent meetings?".to_string(),
        "Are there any action items I need to follow up on?".to_string(),
    ];
    
    if let Some(meeting) = meetings.first() {
        suggestions.push(format!(
            "What was discussed in \"{}\"?",
            meeting.title
        ));
    }
    
    Ok(suggestions)
}
```

### Step 8: Frontend Chat Components

**File: `src/components/chat/ChatView.tsx`**

```typescript
import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Send, Trash2, Sparkles } from 'lucide-react';
import { ChatMessage } from './ChatMessage';
import { ChatInput } from './ChatInput';
import { ChatSuggestions } from './ChatSuggestions';
import { SourceCard } from './SourceCard';
import { useChat } from '../../hooks/useChat';
import { Button } from '../ui/Button';

export function ChatView() {
  const { messages, isLoading, sendMessage, clearChat } = useChat();
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  
  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);
  
  // Load initial suggestions
  useEffect(() => {
    // Fetch suggestions from backend
    import('../../lib/tauri').then(({ invoke }) => {
      invoke('get_chat_suggestions').then((s: string[]) => {
        setSuggestions(s);
      });
    });
  }, []);
  
  const handleSend = (content: string) => {
    sendMessage(content);
    setSuggestions([]); // Clear suggestions after first message
  };
  
  const handleSuggestionClick = (suggestion: string) => {
    handleSend(suggestion);
  };
  
  const isEmpty = messages.length === 0;
  
  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 bg-white border-b border-surface-200 flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold text-surface-900">Chat with Your Meetings</h1>
          <p className="text-sm text-surface-500">Ask questions about your recorded meetings</p>
        </div>
        
        {messages.length > 0 && (
          <Button
            variant="ghost"
            size="sm"
            onClick={clearChat}
            leftIcon={<Trash2 className="w-4 h-4" />}
          >
            Clear
          </Button>
        )}
      </div>
      
      {/* Messages area */}
      <div className="flex-1 overflow-y-auto p-4">
        {isEmpty ? (
          <div className="h-full flex flex-col items-center justify-center text-center px-8">
            <div className="w-16 h-16 bg-primary-100 rounded-full flex items-center justify-center mb-4">
              <Sparkles className="w-8 h-8 text-primary-600" />
            </div>
            <h2 className="text-xl font-semibold text-surface-900 mb-2">
              Ask anything about your meetings
            </h2>
            <p className="text-surface-500 mb-6 max-w-md">
              I can help you find decisions, action items, or any information from your recorded meetings.
            </p>
            
            {/* Suggestions */}
            <ChatSuggestions
              suggestions={suggestions}
              onSelect={handleSuggestionClick}
            />
          </div>
        ) : (
          <div className="space-y-4 max-w-3xl mx-auto">
            <AnimatePresence>
              {messages.map((message) => (
                <motion.div
                  key={message.id}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                >
                  <ChatMessage message={message} />
                  
                  {/* Sources for assistant messages */}
                  {message.role === 'assistant' && message.sources && message.sources.length > 0 && (
                    <div className="ml-12 mt-2 space-y-2">
                      <span className="text-xs text-surface-500 font-medium">Sources:</span>
                      {message.sources.map((source, index) => (
                        <SourceCard key={index} source={source} />
                      ))}
                    </div>
                  )}
                </motion.div>
              ))}
            </AnimatePresence>
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>
      
      {/* Input area */}
      <div className="p-4 bg-white border-t border-surface-200">
        <div className="max-w-3xl mx-auto">
          <ChatInput
            onSend={handleSend}
            isLoading={isLoading}
            placeholder="Ask about your meetings..."
          />
        </div>
      </div>
    </div>
  );
}
```

**File: `src/components/chat/ChatMessage.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { User, Bot } from 'lucide-react';
import { cn } from '../../lib/utils';
import type { ChatMessage as ChatMessageType } from '../../types/chat';

interface ChatMessageProps {
  message: ChatMessageType;
}

export function ChatMessage({ message }: ChatMessageProps) {
  const isUser = message.role === 'user';
  
  return (
    <div className={cn('flex gap-3', isUser && 'flex-row-reverse')}>
      {/* Avatar */}
      <div className={cn(
        'w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0',
        isUser ? 'bg-primary-100' : 'bg-surface-100'
      )}>
        {isUser ? (
          <User className="w-4 h-4 text-primary-600" />
        ) : (
          <Bot className="w-4 h-4 text-surface-600" />
        )}
      </div>
      
      {/* Message bubble */}
      <div className={cn(
        'flex-1 max-w-[80%] rounded-2xl px-4 py-3',
        isUser 
          ? 'bg-primary-600 text-white rounded-tr-sm' 
          : 'bg-white border border-surface-200 rounded-tl-sm'
      )}>
        <p className={cn(
          'text-sm whitespace-pre-wrap',
          isUser ? 'text-white' : 'text-surface-900'
        )}>
          {message.content}
          {message.isStreaming && (
            <motion.span
              className="inline-block w-2 h-4 ml-1 bg-current"
              animate={{ opacity: [1, 0] }}
              transition={{ repeat: Infinity, duration: 0.8 }}
            />
          )}
        </p>
      </div>
    </div>
  );
}
```

**File: `src/components/chat/ChatInput.tsx`**

```typescript
import React, { useState, useRef, useEffect } from 'react';
import { Send, Loader2 } from 'lucide-react';

interface ChatInputProps {
  onSend: (content: string) => void;
  isLoading?: boolean;
  placeholder?: string;
}

export function ChatInput({ onSend, isLoading, placeholder }: ChatInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  
  // Auto-resize textarea
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 150)}px`;
    }
  }, [value]);
  
  const handleSubmit = () => {
    const trimmed = value.trim();
    if (trimmed && !isLoading) {
      onSend(trimmed);
      setValue('');
    }
  };
  
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };
  
  return (
    <div className="relative flex items-end gap-2 bg-surface-100 rounded-2xl p-2">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        rows={1}
        className="flex-1 bg-transparent resize-none px-3 py-2 text-sm focus:outline-none"
        disabled={isLoading}
      />
      
      <button
        onClick={handleSubmit}
        disabled={!value.trim() || isLoading}
        className="w-10 h-10 flex items-center justify-center rounded-xl bg-primary-600 text-white disabled:opacity-50 disabled:cursor-not-allowed hover:bg-primary-700 transition-colors"
      >
        {isLoading ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <Send className="w-5 h-5" />
        )}
      </button>
    </div>
  );
}
```

**File: `src/components/chat/SourceCard.tsx`**

```typescript
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { FileText, Clock, ExternalLink } from 'lucide-react';
import { Card } from '../ui/Card';
import type { ChatSource } from '../../types/chat';
import { formatTimestamp } from '../../lib/formatters';

interface SourceCardProps {
  source: ChatSource;
}

export function SourceCard({ source }: SourceCardProps) {
  const navigate = useNavigate();
  
  const handleClick = () => {
    // Navigate to meeting with optional timestamp
    const url = source.start_ms 
      ? `/meeting/${source.meeting_id}?t=${source.start_ms}`
      : `/meeting/${source.meeting_id}`;
    navigate(url);
  };
  
  const typeIcon = source.chunk_type === 'transcript' ? Clock : FileText;
  const TypeIcon = typeIcon;
  
  return (
    <Card 
      hoverable 
      onClick={handleClick}
      className="p-3"
    >
      <div className="flex items-start gap-3">
        <div className="w-8 h-8 rounded-lg bg-surface-100 flex items-center justify-center flex-shrink-0">
          <TypeIcon className="w-4 h-4 text-surface-500" />
        </div>
        
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-surface-900 truncate">
              {source.meeting_title}
            </span>
            {source.start_ms && (
              <span className="text-xs text-surface-500">
                @ {formatTimestamp(source.start_ms)}
              </span>
            )}
          </div>
          
          <p className="text-xs text-surface-500 line-clamp-2 mt-1">
            {source.text}
          </p>
        </div>
        
        <ExternalLink className="w-4 h-4 text-surface-400 flex-shrink-0" />
      </div>
    </Card>
  );
}
```

**File: `src/components/chat/ChatSuggestions.tsx`**

```typescript
import React from 'react';
import { motion } from 'framer-motion';
import { MessageSquare } from 'lucide-react';

interface ChatSuggestionsProps {
  suggestions: string[];
  onSelect: (suggestion: string) => void;
}

export function ChatSuggestions({ suggestions, onSelect }: ChatSuggestionsProps) {
  if (suggestions.length === 0) return null;
  
  return (
    <div className="flex flex-wrap gap-2 justify-center">
      {suggestions.map((suggestion, index) => (
        <motion.button
          key={index}
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: index * 0.1 }}
          onClick={() => onSelect(suggestion)}
          className="flex items-center gap-2 px-4 py-2 bg-white border border-surface-200 rounded-full text-sm text-surface-700 hover:bg-surface-50 hover:border-surface-300 transition-colors"
        >
          <MessageSquare className="w-4 h-4 text-surface-400" />
          {suggestion}
        </motion.button>
      ))}
    </div>
  );
}
```

## Advanced Features

### Re-ranking with Cross-Encoder

For improved retrieval quality, implement a cross-encoder re-ranking step:

```rust
// Future enhancement: Cross-encoder reranking
pub struct CrossEncoderReranker {
    model: CrossEncoderModel,
}

impl CrossEncoderReranker {
    pub async fn rerank(
        &self,
        query: &str,
        candidates: Vec<RetrievedChunk>,
        top_k: usize,
    ) -> Vec<RetrievedChunk> {
        // Score each candidate with cross-encoder
        let mut scored: Vec<(f32, RetrievedChunk)> = Vec::new();
        
        for chunk in candidates {
            let score = self.model.score(query, &chunk.text).await;
            scored.push((score, chunk));
        }
        
        // Sort by score and take top_k
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.into_iter().take(top_k).map(|(_, c)| c).collect()
    }
}
```

### Multi-hop Retrieval

For complex questions requiring multiple retrieval steps:

```rust
pub async fn multi_hop_retrieve(
    &self,
    query: &str,
) -> Result<Vec<RetrievedChunk>> {
    // First hop: Get initial relevant chunks
    let initial_chunks = self.retrieve(query, &RetrieverConfig::default()).await?;
    
    // Extract key entities/concepts for second hop
    let entities = self.extract_entities(&initial_chunks);
    
    // Second hop: Get chunks related to extracted entities
    let mut all_chunks = initial_chunks;
    for entity in entities {
        let related = self.retrieve(&entity, &RetrieverConfig {
            top_k: 2,
            ..Default::default()
        }).await?;
        all_chunks.extend(related);
    }
    
    // Deduplicate and re-rank
    self.deduplicate_and_rank(all_chunks)
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_context_building() {
        let chunks = vec![
            RetrievedChunk {
                id: "1".into(),
                meeting_id: "m1".into(),
                meeting_title: "Test Meeting".into(),
                meeting_date: 1704067200000,
                chunk_type: "transcript".into(),
                text: "We discussed the timeline.".into(),
                start_ms: Some(60000),
                end_ms: Some(65000),
                speaker: Some("you".into()),
                similarity: 0.9,
            },
        ];
        
        let builder = ContextBuilder::new();
        let context = builder.build(&chunks);
        
        assert!(context.contains("Test Meeting"));
        assert!(context.contains("We discussed"));
    }
    
    #[tokio::test]
    async fn test_prompt_generation() {
        let context = "Meeting content";
        let query = "What was decided?";
        let history = vec![];
        
        let prompt = build_rag_prompt(context, query, &history);
        
        assert!(prompt.contains("Meeting content"));
        assert!(prompt.contains("What was decided?"));
        assert!(prompt.contains("Assistant:"));
    }
}
```

### Integration Tests

```rust
// tests/rag_integration.rs
#[tokio::test]
async fn test_full_rag_pipeline() {
    // Setup test database and services
    let (db, vector_store, embedding_service, llm_service) = setup_test_services().await;
    
    // Insert test meeting data
    insert_test_meeting(&db).await;
    
    // Create pipeline
    let pipeline = RagPipeline::new(
        embedding_service,
        llm_service,
        vector_store,
        db,
    );
    
    // Test query
    let response = pipeline
        .query("What was discussed?", &[])
        .await
        .unwrap();
    
    assert!(!response.answer.is_empty());
    assert!(!response.sources.is_empty());
}
```

## Performance Optimization

### Caching

```rust
use lru::LruCache;

pub struct CachedRetriever {
    retriever: Retriever,
    cache: Mutex<LruCache<String, Vec<RetrievedChunk>>>,
}

impl CachedRetriever {
    pub async fn retrieve(
        &self,
        query: &str,
        config: &RetrieverConfig,
    ) -> Result<Vec<RetrievedChunk>> {
        let cache_key = format!("{}:{:?}", query, config);
        
        // Check cache
        if let Some(cached) = self.cache.lock().unwrap().get(&cache_key) {
            return Ok(cached.clone());
        }
        
        // Retrieve and cache
        let results = self.retriever.retrieve(query, config).await?;
        self.cache.lock().unwrap().put(cache_key, results.clone());
        
        Ok(results)
    }
}
```

### Batch Embedding

```rust
// Process multiple queries in batch
pub async fn batch_retrieve(
    &self,
    queries: &[String],
) -> Result<Vec<Vec<RetrievedChunk>>> {
    // Embed all queries at once
    let embeddings = self.embedding_service
        .embed_batch(queries)
        .await?;
    
    // Parallel search
    let futures: Vec<_> = embeddings.iter()
        .map(|emb| self.vector_store.search(emb, 5, None, None))
        .collect();
    
    futures::future::try_join_all(futures).await
}
```

## Acceptance Criteria

- [ ] Vector search returns relevant chunks with similarity scores
- [ ] Context builder formats chunks appropriately for LLM
- [ ] RAG pipeline generates coherent answers grounded in meeting content
- [ ] Streaming responses work with token-by-token display
- [ ] Source citations link to correct meeting/timestamp
- [ ] Chat history is maintained for follow-up questions
- [ ] Hybrid search combines vector and FTS results
- [ ] Meeting-scoped chat works correctly
- [ ] Suggested questions are contextually relevant
- [ ] Performance is acceptable (<3s for typical queries)

## Troubleshooting

### Low-Quality Responses

1. **Check retrieval quality**: Log retrieved chunks to verify relevance
2. **Adjust similarity threshold**: Lower `min_similarity` if too few results
3. **Increase top_k**: Retrieve more context
4. **Enable hybrid search**: Combine vector + FTS

### Slow Responses

1. **Profile retrieval**: Vector search should be <100ms
2. **Check LLM generation**: Monitor token generation speed
3. **Reduce context size**: Limit `max_context_chars`
4. **Enable caching**: Cache frequent queries

### Missing Context

1. **Verify embeddings exist**: Check LanceDB has embeddings for meeting
2. **Check chunk types**: Ensure notes/summaries are included
3. **Expand query**: Enable query expansion for broader matching

## References

- [RAG for LLMs (Pinecone)](https://www.pinecone.io/learn/retrieval-augmented-generation/)
- [LangChain RAG](https://python.langchain.com/docs/concepts/rag)
- [LanceDB Search](https://lancedb.github.io/lancedb/search/)
- [Building Production RAG Systems](https://www.anyscale.com/blog/a-comprehensive-guide-for-building-rag-based-llm-applications-part-1)

---

**Next:** [10 - Cross-Platform Support](./10-cross-platform.md) - macOS and Linux audio capture
