//! LLM provider implementations.
//!
//! Contains concrete implementations of the [`LlmProvider`] trait
//! defined in `boternity-core`, starting with Anthropic Claude.
//!
//! Also provides a provider factory ([`create_provider`]) that constructs
//! the right provider from a [`ProviderConfig`], and a connection test
//! function ([`test_provider_connection`]) for verifying provider connectivity.

pub mod anthropic;
pub mod bedrock;
pub mod claude_sub;
pub mod openai_compat;
pub mod pricing;

use secrecy::SecretString;

use boternity_core::llm::box_provider::BoxLlmProvider;
use boternity_types::llm::{
    CompletionRequest, LlmError, Message, MessageRole, ProviderConfig, ProviderType,
};

use self::anthropic::AnthropicProvider;
use self::bedrock::BedrockProvider;
use self::claude_sub::ClaudeSubscriptionProvider;
use self::openai_compat::OpenAiCompatibleProvider;

/// Create a [`BoxLlmProvider`] from a [`ProviderConfig`].
///
/// Matches on the provider type to construct the appropriate concrete
/// provider, resolving the API key from the provided secret value.
///
/// # Arguments
///
/// * `config` - Provider configuration specifying type, model, base URL, etc.
/// * `api_key` - The resolved API key secret value (already fetched from vault)
///
/// # Errors
///
/// Returns an error if the provider type requires an API key but none is provided.
pub fn create_provider(config: &ProviderConfig, api_key: Option<&str>) -> Result<BoxLlmProvider, LlmError> {
    match config.provider_type {
        ProviderType::Anthropic => {
            let key = api_key.ok_or_else(|| LlmError::AuthenticationFailed)?;
            let secret = SecretString::from(key.to_string());
            let provider = AnthropicProvider::new(secret, config.model.clone());
            Ok(BoxLlmProvider::new(provider))
        }
        ProviderType::Bedrock => {
            let key = api_key.ok_or_else(|| LlmError::AuthenticationFailed)?;
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            let secret = SecretString::from(key.to_string());
            let provider = BedrockProvider::new(secret, config.model.clone(), region);
            Ok(BoxLlmProvider::new(provider))
        }
        ProviderType::OpenAiCompatible => {
            let key = api_key.ok_or_else(|| LlmError::AuthenticationFailed)?;

            // Use base_url if specified, otherwise infer from provider name
            let provider = match config.base_url.as_deref() {
                Some(base_url) => {
                    let oai_config = openai_compat::config::OpenAiCompatConfig {
                        provider_name: config.name.clone(),
                        base_url: base_url.to_string(),
                        api_key: key.to_string(),
                        model: config.model.clone(),
                        capabilities: config.capabilities.clone(),
                    };
                    OpenAiCompatibleProvider::new(oai_config)
                }
                None => {
                    // Infer from provider name for well-known providers
                    match config.name.as_str() {
                        "openai" => OpenAiCompatibleProvider::openai(key, &config.model),
                        "gemini" => OpenAiCompatibleProvider::gemini(key, &config.model),
                        "mistral" => OpenAiCompatibleProvider::mistral(key, &config.model),
                        "glm" => OpenAiCompatibleProvider::glm(key, &config.model),
                        _ => {
                            // Default to OpenAI base URL for unknown providers
                            OpenAiCompatibleProvider::openai(key, &config.model)
                        }
                    }
                }
            };
            Ok(BoxLlmProvider::new(provider))
        }
        ProviderType::ClaudeSubscription => {
            ClaudeSubscriptionProvider::print_experimental_warning();
            let provider = ClaudeSubscriptionProvider::new(&config.model);
            Ok(BoxLlmProvider::new(provider))
        }
    }
}

