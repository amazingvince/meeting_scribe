//! Text chunking for embedding generation
//!
//! Splits transcripts and text into appropriately-sized chunks for embedding.

use crate::inference::transcription::Speaker;

/// Maximum chunk size in characters (roughly 512 tokens)
pub const MAX_CHUNK_CHARS: usize = 2000;

/// Overlap between chunks for context continuity
pub const CHUNK_OVERLAP: usize = 200;

/// Minimum chunk size (don't create tiny fragments)
pub const MIN_CHUNK_CHARS: usize = 100;

/// A chunk of text with metadata for embedding
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// The chunk text content
    pub text: String,
    /// Start time in milliseconds (for transcript chunks)
    pub start_ms: Option<i64>,
    /// End time in milliseconds (for transcript chunks)
    pub end_ms: Option<i64>,
    /// Primary speaker in this chunk
    pub speaker: Option<Speaker>,
    /// Index of this chunk within the source
    pub chunk_index: usize,
}

impl TextChunk {
    /// Create a new text chunk
    pub fn new(text: impl Into<String>, chunk_index: usize) -> Self {
        Self {
            text: text.into(),
            start_ms: None,
            end_ms: None,
            speaker: None,
            chunk_index,
        }
    }

    /// Set the time range
    pub fn with_time_range(mut self, start_ms: i64, end_ms: i64) -> Self {
        self.start_ms = Some(start_ms);
        self.end_ms = Some(end_ms);
        self
    }

    /// Set the speaker
    pub fn with_speaker(mut self, speaker: Speaker) -> Self {
        self.speaker = Some(speaker);
        self
    }
}

/// Input format for transcript segments (accepts different sources)
#[derive(Debug, Clone)]
pub struct TranscriptSegmentInput {
    /// The transcribed text
    pub text: String,
    /// Start time in milliseconds
    pub start_ms: i64,
    /// End time in milliseconds
    pub end_ms: i64,
    /// Speaker identifier
    pub speaker: Option<Speaker>,
}

impl TranscriptSegmentInput {
    /// Create from inference TranscriptSegment
    pub fn from_inference(seg: &crate::inference::TranscriptSegment) -> Self {
        Self {
            text: seg.text.clone(),
            start_ms: seg.start_ms as i64,
            end_ms: seg.end_ms as i64,
            speaker: Some(seg.speaker),
        }
    }

    /// Create from storage StoredSegment
    pub fn from_stored(seg: &crate::storage::StoredSegment) -> Self {
        Self {
            text: seg.text.clone(),
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            speaker: Some(seg.speaker),
        }
    }
}

/// Chunk transcript segments intelligently
///
/// Groups segments together up to `max_chars`, respecting sentence boundaries
/// and maintaining context with overlap.
pub fn chunk_transcript(segments: &[TranscriptSegmentInput], max_chars: usize) -> Vec<TextChunk> {
    let max_chars = if max_chars == 0 {
        MAX_CHUNK_CHARS
    } else {
        max_chars
    };

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut current_start: Option<i64> = None;
    let mut current_end: Option<i64> = None;
    let mut current_speaker: Option<Speaker> = None;
    let mut chunk_index = 0;

    for segment in segments {
        let speaker_label = segment.speaker.map(speaker_to_label).unwrap_or("SPEAKER");

        let segment_text = format!("[{}] {}\n", speaker_label, segment.text);

        // Check if adding this segment would exceed limit
        if !current_chunk.is_empty() && current_chunk.len() + segment_text.len() > max_chars {
            // Save current chunk
            if current_chunk.len() >= MIN_CHUNK_CHARS {
                let mut chunk = TextChunk::new(current_chunk.trim(), chunk_index);
                if let (Some(start), Some(end)) = (current_start, current_end) {
                    chunk = chunk.with_time_range(start, end);
                }
                if let Some(speaker) = current_speaker {
                    chunk = chunk.with_speaker(speaker);
                }
                chunks.push(chunk);
                chunk_index += 1;
            }

            // Start new chunk with overlap
            let overlap_text = get_overlap_text(&current_chunk, CHUNK_OVERLAP);
            current_chunk = overlap_text;
            current_start = Some(segment.start_ms);
            current_speaker = segment.speaker;
        }

        if current_start.is_none() {
            current_start = Some(segment.start_ms);
        }
        current_end = Some(segment.end_ms);

        // Track first speaker in chunk
        if current_speaker.is_none() {
            current_speaker = segment.speaker;
        }

        current_chunk.push_str(&segment_text);
    }

    // Don't forget the last chunk
    let trimmed = current_chunk.trim();
    if !trimmed.is_empty() && (trimmed.len() >= MIN_CHUNK_CHARS || chunks.is_empty()) {
        let mut chunk = TextChunk::new(trimmed, chunk_index);
        if let (Some(start), Some(end)) = (current_start, current_end) {
            chunk = chunk.with_time_range(start, end);
        }
        if let Some(speaker) = current_speaker {
            chunk = chunk.with_speaker(speaker);
        }
        chunks.push(chunk);
    }

    chunks
}

