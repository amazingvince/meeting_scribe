//! Prompt templates for LLM tasks
//!
//! Contains prompt templates for meeting summarization, action item extraction,
//! title generation, and Q&A. Uses Qwen3 format by default.

/// Format a summarization prompt using Qwen3 template
pub fn summary_prompt(transcript: &str) -> String {
    format!(
        r#"<|im_start|>system
You are a professional meeting assistant. Your task is to create clear, actionable meeting summaries.

Guidelines:
- Be concise but comprehensive
- Use bullet points for lists
- Identify speakers when relevant
- Extract concrete action items with owners
- Note any unresolved questions or concerns<|im_end|>
<|im_start|>user
/no_think

Please summarize the following meeting transcript:

---
{transcript}
---

Provide:
1. **Summary** (2-3 paragraphs)
2. **Key Discussion Points** (bullet list)
3. **Decisions Made** (bullet list)
4. **Action Items** (with owner and deadline if mentioned)
5. **Open Questions** (any unresolved topics)<|im_end|>
<|im_start|>assistant
"#,
        transcript = transcript
    )
}

/// Format an action items extraction prompt
pub fn action_items_prompt(transcript: &str) -> String {
    format!(
        r#"<|im_start|>system
You are a meeting assistant focused on extracting action items.

For each action item, identify:
- What needs to be done
- Who is responsible (if mentioned)
- Deadline (if mentioned)
- Priority (High/Medium/Low based on context)<|im_end|>
<|im_start|>user
/no_think

Extract all action items from this meeting transcript:

---
{transcript}
---

Format as JSON array:
```json
[
  {{
    "task": "Description of the task",
    "owner": "Person name or Unassigned",
    "deadline": "Date or Not specified",
    "priority": "High|Medium|Low"
  }}
]
```<|im_end|>
<|im_start|>assistant
```json
"#,
        transcript = transcript
    )
}

/// Format a title generation prompt
pub fn title_prompt(transcript_start: &str) -> String {
    format!(
        r#"<|im_start|>system
Generate a brief, descriptive title for a meeting based on its opening content.
The title should be 3-8 words, no punctuation at the end.<|im_end|>
<|im_start|>user
/no_think

Opening of meeting transcript:
---
{transcript}
---

Meeting title:<|im_end|>
<|im_start|>assistant
"#,
        transcript = transcript_start
    )
}

/// Format a quick question prompt (direct Q&A without RAG)
pub fn quick_question_prompt(question: &str, transcript: &str) -> String {
    format!(
        r#"<|im_start|>system
You are a meeting assistant. Answer questions about the meeting transcript provided.
Be concise and accurate. If the answer isn't in the transcript, say so.<|im_end|>
<|im_start|>user
/no_think

Meeting transcript:
---
{transcript}
---

Question: {question}<|im_end|>
<|im_start|>assistant
"#,
        transcript = transcript,
        question = question
    )
}

/// Format a RAG chat prompt with retrieved context
pub fn rag_chat_prompt(context: &str, question: &str, chat_history: &str) -> String {
    let history_section = if chat_history.is_empty() {
        String::new()
    } else {
        format!("\nPrevious conversation:\n{}\n", chat_history)
    };

    format!(
        r#"<|im_start|>system
You are a helpful assistant that answers questions about meetings using the provided context.

Rules:
- Only use information from the provided meeting excerpts
- If the answer isn't in the excerpts, say "I don't have that information in the meeting records"
- Cite which meeting or speaker the information comes from when relevant
- Be conversational but accurate<|im_end|>
<|im_start|>user
{history}/no_think

Relevant meeting excerpts:
---
{context}
---

Question: {question}<|im_end|>
<|im_start|>assistant
"#,
        history = history_section,
        context = context,
        question = question
    )
}

/// Format a key points extraction prompt
pub fn key_points_prompt(transcript: &str) -> String {
    format!(
        r#"<|im_start|>system
You are a meeting assistant focused on extracting key discussion points.<|im_end|>
<|im_start|>user
/no_think

Extract the key discussion points from this meeting transcript as a bullet list:

---
{transcript}
---

Key points:<|im_end|>
<|im_start|>assistant
"#,
        transcript = transcript
    )
}

/// Format a decisions extraction prompt
pub fn decisions_prompt(transcript: &str) -> String {
    format!(
        r#"<|im_start|>system
You are a meeting assistant focused on identifying decisions made during meetings.<|im_end|>
<|im_start|>user
/no_think

List all decisions made in this meeting transcript:

---
{transcript}
---

Decisions:<|im_end|>
<|im_start|>assistant
"#,
        transcript = transcript
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_prompt_contains_transcript() {
        let prompt = summary_prompt("Test transcript content");
        assert!(prompt.contains("Test transcript content"));
        assert!(prompt.contains("<|im_start|>"));
        assert!(prompt.contains("<|im_end|>"));
    }

    #[test]
    fn test_action_items_prompt_has_json_format() {
        let prompt = action_items_prompt("Test transcript");
        assert!(prompt.contains("JSON array"));
        assert!(prompt.contains("```json"));
    }

    #[test]
    fn test_title_prompt_is_short() {
        let prompt = title_prompt("Opening content here");
        assert!(prompt.contains("3-8 words"));
    }

    #[test]
    fn test_rag_prompt_with_history() {
        let prompt = rag_chat_prompt("context", "question?", "previous chat");
        assert!(prompt.contains("Previous conversation"));
        assert!(prompt.contains("previous chat"));
    }

    #[test]
    fn test_rag_prompt_without_history() {
        let prompt = rag_chat_prompt("context", "question?", "");
        assert!(!prompt.contains("Previous conversation"));
    }

    #[test]
    fn test_no_think_directive() {
        let prompt = summary_prompt("test");
        assert!(prompt.contains("/no_think"));
    }
}
