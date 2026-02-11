//! SSE stream creation and state machine for Anthropic Messages API.
//!
//! Implements the streaming protocol described in the Anthropic docs:
//! 1. `message_start` -- Message object with initial usage
//! 2. Per block: `content_block_start` -> N x `content_block_delta` -> `content_block_stop`
//! 3. `message_delta` -- stop_reason and cumulative usage
//! 4. `message_stop` -- final event
//! 5. `ping` events may appear anywhere (keepalive)
//! 6. `error` events may appear mid-stream
//!
//! Tool use input arrives as partial JSON fragments via `input_json_delta`.
//! These are accumulated per content block index and parsed only after
//! `content_block_stop`.

use std::pin::Pin;

use futures_util::Stream;
use secrecy::SecretString;

use boternity_types::llm::{LlmError, StreamEvent};

use super::types::AnthropicRequest;

/// Create a streaming SSE connection to the Anthropic Messages API.
///
/// Returns a `Stream` of [`StreamEvent`]s that maps Anthropic-specific
/// SSE events to the provider-agnostic stream event enum.
///
/// # Arguments
///
/// * `client` - Shared reqwest HTTP client
/// * `url` - Full API URL (e.g., "https://api.anthropic.com/v1/messages")
/// * `body` - Serialized Anthropic request with `stream: true`
/// * `api_key` - API key wrapped in SecretString
pub fn create_anthropic_stream(
    _client: &reqwest::Client,
    _url: &str,
    _body: AnthropicRequest,
    _api_key: &SecretString,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
    // Full implementation in Task 2 -- this stub allows client.rs to compile.
    Box::pin(futures_util::stream::empty())
}
