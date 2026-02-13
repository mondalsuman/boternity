//! WebSocket handler for real-time agent event streaming and bidirectional commands.
//!
//! The `/ws/events` endpoint upgrades an HTTP connection to a WebSocket.
//! Once connected, the handler:
//!
//! - **Forwards events:** Subscribes to the [`EventBus`] on [`AppState`] and
//!   pushes every [`AgentEvent`] to the client as a JSON text frame.
//! - **Receives commands:** Parses incoming text frames as [`WsCommand`] and
//!   processes cancellation, budget decisions, and pings.
//!
//! Lagged receivers (when the client is too slow to keep up) are handled
//! gracefully: the handler logs a warning and continues receiving.
//!
//! Disconnecting a WebSocket does **not** cancel running agents. The user
//! must explicitly send a `CancelAgent` command. This allows reconnection
//! without disrupting in-flight work.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::state::AppState;

/// Incoming command from a WebSocket client.
///
/// Clients send JSON-encoded text frames matching one of these variants.
/// Unknown or malformed messages are logged and ignored.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsCommand {
    /// Request cancellation of a specific agent.
    CancelAgent { agent_id: String },
    /// Continue execution after a budget warning pause.
    BudgetContinue { request_id: String },
    /// Stop execution after a budget warning pause.
    BudgetStop { request_id: String },
    /// Keep-alive ping. Server responds with `{"type":"pong"}`.
    Ping,
}

/// Upgrade an HTTP request to a WebSocket connection for agent events.
///
/// This is mounted at `/ws/events` in the router.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Core WebSocket connection handler.
///
/// Uses `tokio::select!` to multiplex between receiving events from the
/// [`EventBus`] and incoming WebSocket messages from the client. This
/// approach keeps both sender and receiver in a single task, enabling
/// bidirectional communication (e.g., responding to `Ping` with a pong).
async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Subscribe to the event bus for agent lifecycle events.
    let mut event_rx = state.event_bus.subscribe();

    let budget_responses = state.budget_responses.clone();
    let agent_cancellations = state.agent_cancellations.clone();

    loop {
        tokio::select! {
            // --- Branch 1: Forward EventBus events to WebSocket client ---
            event_result = event_rx.recv() => {
                match event_result {
                    Ok(event) => {
                        match serde_json::to_string(&event) {
                            Ok(json) => {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    // Client disconnected
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!("Failed to serialize AgentEvent: {err}");
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            skipped = n,
                            "WebSocket subscriber lagged, skipping {n} events"
                        );
                        // Continue receiving -- the client will miss some events
                        // but will catch up with the next ones.
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // EventBus sender was dropped (server shutting down)
                        break;
                    }
                }
            }

            // --- Branch 2: Process commands from WebSocket client ---
            msg_result = ws_receiver.next() => {
                match msg_result {
                    Some(Ok(Message::Text(text))) => {
                        process_command(
                            &text,
                            &mut ws_sender,
                            &budget_responses,
                            &agent_cancellations,
                        ).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        // Client disconnected
                        break;
                    }
                    Some(Err(err)) => {
                        tracing::debug!("WebSocket receive error: {err}");
                        break;
                    }
                    // Ignore binary, ping, pong protocol frames (handled by axum/tungstenite)
                    Some(Ok(_)) => {}
                }
            }
        }
    }

    tracing::debug!("WebSocket connection closed");
}

/// Parse and process a single command from the WebSocket client.
async fn process_command(
    text: &str,
    ws_sender: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    budget_responses: &dashmap::DashMap<Uuid, tokio::sync::oneshot::Sender<bool>>,
    agent_cancellations: &dashmap::DashMap<Uuid, tokio_util::sync::CancellationToken>,
) {
    let cmd: WsCommand = match serde_json::from_str(text) {
        Ok(cmd) => cmd,
        Err(err) => {
            tracing::warn!(
                raw = %text,
                error = %err,
                "Ignoring malformed WebSocket command"
            );
            return;
        }
    };

    match cmd {
        WsCommand::CancelAgent { agent_id } => {
            match Uuid::parse_str(&agent_id) {
                Ok(id) => {
                    if let Some(token) = agent_cancellations.get(&id) {
                        token.cancel();
                        tracing::info!(%agent_id, "Agent cancellation requested via WebSocket");
                    } else {
                        tracing::warn!(%agent_id, "CancelAgent: no active agent with this ID");
                    }
                }
                Err(err) => {
                    tracing::warn!(%agent_id, error = %err, "CancelAgent: invalid UUID");
                }
            }
        }
        WsCommand::BudgetContinue { request_id } => {
            match Uuid::parse_str(&request_id) {
                Ok(id) => {
                    if let Some((_, sender)) = budget_responses.remove(&id) {
                        let _ = sender.send(true);
                        tracing::info!(%request_id, "Budget continue via WebSocket");
                    } else {
                        tracing::warn!(%request_id, "BudgetContinue: no pending budget prompt");
                    }
                }
                Err(err) => {
                    tracing::warn!(%request_id, error = %err, "BudgetContinue: invalid UUID");
                }
            }
        }
        WsCommand::BudgetStop { request_id } => {
            match Uuid::parse_str(&request_id) {
                Ok(id) => {
                    if let Some((_, sender)) = budget_responses.remove(&id) {
                        let _ = sender.send(false);
                        tracing::info!(%request_id, "Budget stop via WebSocket");
                    } else {
                        tracing::warn!(%request_id, "BudgetStop: no pending budget prompt");
                    }
                }
                Err(err) => {
                    tracing::warn!(%request_id, error = %err, "BudgetStop: invalid UUID");
                }
            }
        }
        WsCommand::Ping => {
            let pong = r#"{"type":"pong"}"#;
            if ws_sender.send(Message::Text(pong.into())).await.is_err() {
                tracing::debug!("Failed to send pong (client disconnecting)");
            }
        }
    }
}