/// Chunk plain text (for notes, summaries)
///
/// Splits by paragraphs first, then by sentences if needed.
pub fn chunk_text(text: &str, max_chars: usize) -> Vec<TextChunk> {
    let max_chars = if max_chars == 0 {
        MAX_CHUNK_CHARS
    } else {
        max_chars
    };

    let mut chunks = Vec::new();
    let mut chunk_index = 0;

    // Split by paragraphs first
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut current_chunk = String::new();

    for para in paragraphs {
        if current_chunk.len() + para.len() + 2 > max_chars && !current_chunk.is_empty() {
            if current_chunk.len() >= MIN_CHUNK_CHARS {
                chunks.push(TextChunk::new(current_chunk.trim(), chunk_index));
                chunk_index += 1;
            }

            let overlap = get_overlap_text(&current_chunk, CHUNK_OVERLAP);
            current_chunk = overlap;
        }

        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(para);
    }

    if !current_chunk.is_empty() && current_chunk.len() >= MIN_CHUNK_CHARS {
        chunks.push(TextChunk::new(current_chunk.trim(), chunk_index));
    } else if !current_chunk.is_empty() && chunks.is_empty() {
        // If we only have a small amount of text, still create a chunk
        chunks.push(TextChunk::new(current_chunk.trim(), chunk_index));
    }

    // Handle very long paragraphs by further splitting
    chunks = chunks
        .into_iter()
        .flat_map(|chunk| {
            if chunk.text.len() > max_chars * 2 {
                split_long_chunk(&chunk, max_chars)
            } else {
                vec![chunk]
            }
        })
        .collect();

    // Re-index after potential splitting
    for (i, chunk) in chunks.iter_mut().enumerate() {
        chunk.chunk_index = i;
    }

    chunks
}

/// Get overlap text from the end of a string
fn get_overlap_text(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return String::new();
    }

    // Find a good break point (sentence or word boundary)
    let start_pos = text.len() - max_chars;

    // Look for sentence boundary
    let search_text = &text[start_pos..];
    if let Some(pos) = search_text.find(". ") {
        return search_text[pos + 2..].to_string();
    }

    // Look for word boundary
    if let Some(pos) = search_text.find(' ') {
        return search_text[pos + 1..].to_string();
    }

    search_text.to_string()
}

