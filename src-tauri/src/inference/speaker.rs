//! Speaker labeling and transcript merging
//!
//! Merges transcripts from microphone (You) and system audio (Others)
//! into a single chronological transcript.

use super::transcription::{format_timestamp, Speaker, TranscriptSegment};
use tracing::{debug, info};

/// Merge transcripts from mic and system audio
///
/// Combines segments from both sources into a chronological order,
/// labeling mic segments as "You" and system segments as "Others".
pub fn merge_transcripts(
    mic_segments: Vec<TranscriptSegment>,
    system_segments: Vec<TranscriptSegment>,
) -> Vec<TranscriptSegment> {
    let mic_count = mic_segments.len();
    let system_count = system_segments.len();
    let total_count = mic_count + system_count;

    info!(
        "Merging transcripts: {} mic segments, {} system segments",
        mic_count, system_count
    );

    let mut all_segments = Vec::with_capacity(total_count);

    // Add mic segments with "You" speaker
    for mut segment in mic_segments {
        segment.speaker = Speaker::You;
        all_segments.push(segment);
    }

    // Add system segments with "Others" speaker
    for mut segment in system_segments {
        segment.speaker = Speaker::Others;
        all_segments.push(segment);
    }

    // Sort by start time
    all_segments.sort_by_key(|s| s.start_ms);

    // Log first few segments for debugging
    for (i, seg) in all_segments.iter().take(5).enumerate() {
        debug!(
            "Segment {}: {}ms-{}ms {:?} '{:.50}...'",
            i, seg.start_ms, seg.end_ms, seg.speaker, seg.text
        );
    }

    // Optionally merge overlapping segments from the same speaker
    let merged = merge_consecutive_segments(all_segments);

    info!(
        "After merge: {} segments (from {} total)",
        merged.len(),
        total_count
    );

    merged
}

/// Merge consecutive segments from the same speaker if they're close together
fn merge_consecutive_segments(segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    if segments.is_empty() {
        return segments;
    }

    // Only merge truly overlapping or near-adjacent segments.
    // 500ms was too aggressive and could merge separate utterances.
    const MAX_GAP_MS: u64 = 250;

    let mut merged = Vec::with_capacity(segments.len());
    let mut current = segments[0].clone();

    for segment in segments.into_iter().skip(1) {
        // Check if we should merge with current segment
        let same_speaker = segment.speaker == current.speaker;
        let small_gap = segment.start_ms.saturating_sub(current.end_ms) <= MAX_GAP_MS;

        if same_speaker && small_gap {
            // Merge: extend current segment
            current.end_ms = segment.end_ms;
            current.text = format!("{} {}", current.text, segment.text);

            // Average confidence if both have it
            current.confidence = match (current.confidence, segment.confidence) {
                (Some(c1), Some(c2)) => Some((c1 + c2) / 2.0),
                (Some(c), None) | (None, Some(c)) => Some(c),
                (None, None) => None,
            };
        } else {
            // Don't merge: save current and start new
            merged.push(current);
            current = segment;
        }
    }

    // Don't forget the last segment
    merged.push(current);

    merged
}

/// Handle overlapping speech (when both speakers talk simultaneously)
///
/// This creates separate segments for the overlapping portions.
pub fn split_overlapping_segments(segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    if segments.len() <= 1 {
        return segments;
    }

    let mut result = Vec::with_capacity(segments.len() * 2);
    let mut i = 0;

    while i < segments.len() {
        let current = &segments[i];

        // Find overlapping segments
        let mut j = i + 1;
        let mut overlaps = Vec::new();

        while j < segments.len() && segments[j].start_ms < current.end_ms {
            if segments[j].speaker != current.speaker {
                overlaps.push(j);
            }
            j += 1;
        }

        if overlaps.is_empty() {
            // No overlap, keep as is
            result.push(current.clone());
        } else {
            // Has overlaps - just keep both segments (they'll be interleaved)
            result.push(current.clone());
        }

        i += 1;
    }

    result
}

/// Format transcript as a human-readable string
pub fn format_transcript(segments: &[TranscriptSegment]) -> String {
    let mut output = String::new();

    for segment in segments {
        output.push_str(&format!(
            "[{}] {}: {}\n",
            format_timestamp(segment.start_ms),
            segment.speaker,
            segment.text
        ));
    }

    output
}

/// Format transcript with only speaker changes shown
pub fn format_transcript_compact(segments: &[TranscriptSegment]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mut current_speaker = None;

    for segment in segments {
        if current_speaker != Some(segment.speaker) {
            if !output.is_empty() {
                output.push_str("\n\n");
            }
            output.push_str(&format!(
                "**{}** ({})\n",
                segment.speaker,
                format_timestamp(segment.start_ms)
            ));
            current_speaker = Some(segment.speaker);
        }
        output.push_str(&segment.text);
        output.push(' ');
    }

    output.trim_end().to_string()
}

