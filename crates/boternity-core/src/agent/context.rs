//! Agent conversation context for Boternity.
//!
//! AgentContext holds all the state needed for a conversation: personality
//! content, memories, conversation history, token budget, and the assembled
//! system prompt. Long-term vector memories are injected into the system
//! prompt via a `<long_term_memory>` section when available.

use boternity_types::agent::AgentConfig;
use boternity_types::llm::{Message, MessageRole};
use boternity_types::memory::{MemoryEntry, RankedMemory};

use crate::llm::token_budget::TokenBudget;

use super::prompt::SystemPromptBuilder;

/// Holds all state needed for an agent conversation.
///
/// Created at session start with the bot's personality files and memories,
/// then tracks conversation history and token usage throughout the session.
/// Long-term memories from vector search are stored in `recalled_memories`
/// and injected into the system prompt on each rebuild.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Agent identity and LLM configuration.
    pub agent_config: AgentConfig,
    /// Content from SOUL.md -- the bot's core personality.
    pub soul_content: String,
    /// Content from IDENTITY.md -- structured config.
    pub identity_content: String,
    /// Content from USER.md -- standing user instructions.
    pub user_content: String,
    /// Extracted memories from previous sessions (session-scoped).
    pub memories: Vec<MemoryEntry>,
    /// Semantically recalled long-term memories from vector search.
    ///
    /// Populated before each LLM call by the caller (ChatService).
    /// Injected into the system prompt as a `<long_term_memory>` section.
    pub recalled_memories: Vec<RankedMemory>,
    /// Running conversation history (user + assistant messages).
    pub conversation_history: Vec<Message>,
    /// Token budget for context window management.
    pub token_budget: TokenBudget,
    /// Pre-built system prompt assembled from personality + memories.
    pub system_prompt: String,
    /// Whether verbose mode is enabled (shows memory injection details).
    pub verbose: bool,
}

impl AgentContext {
    /// Create a new agent context with all personality sources.
    ///
    /// Builds the system prompt immediately from the provided content.
    pub fn new(
        config: AgentConfig,
        soul: String,
        identity: String,
        user: String,
        memories: Vec<MemoryEntry>,
        token_budget: TokenBudget,
    ) -> Self {
        let system_prompt =
            SystemPromptBuilder::build(&config, &soul, &identity, &user, &memories, &[]);

        Self {
            agent_config: config,
            soul_content: soul,
            identity_content: identity,
            user_content: user,
            memories,
            recalled_memories: Vec::new(),
            conversation_history: Vec::new(),
            token_budget,
            system_prompt,
            verbose: false,
        }
    }

    /// Set verbose mode (shows which memories were injected).
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Update the recalled long-term memories and rebuild the system prompt.
    ///
    /// Called before each LLM request with fresh vector search results.
    /// The system prompt is rebuilt to include the new `<long_term_memory>` section.
    pub fn set_recalled_memories(&mut self, memories: Vec<RankedMemory>) {
        self.recalled_memories = memories;
        self.rebuild_system_prompt();
    }

    /// Rebuild the system prompt from current state.
    ///
    /// Called after recalled_memories changes to keep the system prompt
    /// in sync with the latest vector search results.
    fn rebuild_system_prompt(&mut self) {
        self.system_prompt = SystemPromptBuilder::build(
            &self.agent_config,
            &self.soul_content,
            &self.identity_content,
            &self.user_content,
            &self.memories,
            &self.recalled_memories,
        );
    }

    /// Add a user message to the conversation history.
    pub fn add_user_message(&mut self, content: String) {
        self.conversation_history.push(Message {
            role: MessageRole::User,
            content,
        });
    }

    /// Add an assistant message to the conversation history.
    pub fn add_assistant_message(&mut self, content: String) {
        self.conversation_history.push(Message {
            role: MessageRole::Assistant,
            content,
        });
    }

    /// Build the message list for an LLM request.
    ///
    /// Returns the conversation history as a `Vec<Message>`.
    /// The system prompt is sent separately (not as a message) per the
    /// Anthropic API convention.
    pub fn build_messages(&self) -> Vec<Message> {
        self.conversation_history.clone()
    }

    /// Whether the conversation should be summarized to free token budget.
    ///
    /// Estimates conversation tokens from the character count of all messages
    /// (rough heuristic: 1 token ~ 4 chars). Returns true when estimated
    /// usage exceeds the token budget's summarization threshold (80% of
    /// conversation budget).
    pub fn should_summarize(&self) -> bool {
        let estimated_tokens = self.estimate_conversation_tokens();
        self.token_budget.should_summarize(estimated_tokens)
    }

    /// Create a child context for a sub-agent task.
    ///
    /// Inherits the bot's personality (SOUL.md) and model config but gets:
    /// - Fresh conversation history (empty)
    /// - Fresh recalled memories (empty)
    /// - A task-focused system prompt with the task injected
    ///
    /// The sub-agent responds in character (inherits soul) but is focused
    /// on a specific task. Per user decision: sub-agents inherit parent bot's
    /// personality and always use the same model.
    pub fn child_for_task(&self, task: &str, depth: u8) -> Self {
        let system_prompt = SystemPromptBuilder::build_for_sub_agent(
            &self.agent_config,
            &self.soul_content,
            &self.identity_content,
            task,
            depth,
        );

        Self {
            agent_config: self.agent_config.clone(),
            soul_content: self.soul_content.clone(),
            identity_content: self.identity_content.clone(),
            user_content: String::new(),
            memories: Vec::new(),
            recalled_memories: Vec::new(),
            conversation_history: Vec::new(),
            token_budget: self.token_budget.clone(),
            system_prompt,
            verbose: self.verbose,
        }
    }