/// Test provider connectivity by sending a minimal completion request.
///
/// Used when a new provider is configured to verify the API key and endpoint
/// are working. Sends a tiny "Hello" message with minimal token budget.
///
/// # Arguments
///
/// * `provider` - The provider to test
///
/// # Errors
///
/// Returns the LLM error if the provider fails to respond.
pub async fn test_provider_connection(provider: &BoxLlmProvider) -> Result<(), LlmError> {
    let request = CompletionRequest {
        model: String::new(), // Provider uses its configured default
        messages: vec![Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        }],
        system: None,
        max_tokens: 10,
        temperature: Some(0.0),
        stream: false,
        stop_sequences: None,
    };
    provider.complete(&request).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::llm::ProviderCapabilities;

    fn default_caps() -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: false,
            extended_thinking: false,
            max_context_tokens: 200_000,
            max_output_tokens: 8_192,
        }
    }

    #[test]
    fn test_create_provider_anthropic() {
        let config = ProviderConfig {
            name: "anthropic".to_string(),
            provider_type: ProviderType::Anthropic,
            api_key_secret_name: Some("ANTHROPIC_API_KEY".to_string()),
            base_url: None,
            model: "claude-sonnet-4-20250514".to_string(),
            priority: 0,
            enabled: true,
            capabilities: default_caps(),
        };
        let provider = create_provider(&config, Some("sk-test-key")).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_create_provider_bedrock() {
        let config = ProviderConfig {
            name: "bedrock".to_string(),
            provider_type: ProviderType::Bedrock,
            api_key_secret_name: Some("BEDROCK_API_KEY".to_string()),
            base_url: None,
            model: "us.anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            priority: 1,
            enabled: true,
            capabilities: default_caps(),
        };
        let provider = create_provider(&config, Some("bedrock-api-key-test")).unwrap();
        assert_eq!(provider.name(), "bedrock");
    }

    #[test]
    fn test_create_provider_openai_compatible_by_name() {
        let config = ProviderConfig {
            name: "openai".to_string(),
            provider_type: ProviderType::OpenAiCompatible,
            api_key_secret_name: Some("OPENAI_API_KEY".to_string()),
            base_url: None,
            model: "gpt-4o".to_string(),
            priority: 2,
            enabled: true,
            capabilities: default_caps(),
        };
        let provider = create_provider(&config, Some("sk-openai-test")).unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_provider_openai_compatible_with_base_url() {
        let config = ProviderConfig {
            name: "custom-provider".to_string(),
            provider_type: ProviderType::OpenAiCompatible,
            api_key_secret_name: Some("CUSTOM_API_KEY".to_string()),
            base_url: Some("https://custom.api.example.com/v1".to_string()),
            model: "custom-model".to_string(),
            priority: 3,
            enabled: true,
            capabilities: default_caps(),
        };
        let provider = create_provider(&config, Some("custom-key")).unwrap();
        assert_eq!(provider.name(), "custom-provider");
    }

    #[test]
    fn test_create_provider_claude_subscription() {
        let config = ProviderConfig {
            name: "claude_subscription".to_string(),
            provider_type: ProviderType::ClaudeSubscription,
            api_key_secret_name: None,
            base_url: None,
            model: "claude-opus-4-20250514".to_string(),
            priority: 10,
            enabled: true,
            capabilities: default_caps(),
        };
        // ClaudeSubscription doesn't need an API key
        let provider = create_provider(&config, None).unwrap();
        assert_eq!(provider.name(), "claude_subscription");
    }

    #[test]
    fn test_create_provider_anthropic_missing_key() {
        let config = ProviderConfig {
            name: "anthropic".to_string(),
            provider_type: ProviderType::Anthropic,
            api_key_secret_name: Some("ANTHROPIC_API_KEY".to_string()),
            base_url: None,
            model: "claude-sonnet-4-20250514".to_string(),
            priority: 0,
            enabled: true,
            capabilities: default_caps(),
        };
        let result = create_provider(&config, None);
        assert!(result.is_err());
        match result {
            Err(LlmError::AuthenticationFailed) => {} // expected
            Err(other) => panic!("Expected AuthenticationFailed, got: {other}"),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[test]
    fn test_create_provider_gemini_by_name() {
        let config = ProviderConfig {
            name: "gemini".to_string(),
            provider_type: ProviderType::OpenAiCompatible,
            api_key_secret_name: Some("GEMINI_API_KEY".to_string()),
            base_url: None,
            model: "gemini-2.5-pro".to_string(),
            priority: 2,
            enabled: true,
            capabilities: default_caps(),
        };
        let provider = create_provider(&config, Some("gemini-key")).unwrap();
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn test_create_provider_mistral_by_name() {
        let config = ProviderConfig {
            name: "mistral".to_string(),
            provider_type: ProviderType::OpenAiCompatible,
            api_key_secret_name: Some("MISTRAL_API_KEY".to_string()),
            base_url: None,
            model: "mistral-large-latest".to_string(),
            priority: 2,
            enabled: true,
            capabilities: default_caps(),
        };
        let provider = create_provider(&config, Some("mistral-key")).unwrap();
        assert_eq!(provider.name(), "mistral");
    }
}