/// Calculate transcript statistics
#[derive(Debug, Clone)]
pub struct TranscriptStats {
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// Number of segments
    pub segment_count: usize,
    /// Number of segments by You
    pub you_segments: usize,
    /// Number of segments by Others
    pub others_segments: usize,
    /// Total words spoken
    pub word_count: usize,
    /// Words spoken by You
    pub you_words: usize,
    /// Words spoken by Others
    pub others_words: usize,
}

impl TranscriptStats {
    /// Calculate statistics from segments
    pub fn from_segments(segments: &[TranscriptSegment]) -> Self {
        let mut stats = Self {
            duration_ms: 0,
            segment_count: segments.len(),
            you_segments: 0,
            others_segments: 0,
            word_count: 0,
            you_words: 0,
            others_words: 0,
        };

        if segments.is_empty() {
            return stats;
        }

        let start = segments.iter().map(|s| s.start_ms).min().unwrap_or(0);
        let end = segments.iter().map(|s| s.end_ms).max().unwrap_or(0);
        stats.duration_ms = end.saturating_sub(start);

        for segment in segments {
            let words = segment.text.split_whitespace().count();
            stats.word_count += words;

            match segment.speaker {
                Speaker::You => {
                    stats.you_segments += 1;
                    stats.you_words += words;
                }
                Speaker::Others | Speaker::Unknown => {
                    stats.others_segments += 1;
                    stats.others_words += words;
                }
            }
        }

        stats
    }

    /// Get talk ratio (You vs Others) as a percentage
    pub fn you_talk_ratio(&self) -> f32 {
        if self.word_count == 0 {
            return 50.0;
        }
        (self.you_words as f32 / self.word_count as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_segment(start_ms: u64, end_ms: u64, text: &str, speaker: Speaker) -> TranscriptSegment {
        TranscriptSegment {
            start_ms,
            end_ms,
            text: text.to_string(),
            speaker,
            confidence: None,
        }
    }

    #[test]
    fn test_merge_transcripts() {
        let mic = vec![
            make_segment(0, 2000, "Hello", Speaker::Unknown),
            make_segment(5000, 7000, "How are you", Speaker::Unknown),
        ];

        let system = vec![make_segment(2500, 4500, "Hi there", Speaker::Unknown)];

        let merged = merge_transcripts(mic, system);

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].speaker, Speaker::You);
        assert_eq!(merged[1].speaker, Speaker::Others);
        assert_eq!(merged[2].speaker, Speaker::You);

        // Check chronological order
        assert!(merged[0].start_ms <= merged[1].start_ms);
        assert!(merged[1].start_ms <= merged[2].start_ms);
    }

    #[test]
    fn test_merge_consecutive_same_speaker() {
        let segments = vec![
            make_segment(0, 1000, "Hello", Speaker::You),
            make_segment(1200, 2000, "world", Speaker::You), // Close, same speaker
            make_segment(5000, 6000, "Other speech", Speaker::Others),
        ];

        let merged = merge_consecutive_segments(segments);

        assert_eq!(merged.len(), 2); // First two should be merged
        assert_eq!(merged[0].text, "Hello world");
        assert_eq!(merged[0].speaker, Speaker::You);
        assert_eq!(merged[1].speaker, Speaker::Others);
    }

    #[test]
    fn test_format_transcript() {
        let segments = vec![
            make_segment(0, 2000, "Hello", Speaker::You),
            make_segment(2500, 4500, "Hi there", Speaker::Others),
        ];

        let formatted = format_transcript(&segments);

        assert!(formatted.contains("[00:00] You: Hello"));
        assert!(formatted.contains("[00:02] Others: Hi there"));
    }

    #[test]
    fn test_transcript_stats() {
        let segments = vec![
            make_segment(0, 2000, "Hello world", Speaker::You),
            make_segment(2500, 4500, "Hi there friend", Speaker::Others),
        ];

        let stats = TranscriptStats::from_segments(&segments);

        assert_eq!(stats.segment_count, 2);
        assert_eq!(stats.you_segments, 1);
        assert_eq!(stats.others_segments, 1);
        assert_eq!(stats.word_count, 5);
        assert_eq!(stats.you_words, 2);
        assert_eq!(stats.others_words, 3);
        assert!(stats.you_talk_ratio() < 50.0); // 2/5 = 40%
    }

    #[test]
    fn test_empty_merge() {
        let merged = merge_transcripts(vec![], vec![]);
        assert!(merged.is_empty());
    }
}
