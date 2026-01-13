//! Summarization service for meeting content
//!
//! Provides high-level API for generating summaries, extracting action items,
//! and other structured outputs from meeting transcripts.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::llm::{prepare_transcript_for_llm, GenerationConfig, LlmService};
use super::prompts;

/// Maximum transcript length in characters for LLM processing
const MAX_TRANSCRIPT_CHARS: usize = 12000;

/// Types of summaries that can be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SummaryType {
    /// Complete summary with all sections
    Full,
    /// Brief 2-3 sentence overview
    Brief,
    /// Just action items
    ActionItems,
    /// Just key discussion points
    KeyPoints,
    /// Just decisions made
    Decisions,
}

/// Action item extracted from meeting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    /// Description of what needs to be done
    pub task: String,
    /// Person responsible (if identified)
    pub owner: Option<String>,
    /// Deadline (if mentioned)
    pub deadline: Option<String>,
    /// Priority level
    pub priority: Priority,
}

/// Priority level for action items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium
    }
}

/// Raw action item format from LLM JSON output
#[derive(Debug, Deserialize)]
struct ActionItemRaw {
    task: String,
    owner: String,
    deadline: String,
    priority: String,
}

/// Summarization service wrapping LlmService
pub struct SummarizationService<'a> {
    llm: &'a LlmService,
}

impl<'a> SummarizationService<'a> {
    /// Create a new summarization service
    pub fn new(llm: &'a LlmService) -> Self {
        Self { llm }
    }

    /// Generate a full meeting summary
    pub fn summarize(&self, transcript: &str) -> Result<String> {
        info!(
            "Generating meeting summary for {} characters",
            transcript.len()
        );

        // Prepare transcript (truncate if too long)
        let prepared = prepare_transcript_for_llm(transcript, MAX_TRANSCRIPT_CHARS);

        // Generate summary
        let prompt = prompts::summary_prompt(&prepared);
        let config = GenerationConfig::for_summarization();

        self.llm.generate(&prompt, &config)
    }

    /// Generate a brief summary (2-3 sentences)
    pub fn summarize_brief(&self, transcript: &str) -> Result<String> {
        let prepared = prepare_transcript_for_llm(transcript, MAX_TRANSCRIPT_CHARS);

        let prompt = format!(
            r#"<|im_start|>system
Provide a brief 2-3 sentence summary of the meeting.<|im_end|>
<|im_start|>user
/no_think

Meeting transcript:
---
{}
---

Brief summary:<|im_end|>
<|im_start|>assistant
"#,
            prepared
        );

        let config = GenerationConfig {
            max_tokens: 200,
            temperature: 0.3,
            ..GenerationConfig::default()
        };

        self.llm.generate(&prompt, &config)
    }

    /// Extract action items as structured data
    pub fn extract_action_items(&self, transcript: &str) -> Result<Vec<ActionItem>> {
        info!("Extracting action items from transcript");

        let prepared = prepare_transcript_for_llm(transcript, MAX_TRANSCRIPT_CHARS);
        let prompt = prompts::action_items_prompt(&prepared);
        let config = GenerationConfig::for_json();

        let output = self.llm.generate(&prompt, &config)?;

        // Parse JSON output
        let items: Vec<ActionItemRaw> = serde_json::from_str(&output).unwrap_or_default();

        // Convert to structured ActionItem
        Ok(items
            .into_iter()
            .map(|raw| ActionItem {
                task: raw.task,
                owner: if raw.owner == "Unassigned" || raw.owner.is_empty() {
                    None
                } else {
                    Some(raw.owner)
                },
                deadline: if raw.deadline == "Not specified" || raw.deadline.is_empty() {
                    None
                } else {
                    Some(raw.deadline)
                },
                priority: match raw.priority.to_lowercase().as_str() {
                    "high" => Priority::High,
                    "low" => Priority::Low,
                    _ => Priority::Medium,
                },
            })
            .collect())
    }

    /// Extract key discussion points
    pub fn extract_key_points(&self, transcript: &str) -> Result<String> {
        let prepared = prepare_transcript_for_llm(transcript, MAX_TRANSCRIPT_CHARS);
        let prompt = prompts::key_points_prompt(&prepared);
        let config = GenerationConfig::for_summarization();

        self.llm.generate(&prompt, &config)
    }

    /// Extract decisions made in the meeting
    pub fn extract_decisions(&self, transcript: &str) -> Result<String> {
        let prepared = prepare_transcript_for_llm(transcript, MAX_TRANSCRIPT_CHARS);
        let prompt = prompts::decisions_prompt(&prepared);
        let config = GenerationConfig::for_summarization();

        self.llm.generate(&prompt, &config)
    }

    /// Generate a meeting title from the transcript
    pub fn generate_title(&self, transcript_start: &str) -> Result<String> {
        info!("Generating meeting title");

        // Only use the first ~2000 chars for title generation
        let truncated = if transcript_start.len() > 2000 {
            &transcript_start[..2000]
        } else {
            transcript_start
        };

        let prompt = prompts::title_prompt(truncated);
        let config = GenerationConfig::for_title();

        let title = self.llm.generate(&prompt, &config)?;

        // Clean up the title
        let title = title
            .lines()
            .next()
            .unwrap_or(&title)
            .trim()
            .trim_matches('"')
            .trim_matches('*')
            .to_string();

        Ok(title)
    }

    /// Answer a question about the transcript
    pub fn answer_question(&self, question: &str, transcript: &str) -> Result<String> {
        let prepared = prepare_transcript_for_llm(transcript, MAX_TRANSCRIPT_CHARS);
        let prompt = prompts::quick_question_prompt(question, &prepared);
        let config = GenerationConfig::for_chat();

        self.llm.generate(&prompt, &config)
    }

    /// Answer a question with RAG context
    pub fn answer_with_context(
        &self,
        question: &str,
        context: &str,
        chat_history: &str,
    ) -> Result<String> {
        let prompt = prompts::rag_chat_prompt(context, question, chat_history);
        let config = GenerationConfig::for_chat();

        self.llm.generate(&prompt, &config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_default() {
        assert_eq!(Priority::default(), Priority::Medium);
    }

    #[test]
    fn test_action_item_creation() {
        let item = ActionItem {
            task: "Review document".to_string(),
            owner: Some("John".to_string()),
            deadline: Some("Friday".to_string()),
            priority: Priority::High,
        };
        assert_eq!(item.task, "Review document");
        assert_eq!(item.priority, Priority::High);
    }

    #[test]
    fn test_summary_type_variants() {
        let types = vec![
            SummaryType::Full,
            SummaryType::Brief,
            SummaryType::ActionItems,
            SummaryType::KeyPoints,
            SummaryType::Decisions,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_action_item_json_parsing() {
        let json = r#"[{"task": "Review", "owner": "John", "deadline": "Friday", "priority": "High"}]"#;
        let items: Vec<ActionItemRaw> = serde_json::from_str(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].task, "Review");
    }

    #[test]
    fn test_action_item_json_empty_fields() {
        let json =
            r#"[{"task": "Task", "owner": "Unassigned", "deadline": "Not specified", "priority": "medium"}]"#;
        let items: Vec<ActionItemRaw> = serde_json::from_str(json).unwrap();
        let action = ActionItem {
            task: items[0].task.clone(),
            owner: if items[0].owner == "Unassigned" {
                None
            } else {
                Some(items[0].owner.clone())
            },
            deadline: if items[0].deadline == "Not specified" {
                None
            } else {
                Some(items[0].deadline.clone())
            },
            priority: Priority::Medium,
        };
        assert!(action.owner.is_none());
        assert!(action.deadline.is_none());
    }
}
