//! Speaker labeling and transcript merging
//!
//! Merges transcripts from microphone (You) and system audio (Others)
//! into a single chronological transcript.

use super::transcription::{format_timestamp, Speaker, TranscriptSegment};
use std::collections::HashSet;
use tracing::{debug, info};

const ECHO_MAX_TIME_OFFSET_MS: u64 = 1200;
const ECHO_MIN_OVERLAP_RATIO: f32 = 0.45;
const ECHO_MIN_TEXT_CHARS: usize = 10;
const ECHO_TEXT_SIMILARITY_THRESHOLD: f32 = 0.72;

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
    let filtered_mic_segments = filter_echoed_mic_segments(mic_segments, &system_segments);
    let filtered_mic_count = filtered_mic_segments.len();
    let dropped_mic = mic_count.saturating_sub(filtered_mic_count);
    let total_count = filtered_mic_count + system_count;

    info!(
        "Merging transcripts: {} mic segments, {} system segments",
        mic_count, system_count
    );
    if dropped_mic > 0 {
        info!(
            "Dropped {} likely echoed mic segment(s) before merge",
            dropped_mic
        );
    }

    let mut all_segments = Vec::with_capacity(total_count);

    // Add mic segments with "You" speaker
    for mut segment in filtered_mic_segments {
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

/// Remove microphone segments that are likely just leaked playback from speakers.
fn filter_echoed_mic_segments(
    mic_segments: Vec<TranscriptSegment>,
    system_segments: &[TranscriptSegment],
) -> Vec<TranscriptSegment> {
    if mic_segments.is_empty() || system_segments.is_empty() {
        return mic_segments;
    }

    let mut sorted_system = system_segments.to_vec();
    sorted_system.sort_by_key(|s| s.start_ms);

    let mut filtered = Vec::with_capacity(mic_segments.len());
    let mut scan_start = 0usize;

    for mic in mic_segments {
        while scan_start < sorted_system.len()
            && sorted_system[scan_start]
                .end_ms
                .saturating_add(ECHO_MAX_TIME_OFFSET_MS)
                < mic.start_ms
        {
            scan_start += 1;
        }

        let mut echoed = false;
        let mut idx = scan_start;
        while idx < sorted_system.len()
            && sorted_system[idx].start_ms <= mic.end_ms.saturating_add(ECHO_MAX_TIME_OFFSET_MS)
        {
            if is_likely_echo_segment(&mic, &sorted_system[idx]) {
                echoed = true;
                break;
            }
            idx += 1;
        }

        if !echoed {
            filtered.push(mic);
        }
    }

    filtered
}

fn is_likely_echo_segment(mic: &TranscriptSegment, system: &TranscriptSegment) -> bool {
    let overlap_ratio = temporal_overlap_ratio(mic, system, ECHO_MAX_TIME_OFFSET_MS);
    if overlap_ratio < ECHO_MIN_OVERLAP_RATIO {
        return false;
    }

    let mic_text = normalize_for_similarity(&mic.text);
    let system_text = normalize_for_similarity(&system.text);

    if mic_text.len() < ECHO_MIN_TEXT_CHARS || system_text.len() < ECHO_MIN_TEXT_CHARS {
        return false;
    }

    text_similarity_score(&mic_text, &system_text) >= ECHO_TEXT_SIMILARITY_THRESHOLD
}

fn temporal_overlap_ratio(a: &TranscriptSegment, b: &TranscriptSegment, tolerance_ms: u64) -> f32 {
    let a_start = a.start_ms as i64;
    let a_end = a.end_ms as i64;
    let b_start = b.start_ms.saturating_sub(tolerance_ms) as i64;
    let b_end = b.end_ms.saturating_add(tolerance_ms) as i64;

    let overlap_start = a_start.max(b_start);
    let overlap_end = a_end.min(b_end);
    if overlap_end <= overlap_start {
        return 0.0;
    }

    let overlap = (overlap_end - overlap_start) as u64;
    let base = a.duration_ms().min(b.duration_ms()).max(1);
    overlap as f32 / base as f32
}

fn normalize_for_similarity(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = true;
    for c in text.chars() {
        let normalized = if c.is_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            ' '
        };

        if normalized == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(normalized);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn text_similarity_score(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    if a == b {
        return 1.0;
    }

    if a.contains(b) || b.contains(a) {
        let shorter = a.len().min(b.len()) as f32;
        let longer = a.len().max(b.len()) as f32;
        return (shorter / longer).max(0.78);
    }

    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();

    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }

    let intersection = set_a.intersection(&set_b).count() as f32;
    if intersection == 0.0 {
        return 0.0;
    }

    let min_len = set_a.len().min(set_b.len()) as f32;
    let union_len = set_a.union(&set_b).count() as f32;
    let containment = intersection / min_len;
    let jaccard = intersection / union_len;

    containment.max(jaccard)
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

    #[test]
    fn test_merge_transcripts_filters_likely_echoed_mic_segment() {
        let mic = vec![
            make_segment(
                1000,
                3200,
                "Let's review Q4 pipeline metrics today",
                Speaker::Unknown,
            ),
            make_segment(
                5000,
                6700,
                "Can we schedule the customer follow up",
                Speaker::Unknown,
            ),
        ];
        let system = vec![make_segment(
            900,
            3300,
            "Let's review q4 pipeline metrics today",
            Speaker::Unknown,
        )];

        let merged = merge_transcripts(mic, system);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].speaker, Speaker::Others);
        assert_eq!(merged[1].speaker, Speaker::You);
        assert!(merged[1].text.contains("customer follow up"));
    }

    #[test]
    fn test_merge_transcripts_keeps_overlapping_different_content() {
        let mic = vec![make_segment(
            1000,
            3200,
            "I will send the action items after this call",
            Speaker::Unknown,
        )];
        let system = vec![make_segment(
            900,
            3300,
            "The architecture review starts next week",
            Speaker::Unknown,
        )];

        let merged = merge_transcripts(mic, system);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].speaker, Speaker::Others);
        assert_eq!(merged[1].speaker, Speaker::You);
    }
}