/// Split a very long chunk into smaller pieces
fn split_long_chunk(chunk: &TextChunk, max_chars: usize) -> Vec<TextChunk> {
    let mut result = Vec::new();
    let text = &chunk.text;
    let mut start = 0;
    let mut local_index = 0;

    while start < text.len() {
        let end = (start + max_chars).min(text.len());

        // Find a good break point
        let actual_end = if end < text.len() {
            text[start..end]
                .rfind(['.', '!', '?', '\n'])
                .map(|pos| start + pos + 1)
                .unwrap_or(end)
        } else {
            end
        };

        let chunk_text = text[start..actual_end].trim();
        if !chunk_text.is_empty() {
            let mut new_chunk = TextChunk::new(chunk_text, chunk.chunk_index + local_index);
            new_chunk.start_ms = chunk.start_ms;
            new_chunk.end_ms = chunk.end_ms;
            new_chunk.speaker = chunk.speaker;
            result.push(new_chunk);
            local_index += 1;
        }

        start = actual_end;
    }

    result
}

/// Convert Speaker enum to display label
fn speaker_to_label(speaker: Speaker) -> &'static str {
    match speaker {
        Speaker::You => "YOU",
        Speaker::Others => "OTHERS",
        Speaker::Unknown => "SPEAKER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_short_transcript() {
        let segments = vec![
            TranscriptSegmentInput {
                text: "Hello world".to_string(),
                start_ms: 0,
                end_ms: 1000,
                speaker: Some(Speaker::You),
            },
            TranscriptSegmentInput {
                text: "How are you".to_string(),
                start_ms: 1000,
                end_ms: 2000,
                speaker: Some(Speaker::Others),
            },
        ];

        let chunks = chunk_transcript(&segments, MAX_CHUNK_CHARS);

        // Short segments should fit in one chunk
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("Hello world"));
        assert!(chunks[0].text.contains("How are you"));
        assert_eq!(chunks[0].start_ms, Some(0));
        assert_eq!(chunks[0].end_ms, Some(2000));
    }

    #[test]
    fn test_chunk_long_transcript() {
        // Create segments that exceed chunk limit
        let long_text = "a".repeat(1500);
        let segments = vec![
            TranscriptSegmentInput {
                text: long_text.clone(),
                start_ms: 0,
                end_ms: 5000,
                speaker: Some(Speaker::You),
            },
            TranscriptSegmentInput {
                text: long_text.clone(),
                start_ms: 5000,
                end_ms: 10000,
                speaker: Some(Speaker::Others),
            },
        ];

        let chunks = chunk_transcript(&segments, 2000);

        // Should create multiple chunks
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_chunk_text_simple() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";

        let chunks = chunk_text(text, MAX_CHUNK_CHARS);

        // Short text should fit in one chunk
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("First paragraph"));
    }

    #[test]
    fn test_chunk_text_long() {
        // Create text that exceeds chunk limit
        let paragraph = "a".repeat(1000);
        let text = format!("{}\n\n{}\n\n{}", paragraph, paragraph, paragraph);

        let chunks = chunk_text(&text, 1500);

        // Should create multiple chunks
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_get_overlap_text() {
        let text = "First sentence. Second sentence. Third sentence.";

        let overlap = get_overlap_text(text, 20);

        // Should find a sentence boundary
        assert!(!overlap.is_empty());
    }

    #[test]
    fn test_speaker_labels() {
        assert_eq!(speaker_to_label(Speaker::You), "YOU");
        assert_eq!(speaker_to_label(Speaker::Others), "OTHERS");
        assert_eq!(speaker_to_label(Speaker::Unknown), "SPEAKER");
    }

    #[test]
    fn test_text_chunk_builder() {
        let chunk = TextChunk::new("test", 0)
            .with_time_range(1000, 2000)
            .with_speaker(Speaker::You);

        assert_eq!(chunk.text, "test");
        assert_eq!(chunk.start_ms, Some(1000));
        assert_eq!(chunk.end_ms, Some(2000));
        assert_eq!(chunk.speaker, Some(Speaker::You));
        assert_eq!(chunk.chunk_index, 0);
    }

    #[test]
    fn test_empty_input() {
        let chunks = chunk_transcript(&[], MAX_CHUNK_CHARS);
        assert!(chunks.is_empty());

        let chunks = chunk_text("", MAX_CHUNK_CHARS);
        assert!(chunks.is_empty());
    }
}
