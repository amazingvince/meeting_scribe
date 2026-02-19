//! Text chunking for embedding generation
//!
//! Splits transcripts and text into appropriately-sized chunks for embedding.

use crate::inference::transcription::Speaker;

/// Maximum chunk size in characters (roughly 512 tokens)
pub const MAX_CHUNK_CHARS: usize = 2000;

/// Overlap between chunks for context continuity
pub const CHUNK_OVERLAP: usize = 500;

/// Maximum transcript span (ms) represented by a single chunk.
///
/// Keeping chunk timespans bounded avoids over-broad chunks on low-density speech
/// and improves timestamp-specific retrieval for playback jumps.
pub const MAX_CHUNK_DURATION_MS: i64 = 120_000;

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

    if segments.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_segment_indexes = Vec::new();
    let mut current_len = 0usize;
    let mut chunk_index = 0;
    let mut current_start_ms: Option<i64> = None;

    for (idx, segment) in segments.iter().enumerate() {
        let segment_len = transcript_segment_len(segment);
        let would_exceed_chars =
            !current_segment_indexes.is_empty() && current_len + segment_len > max_chars;
        let would_exceed_duration = current_start_ms
            .map(|start| segment.end_ms - start > MAX_CHUNK_DURATION_MS)
            .unwrap_or(false)
            && !current_segment_indexes.is_empty();

        if would_exceed_chars || would_exceed_duration {
            if let Some(chunk) =
                build_transcript_chunk(segments, &current_segment_indexes, chunk_index)
            {
                chunks.push(chunk);
                chunk_index += 1;
            }

            if would_exceed_duration {
                current_segment_indexes.clear();
                current_len = 0;
                current_start_ms = None;
            } else {
                current_segment_indexes =
                    overlap_segment_indexes(segments, &current_segment_indexes, CHUNK_OVERLAP);
                current_len = current_segment_indexes
                    .iter()
                    .map(|segment_idx| transcript_segment_len(&segments[*segment_idx]))
                    .sum();
                current_start_ms = current_segment_indexes
                    .first()
                    .map(|segment_idx| segments[*segment_idx].start_ms);
                if current_len > max_chars {
                    current_segment_indexes.clear();
                    current_len = 0;
                    current_start_ms = None;
                }
            }
        }

        if current_segment_indexes.is_empty() {
            current_start_ms = Some(segment.start_ms);
        }

        current_segment_indexes.push(idx);
        current_len += segment_len;
    }

    // Don't forget the last chunk
    if let Some(chunk) = build_transcript_chunk(segments, &current_segment_indexes, chunk_index) {
        if chunk.text.len() >= MIN_CHUNK_CHARS || chunks.is_empty() {
            chunks.push(chunk);
        } else if let Some(previous) = chunks.last_mut() {
            let gap_ms = match (previous.end_ms, chunk.start_ms) {
                (Some(previous_end), Some(next_start)) => next_start.saturating_sub(previous_end),
                _ => 0,
            };
            // Keep short trailing chunks when they represent a distinct time range;
            // this preserves precise timestamp navigation and chunk locality.
            if gap_ms <= 20_000 {
                previous.text.push('\n');
                previous.text.push_str(&chunk.text);
                previous.end_ms = chunk.end_ms;
            } else {
                chunks.push(chunk);
            }
        }
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

fn transcript_segment_text(segment: &TranscriptSegmentInput) -> String {
    let speaker_label = segment.speaker.map(speaker_to_label).unwrap_or("SPEAKER");
    format!("[{}] {}\n", speaker_label, segment.text)
}

fn transcript_segment_len(segment: &TranscriptSegmentInput) -> usize {
    let speaker_label = segment.speaker.map(speaker_to_label).unwrap_or("SPEAKER");
    // Prefix format: "[LABEL] " plus trailing '\n'
    speaker_label.len() + segment.text.len() + 4
}

fn build_transcript_chunk(
    segments: &[TranscriptSegmentInput],
    segment_indexes: &[usize],
    chunk_index: usize,
) -> Option<TextChunk> {
    if segment_indexes.is_empty() {
        return None;
    }

    let chunk_text = segment_indexes
        .iter()
        .map(|idx| transcript_segment_text(&segments[*idx]))
        .collect::<String>();
    let trimmed = chunk_text.trim();

    if trimmed.is_empty() {
        return None;
    }

    let first = &segments[*segment_indexes.first()?];
    let last = &segments[*segment_indexes.last()?];

    let mut chunk =
        TextChunk::new(trimmed, chunk_index).with_time_range(first.start_ms, last.end_ms);
    if let Some(speaker) = first.speaker {
        chunk = chunk.with_speaker(speaker);
    }

    Some(chunk)
}

fn overlap_segment_indexes(
    segments: &[TranscriptSegmentInput],
    segment_indexes: &[usize],
    overlap_chars: usize,
) -> Vec<usize> {
    if overlap_chars == 0 || segment_indexes.is_empty() {
        return Vec::new();
    }

    let mut overlap_reversed = Vec::new();
    let mut overlap_len = 0usize;

    for idx in segment_indexes.iter().rev() {
        overlap_reversed.push(*idx);
        overlap_len += transcript_segment_len(&segments[*idx]);
        if overlap_len >= overlap_chars {
            break;
        }
    }

    overlap_reversed.reverse();
    overlap_reversed
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
    fn test_chunk_transcript_respects_duration_boundary() {
        let segments = vec![
            TranscriptSegmentInput {
                text: "Kickoff updates".to_string(),
                start_ms: 0,
                end_ms: 1_000,
                speaker: Some(Speaker::You),
            },
            TranscriptSegmentInput {
                text: "Roadmap discussion".to_string(),
                start_ms: 60_000,
                end_ms: 61_000,
                speaker: Some(Speaker::Others),
            },
            TranscriptSegmentInput {
                text: "Budget review".to_string(),
                start_ms: 181_000,
                end_ms: 182_000,
                speaker: Some(Speaker::You),
            },
        ];

        let chunks = chunk_transcript(&segments, MAX_CHUNK_CHARS);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_ms, Some(0));
        assert_eq!(chunks[0].end_ms, Some(61_000));
        assert_eq!(chunks[1].start_ms, Some(181_000));
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
