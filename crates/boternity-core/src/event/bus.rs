//! Broadcast event bus for distributing `AgentEvent` to multiple subscribers.
//!
//! Built on `tokio::sync::broadcast`, the `EventBus` supports multiple
//! concurrent subscribers. Publishing with no active subscribers is a no-op.

use boternity_types::event::AgentEvent;
use tokio::sync::broadcast;

/// Multi-consumer event bus for agent hierarchy events.
///
/// Wraps a `tokio::sync::broadcast` channel. Cloning the bus clones the
/// sender, allowing multiple producers and consumers.
pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    ///
    /// A capacity of 1024 is recommended for typical agent hierarchies.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Create a new subscriber that will receive all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    /// Publish an event to all current subscribers.
    ///
    /// If there are no subscribers, the event is silently dropped.
    pub fn publish(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }

    /// Access the underlying broadcast sender.
    pub fn sender(&self) -> &broadcast::Sender<AgentEvent> {
        &self.sender
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("receiver_count", &self.sender.receiver_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample_event() -> AgentEvent {
        AgentEvent::AgentSpawned {
            agent_id: Uuid::now_v7(),
            parent_id: None,
            task_description: "test task".to_string(),
            depth: 0,
            index: 0,
            total: 1,
        }
    }

    #[tokio::test]
    async fn publish_and_subscribe_delivers_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(sample_event());

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, AgentEvent::AgentSpawned { depth: 0, .. }));
    }

    #[tokio::test]
    async fn multiple_subscribers_each_receive_event() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(sample_event());

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert!(matches!(e1, AgentEvent::AgentSpawned { .. }));
        assert!(matches!(e2, AgentEvent::AgentSpawned { .. }));
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_does_not_panic() {
        let bus = EventBus::new(16);
        // No subscribers -- should not panic
        bus.publish(sample_event());
        bus.publish(sample_event());
    }

    #[tokio::test]
    async fn lagged_receiver_handles_gracefully() {
        let bus = EventBus::new(4); // Small capacity to trigger lag
        let mut rx = bus.subscribe();

        // Publish more events than the channel capacity
        for i in 0..10 {
            bus.publish(AgentEvent::AgentTextDelta {
                agent_id: Uuid::now_v7(),
                text: format!("delta {i}"),
            });
        }

        // Receiver may get a Lagged error -- should not panic
        let result = rx.try_recv();
        // Either we get a value or a Lagged error; both are acceptable
        match result {
            Ok(_) => {} // got a message
            Err(broadcast::error::TryRecvError::Lagged(_)) => {} // expected lag
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn clone_shares_channel() {
        let bus = EventBus::new(16);
        let bus2 = bus.clone();
        let mut rx = bus.subscribe();

        // Publish via clone, receive via original's subscriber
        bus2.publish(sample_event());

        let result = rx.try_recv();
        assert!(result.is_ok());
    }

    #[test]
    fn debug_impl() {
        let bus = EventBus::new(16);
        let _rx = bus.subscribe();
        let debug = format!("{bus:?}");
        assert!(debug.contains("EventBus"));
        assert!(debug.contains("receiver_count"));
    }
}