    /// Rough estimate of tokens used by conversation history.
    ///
    /// Uses 1 token ~ 4 characters as a conservative heuristic.
    /// Exact counting would require a tokenizer or API call.
    fn estimate_conversation_tokens(&self) -> u32 {
        let total_chars: usize = self
            .conversation_history
            .iter()
            .map(|m| m.content.len())
            .sum();
        // Conservative estimate: ~4 chars per token
        (total_chars / 4) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_config() -> AgentConfig {
        AgentConfig {
            bot_id: Uuid::now_v7(),
            bot_name: "Luna".to_string(),
            bot_slug: "luna".to_string(),
            bot_emoji: Some("🌙".to_string()),
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }

    #[test]
    fn test_context_new_builds_system_prompt() {
        let ctx = AgentContext::new(
            test_config(),
            "I am creative.".to_string(),
            "Name: Luna".to_string(),
            "Be concise.".to_string(),
            vec![],
            TokenBudget::new(200_000),
        );

        assert!(ctx.system_prompt.contains("<soul>"));
        assert!(ctx.system_prompt.contains("I am creative."));
        assert!(ctx.system_prompt.contains("<identity>"));
        assert!(ctx.system_prompt.contains("<instructions>"));
    }

    #[test]
    fn test_add_messages() {
        let mut ctx = AgentContext::new(
            test_config(),
            String::new(),
            String::new(),
            String::new(),
            vec![],
            TokenBudget::new(200_000),
        );

        ctx.add_user_message("Hello!".to_string());
        ctx.add_assistant_message("Hi there!".to_string());

        assert_eq!(ctx.conversation_history.len(), 2);
        assert_eq!(ctx.conversation_history[0].role, MessageRole::User);
        assert_eq!(ctx.conversation_history[0].content, "Hello!");
        assert_eq!(ctx.conversation_history[1].role, MessageRole::Assistant);
        assert_eq!(ctx.conversation_history[1].content, "Hi there!");
    }

    #[test]
    fn test_build_messages() {
        let mut ctx = AgentContext::new(
            test_config(),
            String::new(),
            String::new(),
            String::new(),
            vec![],
            TokenBudget::new(200_000),
        );

        ctx.add_user_message("Hello!".to_string());
        let messages = ctx.build_messages();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, MessageRole::User);
    }

    #[test]
    fn test_should_summarize_false_when_small() {
        let mut ctx = AgentContext::new(
            test_config(),
            String::new(),
            String::new(),
            String::new(),
            vec![],
            TokenBudget::new(200_000),
        );

        ctx.add_user_message("Short message".to_string());
        assert!(!ctx.should_summarize());
    }

    #[test]
    fn test_should_summarize_true_when_large() {
        let mut ctx = AgentContext::new(
            test_config(),
            String::new(),
            String::new(),
            String::new(),
            vec![],
            // Small budget: conversation_budget = 700 tokens (70% of 1000)
            // 80% threshold = 560 tokens
            TokenBudget::new(1_000),
        );

        // Add enough text to exceed 560 tokens * 4 chars = 2240 chars
        let long_msg = "x".repeat(3000);
        ctx.add_user_message(long_msg);
        assert!(ctx.should_summarize());
    }

    #[test]
    fn test_child_for_task_has_empty_conversation_history() {
        let mut ctx = AgentContext::new(
            test_config(),
            "I am creative.".to_string(),
            "Name: Luna".to_string(),
            "Be concise.".to_string(),
            vec![],
            TokenBudget::new(200_000),
        );
        ctx.add_user_message("Hello!".to_string());
        ctx.add_assistant_message("Hi there!".to_string());

        let child = ctx.child_for_task("Research quantum computing", 1);

        assert!(child.conversation_history.is_empty());
        assert!(child.memories.is_empty());
        assert!(child.recalled_memories.is_empty());
        assert!(child.user_content.is_empty());
    }

    #[test]
    fn test_child_for_task_inherits_soul_and_config() {
        let ctx = AgentContext::new(
            test_config(),
            "I am creative.".to_string(),
            "Name: Luna".to_string(),
            "Be concise.".to_string(),
            vec![],
            TokenBudget::new(200_000),
        );

        let child = ctx.child_for_task("Research quantum computing", 1);

        assert_eq!(child.soul_content, "I am creative.");
        assert_eq!(child.identity_content, "Name: Luna");
        assert_eq!(child.agent_config.bot_name, "Luna");
        assert_eq!(child.agent_config.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_child_for_task_has_task_focused_system_prompt() {
        let ctx = AgentContext::new(
            test_config(),
            "I am creative.".to_string(),
            "Name: Luna".to_string(),
            String::new(),
            vec![],
            TokenBudget::new(200_000),
        );

        let child = ctx.child_for_task("Research quantum computing", 1);

        assert!(child.system_prompt.contains("<task>"));
        assert!(child.system_prompt.contains("Research quantum computing"));
        assert!(child.system_prompt.contains("<sub_agent_instructions>"));
        assert!(child.system_prompt.contains("<soul>"));
        // Does not include user_context (sub-agents don't get USER.md)
        assert!(!child.system_prompt.contains("<user_context>"));
    }
}
