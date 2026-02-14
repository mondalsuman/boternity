//! LLM-powered builder agent implementation.
//!
//! `LlmBuilderAgent` implements the `BuilderAgent` trait using Claude's
//! structured output capability. Each turn sends the accumulated builder
//! state via the Forge system prompt and receives a JSON-schema-constrained
//! `BuilderTurn` response.
//!
//! The agent queries builder memory on start/resume to pass recalled sessions
//! into the Forge prompt for cross-session suggestion continuity.

use uuid::Uuid;

use boternity_core::builder::agent::{BuilderAgent, BuilderError};
use boternity_core::builder::defaults::classify_purpose;
use boternity_core::builder::memory::{BuilderMemoryEntry, BuilderMemoryStore};
use boternity_core::builder::prompt::{
    BuilderMode, RecalledBuilderMemory, build_forge_system_prompt,
};
use boternity_core::builder::state::{BuilderStateExt, new_builder_state};
use boternity_core::llm::box_provider::BoxLlmProvider;
use boternity_types::builder::{
    BuilderAnswer, BuilderConfig, BuilderState, BuilderTurn, add_additional_properties_false,
};
use boternity_types::llm::{
    CompletionRequest, Message, MessageRole, OutputConfig, OutputFormat, OutputJsonSchema,
};

// ---------------------------------------------------------------------------
// LlmBuilderAgent
// ---------------------------------------------------------------------------

/// LLM-powered builder agent that drives the interactive bot creation flow.
///
/// Uses Claude's structured output (`output_config`) to constrain responses
/// to the `BuilderTurn` JSON schema. Queries builder memory on start/resume
/// to provide cross-session suggestion continuity.
///
/// Generic over `M: BuilderMemoryStore` to allow injecting the SQLite memory
/// store from AppState or `()` for tests without a database.
pub struct LlmBuilderAgent<M: BuilderMemoryStore> {
    provider: BoxLlmProvider,
    memory_store: Option<M>,
    /// Model identifier for the LLM calls.
    model: String,
}

impl<M: BuilderMemoryStore> LlmBuilderAgent<M> {
    /// Create a new LlmBuilderAgent.
    ///
    /// # Arguments
    /// * `provider` - The LLM provider for structured output calls
    /// * `memory_store` - Optional builder memory store for past session recall
    /// * `model` - Model identifier to use for LLM calls
    pub fn new(provider: BoxLlmProvider, memory_store: Option<M>, model: String) -> Self {
        Self {
            provider,
            memory_store,
            model,
        }
    }

    /// Generate the JSON schema for `BuilderTurn` with `additionalProperties: false`.
    fn builder_turn_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(BuilderTurn);
        let mut schema_value = serde_json::to_value(schema)
            .expect("BuilderTurn schema serialization should not fail");
        add_additional_properties_false(&mut schema_value);
        schema_value
    }

    /// Build an `OutputConfig` for structured output with the BuilderTurn schema.
    fn output_config() -> OutputConfig {
        OutputConfig {
            format: OutputFormat {
                type_field: "json_schema".to_string(),
                json_schema: OutputJsonSchema {
                    name: "BuilderTurn".to_string(),
                    schema: Self::builder_turn_schema(),
                    strict: Some(true),
                },
            },
        }
    }

    /// Call the LLM with the given system prompt and user message,
    /// parse the response as a `BuilderTurn`.
    async fn call_llm(
        &self,
        system_prompt: String,
        user_message: String,
    ) -> Result<BuilderTurn, BuilderError> {
        let request = CompletionRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: MessageRole::User,
                content: user_message,
            }],
            system: Some(system_prompt),
            max_tokens: 2048,
            temperature: Some(0.7),
            stream: false,
            stop_sequences: None,
            output_config: Some(Self::output_config()),
        };

        let response = self
            .provider
            .complete(&request)
            .await
            .map_err(|e| BuilderError::LlmError(e.to_string()))?;

        serde_json::from_str::<BuilderTurn>(&response.content).map_err(|e| {
            BuilderError::ParseError(format!(
                "failed to parse BuilderTurn: {e}\nraw content: {}",
                response.content
            ))
        })
    }

    /// Query builder memory for past sessions matching the given purpose category.
    ///
    /// Returns up to 3 recalled sessions converted to `RecalledBuilderMemory`.
    /// Returns an empty vec if memory_store is None or the query fails.
    async fn recall_memories(
        &self,
        category: &boternity_types::builder::PurposeCategory,
    ) -> Vec<RecalledBuilderMemory> {
        match &self.memory_store {
            Some(store) => store
                .recall_by_category(category, 3)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(Self::entry_to_recalled)
                .collect(),
            None => vec![],
        }
    }

    /// Convert a `BuilderMemoryEntry` to a `RecalledBuilderMemory`.
    fn entry_to_recalled(entry: BuilderMemoryEntry) -> RecalledBuilderMemory {
        RecalledBuilderMemory {
            initial_description: entry.initial_description,
            chosen_tone: entry.chosen_tone,
            chosen_model: entry.chosen_model,
            chosen_skills: entry.chosen_skills,
            bot_slug: entry.bot_slug,
        }
    }

    /// Format a `BuilderAnswer` as a human-readable string for the LLM.
    fn format_answer(answer: &BuilderAnswer) -> String {
        match answer {
            BuilderAnswer::OptionIndex(idx) => format!("Selected option {idx}"),
            BuilderAnswer::FreeText(text) => text.clone(),
            BuilderAnswer::Confirm(yes) => {
                if *yes {
                    "Yes, confirmed.".to_string()
                } else {
                    "No, I'd like to make changes.".to_string()
                }
            }
            BuilderAnswer::Back => "Go back to the previous step.".to_string(),
        }
    }
}

