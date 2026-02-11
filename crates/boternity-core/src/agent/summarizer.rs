//! Context summarizer for sliding window management.
//!
//! `ContextSummarizer` condenses older conversation messages into a concise
//! summary when the conversation approaches the token limit. This preserves
//! key context while freeing token budget for new messages, preventing
//! personality drift in long conversations.

use boternity_types::llm::{CompletionRequest, LlmError, Message, MessageRole};

use crate::llm::box_provider::BoxLlmProvider;

/// System prompt for the context summarization LLM call.
const SUMMARY_SYSTEM_PROMPT: &str = r#"Summarize the following conversation segment concisely. Preserve:
1. Key decisions and conclusions
2. Important facts mentioned
3. The user's current goals and context
4. Any unresolved questions

Keep the summary under 500 words. Write in third person (e.g., "The user asked about..." "The assistant recommended...")."#;

/// Stateless utility for summarizing conversation context.
///
/// Used by the sliding window manager to condense older messages into
/// a summary, freeing token budget for recent conversation while
/// maintaining continuity.
pub struct ContextSummarizer;

impl ContextSummarizer {
    /// Summarize a set of messages into a concise text summary.
    ///
    /// Sends the messages to the LLM with instructions to preserve key
    /// decisions, facts, goals, and unresolved questions.
    #[tracing::instrument(
        name = "summarize_context",
        skip(provider, messages),
        fields(
            model = %model,
            message_count = messages.len(),
        )
    )]
    pub async fn summarize(
        provider: &BoxLlmProvider,
        messages: &[Message],
        model: &str,
    ) -> Result<String, LlmError> {
        if messages.is_empty() {
            return Ok(String::new());
        }

        // Format the conversation for the summarizer
        let conversation_text: String = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = CompletionRequest {
            model: model.to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: format!(
                    "Please summarize this conversation:\n\n<conversation>\n{conversation_text}\n</conversation>"
                ),
            }],
            system: Some(SUMMARY_SYSTEM_PROMPT.to_string()),
            max_tokens: 1024,
            temperature: Some(0.0),
            stream: false,
            stop_sequences: None,
        };

        let response = provider.complete(&request).await?;
        Ok(response.content.trim().to_string())
    }

    /// Split messages into two slices: those to summarize, and those to keep.
    ///
    /// Returns `(to_summarize, to_keep)` where `to_keep` contains the most
    /// recent `keep_recent` messages and `to_summarize` contains everything
    /// before them.
    ///
    /// If there are fewer than or equal to `keep_recent` messages, returns
    /// an empty slice for `to_summarize` and all messages for `to_keep`.
    pub fn select_messages_to_summarize(
        messages: &[Message],
        keep_recent: usize,
    ) -> (&[Message], &[Message]) {
        if messages.len() <= keep_recent {
            (&[], messages)
        } else {
            let split_point = messages.len() - keep_recent;
            (&messages[..split_point], &messages[split_point..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_messages_fewer_than_keep() {
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Hi!".to_string(),
            },
        ];

        let (to_summarize, to_keep) =
            ContextSummarizer::select_messages_to_summarize(&messages, 5);
        assert!(to_summarize.is_empty());
        assert_eq!(to_keep.len(), 2);
    }

    #[test]
    fn test_select_messages_exact_keep() {
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "One".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Two".to_string(),
            },
        ];

        let (to_summarize, to_keep) =
            ContextSummarizer::select_messages_to_summarize(&messages, 2);
        assert!(to_summarize.is_empty());
        assert_eq!(to_keep.len(), 2);
    }

    #[test]
    fn test_select_messages_splits_correctly() {
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "Oldest".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Old reply".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "Middle".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Middle reply".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "Recent".to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Recent reply".to_string(),
            },
        ];

        let (to_summarize, to_keep) =
            ContextSummarizer::select_messages_to_summarize(&messages, 2);
        assert_eq!(to_summarize.len(), 4);
        assert_eq!(to_keep.len(), 2);
        assert_eq!(to_keep[0].content, "Recent");
        assert_eq!(to_keep[1].content, "Recent reply");
        assert_eq!(to_summarize[0].content, "Oldest");
        assert_eq!(to_summarize[3].content, "Middle reply");
    }

    #[test]
    fn test_select_messages_empty() {
        let messages: Vec<Message> = vec![];
        let (to_summarize, to_keep) =
            ContextSummarizer::select_messages_to_summarize(&messages, 5);
        assert!(to_summarize.is_empty());
        assert!(to_keep.is_empty());
    }

    #[test]
    fn test_summary_system_prompt_instructions() {
        assert!(SUMMARY_SYSTEM_PROMPT.contains("Key decisions"));
        assert!(SUMMARY_SYSTEM_PROMPT.contains("Important facts"));
        assert!(SUMMARY_SYSTEM_PROMPT.contains("third person"));
        assert!(SUMMARY_SYSTEM_PROMPT.contains("500 words"));
    }
}
