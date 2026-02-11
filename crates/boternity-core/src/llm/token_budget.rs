//! Token budget management for LLM context windows.
//!
//! Allocates the finite context window across competing priorities:
//! soul prompt, memories, user context, and conversation history.

use boternity_types::llm::ProviderCapabilities;

/// Manages the allocation of an LLM's context window across priorities.
///
/// Budget allocation:
/// - Soul (system prompt): 15% of context
/// - Memory (long-term facts): 10% of context
/// - User context (current session metadata): 5% of context
/// - Conversation (recent messages): 70% of context
///
/// The conversation budget is the largest because most tokens go to
/// the ongoing exchange between user and bot.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub max_context_tokens: u32,
    pub soul_budget: u32,
    pub memory_budget: u32,
    pub user_context_budget: u32,
    pub conversation_budget: u32,
}

impl TokenBudget {
    /// Create a new token budget from a maximum context size.
    ///
    /// Allocates: soul 15%, memory 10%, user_context 5%, conversation 70%.
    pub fn new(max_context: u32) -> Self {
        Self {
            max_context_tokens: max_context,
            soul_budget: max_context * 15 / 100,
            memory_budget: max_context * 10 / 100,
            user_context_budget: max_context * 5 / 100,
            conversation_budget: max_context * 70 / 100,
        }
    }

    /// Calculate how many conversation tokens remain given current usage.
    pub fn conversation_remaining(&self, used: u32) -> u32 {
        self.conversation_budget.saturating_sub(used)
    }

    /// Whether the conversation should be summarized (compressed).
    ///
    /// Returns `true` when conversation tokens exceed 80% of the
    /// conversation budget, signaling the sliding window should
    /// summarize older messages to free space.
    pub fn should_summarize(&self, conversation_tokens: u32) -> bool {
        conversation_tokens > self.conversation_budget * 80 / 100
    }

    /// Derive a token budget from provider capabilities.
    pub fn from_capabilities(caps: &ProviderCapabilities) -> Self {
        Self::new(caps.max_context_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_allocation() {
        let budget = TokenBudget::new(100_000);
        assert_eq!(budget.soul_budget, 15_000);
        assert_eq!(budget.memory_budget, 10_000);
        assert_eq!(budget.user_context_budget, 5_000);
        assert_eq!(budget.conversation_budget, 70_000);
    }

    #[test]
    fn test_conversation_remaining() {
        let budget = TokenBudget::new(100_000);
        assert_eq!(budget.conversation_remaining(50_000), 20_000);
        assert_eq!(budget.conversation_remaining(70_000), 0);
        assert_eq!(budget.conversation_remaining(80_000), 0); // saturating sub
    }

    #[test]
    fn test_should_summarize() {
        let budget = TokenBudget::new(100_000);
        // 80% of 70_000 = 56_000
        assert!(!budget.should_summarize(55_000));
        assert!(budget.should_summarize(57_000));
        assert!(budget.should_summarize(70_000));
    }

    #[test]
    fn test_from_capabilities() {
        let caps = ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false,
            extended_thinking: false,
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
        };
        let budget = TokenBudget::from_capabilities(&caps);
        assert_eq!(budget.max_context_tokens, 200_000);
        assert_eq!(budget.conversation_budget, 140_000);
    }
}