impl<M: BuilderMemoryStore + 'static> BuilderAgent for LlmBuilderAgent<M> {
    async fn start(
        &self,
        session_id: Uuid,
        initial_description: &str,
    ) -> Result<BuilderTurn, BuilderError> {
        // 1. Create new state
        let mut state =
            new_builder_state(session_id, initial_description.to_string());

        // 2. Classify purpose
        let category = classify_purpose(initial_description);
        state.purpose_category = Some(category.clone());

        // 3. Query memory for past sessions in same category
        let recalled = self.recall_memories(&category).await;

        // 4. Build Forge system prompt with recalled memories
        let system_prompt =
            build_forge_system_prompt(&state, &BuilderMode::NewBot, &recalled);

        // 5. Call LLM
        let turn = self.call_llm(system_prompt, initial_description.to_string()).await?;

        // Store recalled memories for reuse (interior mutability not needed here
        // since start() is called once per session; next_turn will use the state
        // passed by the caller who can cache recalled_memories externally).
        // Note: We can't mutate &self here, but the caller (CLI/web) will hold
        // the recalled memories in the state or pass them to next_turn. For the
        // current trait design, we query once at start and the prompt builder
        // includes the accumulated context.

        Ok(turn)
    }

    async fn next_turn(
        &self,
        state: &mut BuilderState,
        answer: BuilderAnswer,
    ) -> Result<BuilderTurn, BuilderError> {
        // Handle Back navigation
        if matches!(answer, BuilderAnswer::Back) {
            let went_back = state.go_back();
            if went_back.is_none() {
                return Err(BuilderError::StateError(
                    "cannot go back from the first phase".to_string(),
                ));
            }

            // Rebuild prompt and ask for new input at the restored phase
            let recalled = self.recall_memories(
                state.purpose_category.as_ref().unwrap_or(
                    &boternity_types::builder::PurposeCategory::Custom("unknown".to_string()),
                ),
            ).await;
            let system_prompt =
                build_forge_system_prompt(state, &BuilderMode::NewBot, &recalled);
            return self
                .call_llm(
                    system_prompt,
                    "The user went back. Ask the appropriate question for this phase again.".to_string(),
                )
                .await;
        }

        // Format the answer as a string
        let answer_text = Self::format_answer(&answer);

        // Record the exchange (the question was from the previous turn)
        // We record the answer with a placeholder question since the actual
        // question text was in the previous BuilderTurn response
        state.record_exchange("(previous question)".to_string(), answer_text.clone());

        // Rebuild Forge system prompt with updated state
        let recalled = self.recall_memories(
            state.purpose_category.as_ref().unwrap_or(
                &boternity_types::builder::PurposeCategory::Custom("unknown".to_string()),
            ),
        ).await;
        let system_prompt =
            build_forge_system_prompt(state, &BuilderMode::NewBot, &recalled);

        // Call LLM with the user's answer
        let turn = self.call_llm(system_prompt, answer_text).await?;

        // If the turn indicates a new phase, advance state
        match &turn {
            BuilderTurn::AskQuestion { phase, .. } | BuilderTurn::ShowPreview { phase, .. } => {
                if *phase != state.phase {
                    state.advance_phase(phase.clone());
                }
            }
            _ => {}
        }

        Ok(turn)
    }

    async fn resume(
        &self,
        state: &BuilderState,
    ) -> Result<BuilderTurn, BuilderError> {
        // Query memory again for recalled_memories
        let recalled = self.recall_memories(
            state.purpose_category.as_ref().unwrap_or(
                &boternity_types::builder::PurposeCategory::Custom("unknown".to_string()),
            ),
        ).await;

        // Build prompt from saved state with resume instruction
        let system_prompt =
            build_forge_system_prompt(state, &BuilderMode::NewBot, &recalled);

        let resume_message = format!(
            "The user is resuming a previous builder session. Their progress so far \
             is in the accumulated context. Continue from where they left off.\n\n\
             I'd like to continue where I left off building: {}",
            state.initial_description
        );

        self.call_llm(system_prompt, resume_message).await
    }

    async fn reconfigure(
        &self,
        state: &mut BuilderState,
        current_config: BuilderConfig,
    ) -> Result<BuilderTurn, BuilderError> {
        // Populate state.config from current_config
        state.config.name = Some(current_config.name.clone());
        state.config.description = Some(current_config.description.clone());
        state.config.category = Some(current_config.category.clone());
        state.config.tags = Some(current_config.tags.clone());
        state.config.tone = Some(current_config.personality.tone.clone());
        state.config.traits = Some(current_config.personality.traits.clone());
        state.config.purpose = Some(current_config.personality.purpose.clone());
        state.config.boundaries = current_config.personality.boundaries.clone();
        state.config.model = Some(current_config.model_config.model.clone());
        state.config.temperature = Some(current_config.model_config.temperature);
        state.config.max_tokens = Some(current_config.model_config.max_tokens);
        state.config.skills = current_config.skills;

        // Build prompt with ReconfigureBot mode
        let recalled = self.recall_memories(
            state.purpose_category.as_ref().unwrap_or(
                &boternity_types::builder::PurposeCategory::Custom("unknown".to_string()),
            ),
        ).await;
        let system_prompt =
            build_forge_system_prompt(state, &BuilderMode::ReconfigureBot, &recalled);

        self.call_llm(
            system_prompt,
            format!(
                "I want to reconfigure my bot \"{}\". Show me the current configuration and ask what I'd like to adjust.",
                current_config.name
            ),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;

    use futures_util::Stream;

    use boternity_core::llm::provider::LlmProvider;
    use boternity_types::llm::{
        CompletionResponse, LlmError, ProviderCapabilities, StopReason, StreamEvent,
        TokenCount, Usage,
    };

    // -----------------------------------------------------------------------
    // MockLlmProvider
    // -----------------------------------------------------------------------

    /// A minimal mock LLM provider that returns a static response.
    struct MockLlmProvider {
        response_content: String,
    }

    impl MockLlmProvider {
        fn with_response(content: &str) -> Self {
            Self {
                response_content: content.to_string(),
            }
        }
    }

    impl LlmProvider for MockLlmProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn capabilities(&self) -> &ProviderCapabilities {
            &ProviderCapabilities {
                streaming: false,
                tool_calling: false,
                vision: false,
                extended_thinking: false,
                max_context_tokens: 200_000,
                max_output_tokens: 4096,
            }
        }

        async fn complete(
            &self,
            _request: &CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            Ok(CompletionResponse {
                id: "msg_mock_123".to_string(),
                content: self.response_content.clone(),
                model: "mock-model".to_string(),
                stop_reason: StopReason::EndTurn,
                usage: Usage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                },
            })
        }

        fn stream(
            &self,
            _request: CompletionRequest,
        ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
            Box::pin(futures_util::stream::empty())
        }

        async fn count_tokens(
            &self,
            _request: &CompletionRequest,
        ) -> Result<TokenCount, LlmError> {
            Ok(TokenCount { input_tokens: 100 })
        }
    }

    // -----------------------------------------------------------------------
    // NullMemoryStore (implements BuilderMemoryStore with no-ops)
    // -----------------------------------------------------------------------

    struct NullMemoryStore;

    impl BuilderMemoryStore for NullMemoryStore {
        async fn record_session(
            &self,
            _memory: BuilderMemoryEntry,
        ) -> Result<(), boternity_types::error::RepositoryError> {
            Ok(())
        }

        async fn recall_by_category(
            &self,
            _category: &boternity_types::builder::PurposeCategory,
            _limit: usize,
        ) -> Result<Vec<BuilderMemoryEntry>, boternity_types::error::RepositoryError> {
            Ok(vec![])
        }

        async fn recall_recent(
            &self,
            _limit: usize,
        ) -> Result<Vec<BuilderMemoryEntry>, boternity_types::error::RepositoryError> {
            Ok(vec![])
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    fn mock_ask_question_json() -> String {
        serde_json::to_string(&BuilderTurn::AskQuestion {
            phase: boternity_types::builder::BuilderPhase::Basics,
            question: "What should your bot be called?".to_string(),
            options: vec![
                boternity_types::builder::QuestionOption {
                    id: "1".to_string(),
                    label: "CodeHelper".to_string(),
                    description: Some("A coding assistant name".to_string()),
                },
                boternity_types::builder::QuestionOption {
                    id: "other".to_string(),
                    label: "Other".to_string(),
                    description: Some("Type your own name".to_string()),
                },
            ],
            allow_free_text: true,
            phase_label: Some("Setting up basics...".to_string()),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn test_start_returns_ask_question() {
        let mock_json = mock_ask_question_json();
        let provider = BoxLlmProvider::new(MockLlmProvider::with_response(&mock_json));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, None, "mock-model".to_string());

        let turn = agent
            .start(Uuid::now_v7(), "I want a coding assistant")
            .await
            .unwrap();

        match turn {
            BuilderTurn::AskQuestion { question, options, allow_free_text, .. } => {
                assert_eq!(question, "What should your bot be called?");
                assert!(!options.is_empty());
                assert!(allow_free_text);
            }
            other => panic!("expected AskQuestion, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_start_with_memory_store() {
        let mock_json = mock_ask_question_json();
        let provider = BoxLlmProvider::new(MockLlmProvider::with_response(&mock_json));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, Some(NullMemoryStore), "mock-model".to_string());

        let turn = agent
            .start(Uuid::now_v7(), "I want a coding assistant")
            .await
            .unwrap();

        assert!(matches!(turn, BuilderTurn::AskQuestion { .. }));
    }

    #[tokio::test]
    async fn test_next_turn_advances_phase() {
        let personality_json = serde_json::to_string(&BuilderTurn::AskQuestion {
            phase: boternity_types::builder::BuilderPhase::Personality,
            question: "What tone should your bot use?".to_string(),
            options: vec![boternity_types::builder::QuestionOption {
                id: "1".to_string(),
                label: "Formal".to_string(),
                description: Some("Professional tone".to_string()),
            }],
            allow_free_text: true,
            phase_label: Some("Defining personality...".to_string()),
        })
        .unwrap();

        let provider = BoxLlmProvider::new(MockLlmProvider::with_response(&personality_json));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, None, "mock-model".to_string());

        let mut state = new_builder_state(Uuid::now_v7(), "A coding assistant".to_string());
        state.purpose_category = Some(boternity_types::builder::PurposeCategory::Coding);

        let turn = agent
            .next_turn(&mut state, BuilderAnswer::FreeText("CodeHelper".to_string()))
            .await
            .unwrap();

        assert!(matches!(turn, BuilderTurn::AskQuestion { .. }));
        assert_eq!(state.phase, boternity_types::builder::BuilderPhase::Personality);
    }

    #[tokio::test]
    async fn test_back_from_first_phase_errors() {
        let mock_json = mock_ask_question_json();
        let provider = BoxLlmProvider::new(MockLlmProvider::with_response(&mock_json));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, None, "mock-model".to_string());

        let mut state = new_builder_state(Uuid::now_v7(), "A coding assistant".to_string());
        let result = agent.next_turn(&mut state, BuilderAnswer::Back).await;

        assert!(result.is_err());
        match result {
            Err(BuilderError::StateError(msg)) => {
                assert!(msg.contains("cannot go back"));
            }
            other => panic!("expected StateError, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_resume_produces_turn() {
        let mock_json = mock_ask_question_json();
        let provider = BoxLlmProvider::new(MockLlmProvider::with_response(&mock_json));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, None, "mock-model".to_string());

        let state = new_builder_state(Uuid::now_v7(), "A coding assistant".to_string());
        let turn = agent.resume(&state).await.unwrap();

        assert!(matches!(turn, BuilderTurn::AskQuestion { .. }));
    }

    #[tokio::test]
    async fn test_reconfigure_populates_state() {
        let clarify_json = serde_json::to_string(&BuilderTurn::Clarify {
            message: "What would you like to adjust?".to_string(),
        })
        .unwrap();

        let provider = BoxLlmProvider::new(MockLlmProvider::with_response(&clarify_json));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, None, "mock-model".to_string());

        let mut state = new_builder_state(Uuid::now_v7(), "My bot".to_string());
        let config = BuilderConfig {
            name: "Luna".to_string(),
            description: "A creative bot".to_string(),
            category: "creative".to_string(),
            tags: vec!["writing".to_string()],
            personality: boternity_types::builder::PersonalityConfig {
                tone: "expressive".to_string(),
                traits: vec!["creative".to_string()],
                purpose: "Write stories".to_string(),
                boundaries: Some("No violence".to_string()),
            },
            model_config: boternity_types::builder::ModelConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                temperature: 0.9,
                max_tokens: 4096,
            },
            skills: vec![],
        };

        let turn = agent.reconfigure(&mut state, config).await.unwrap();
        assert!(matches!(turn, BuilderTurn::Clarify { .. }));

        // Verify state was populated
        assert_eq!(state.config.name, Some("Luna".to_string()));
        assert_eq!(state.config.tone, Some("expressive".to_string()));
        assert_eq!(state.config.model, Some("claude-sonnet-4-20250514".to_string()));
        assert_eq!(state.config.temperature, Some(0.9));
    }

    #[tokio::test]
    async fn test_parse_error_returns_builder_error() {
        let provider =
            BoxLlmProvider::new(MockLlmProvider::with_response("not valid json at all"));
        let agent: LlmBuilderAgent<NullMemoryStore> =
            LlmBuilderAgent::new(provider, None, "mock-model".to_string());

        let result = agent
            .start(Uuid::now_v7(), "A coding assistant")
            .await;

        assert!(result.is_err());
        match result {
            Err(BuilderError::ParseError(msg)) => {
                assert!(msg.contains("not valid json at all"));
            }
            other => panic!("expected ParseError, got: {other:?}"),
        }
    }

    #[test]
    fn test_builder_turn_schema_has_additional_properties_false() {
        let schema = LlmBuilderAgent::<NullMemoryStore>::builder_turn_schema();
        let json = serde_json::to_string_pretty(&schema).unwrap();
        assert!(json.contains("\"additionalProperties\""));
    }

    #[test]
    fn test_output_config_structure() {
        let config = LlmBuilderAgent::<NullMemoryStore>::output_config();
        assert_eq!(config.format.type_field, "json_schema");
        assert_eq!(config.format.json_schema.name, "BuilderTurn");
        assert_eq!(config.format.json_schema.strict, Some(true));
    }

    #[test]
    fn test_format_answer_variants() {
        assert_eq!(
            LlmBuilderAgent::<NullMemoryStore>::format_answer(&BuilderAnswer::OptionIndex(2)),
            "Selected option 2"
        );
        assert_eq!(
            LlmBuilderAgent::<NullMemoryStore>::format_answer(&BuilderAnswer::FreeText("hello".to_string())),
            "hello"
        );
        assert_eq!(
            LlmBuilderAgent::<NullMemoryStore>::format_answer(&BuilderAnswer::Confirm(true)),
            "Yes, confirmed."
        );
        assert_eq!(
            LlmBuilderAgent::<NullMemoryStore>::format_answer(&BuilderAnswer::Confirm(false)),
            "No, I'd like to make changes."
        );
        assert_eq!(
            LlmBuilderAgent::<NullMemoryStore>::format_answer(&BuilderAnswer::Back),
            "Go back to the previous step."
        );
    }
}
