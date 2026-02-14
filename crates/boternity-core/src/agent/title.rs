//! Session title generation via LLM.
//!
//! `generate_title` creates a short, descriptive title for a chat session
//! based on the first user-assistant exchange. Titles are auto-generated
//! in the style of ChatGPT conversation naming.

use boternity_types::llm::{CompletionRequest, LlmError, Message, MessageRole};

use crate::llm::box_provider::BoxLlmProvider;

/// System prompt for the title generation LLM call.
const TITLE_SYSTEM_PROMPT: &str = r#"Generate a short, descriptive title (3-7 words) for this conversation based on the first exchange. The title should capture the main topic or intent. Return ONLY the title text, nothing else.

Examples:
- "Debugging Rust lifetime errors"
- "Planning a weekend trip to Tokyo"
- "Understanding quantum computing basics"
- "Recipe ideas for dinner party""#;

/// Generate a session title from the first user-assistant exchange.
///
/// Uses an LLM call at low temperature (0.3) with a strict prompt to
/// produce a concise title. The result is trimmed of whitespace and
/// surrounding quotes.
///
/// # Arguments
/// * `provider` - The LLM provider for the title generation call
/// * `first_user_message` - The first message the user sent
/// * `first_assistant_message` - The bot's first response
/// * `model` - The model to use for title generation
#[tracing::instrument(
    name = "generate_title",
    skip(provider, first_user_message, first_assistant_message),
    fields(model = %model)
)]
pub async fn generate_title(
    provider: &BoxLlmProvider,
    first_user_message: &str,
    first_assistant_message: &str,
    model: &str,
) -> Result<String, LlmError> {
    let request = CompletionRequest {
        model: model.to_string(),
        messages: vec![
            Message {
                role: MessageRole::User,
                content: first_user_message.to_string(),
            },
            Message {
                role: MessageRole::Assistant,
                content: first_assistant_message.to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "Based on our exchange above, generate a title.".to_string(),
            },
        ],
        system: Some(TITLE_SYSTEM_PROMPT.to_string()),
        max_tokens: 50,
        temperature: Some(0.3),
        stream: false,
        stop_sequences: None,
        output_config: None,
    };

    let response = provider.complete(&request).await?;

    // Trim whitespace and surrounding quotes from the title
    let title = response
        .content
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();

    Ok(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_trimming() {
        // Simulate what the function does to the response
        let raw = "  \"Debugging Rust Lifetimes\"  ";
        let title = raw.trim().trim_matches('"').trim_matches('\'').trim();
        assert_eq!(title, "Debugging Rust Lifetimes");
    }

    #[test]
    fn test_title_trimming_single_quotes() {
        let raw = "'Planning a Trip'";
        let title = raw.trim().trim_matches('"').trim_matches('\'').trim();
        assert_eq!(title, "Planning a Trip");
    }

    #[test]
    fn test_title_trimming_no_quotes() {
        let raw = "  Understanding Quantum Computing  ";
        let title = raw.trim().trim_matches('"').trim_matches('\'').trim();
        assert_eq!(title, "Understanding Quantum Computing");
    }

    #[test]
    fn test_title_system_prompt_constraints() {
        assert!(TITLE_SYSTEM_PROMPT.contains("3-7 words"));
        assert!(TITLE_SYSTEM_PROMPT.contains("ONLY the title text"));
        assert!(TITLE_SYSTEM_PROMPT.contains("main topic or intent"));
    }
}
