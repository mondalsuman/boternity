//! Agent configuration types for Boternity.
//!
//! AgentConfig bundles the identity and LLM settings needed to run
//! a bot as a conversational agent.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for running a bot as a conversational agent.
///
/// Combines bot identity fields (name, slug, emoji) with LLM parameters
/// (model, temperature, max_tokens). Built from a `Bot` + `Identity` at
/// session start time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub bot_id: Uuid,
    pub bot_name: String,
    pub bot_slug: String,
    pub bot_emoji: Option<String>,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
}

/// Mode for spawning sub-agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnMode {
    /// Execute tasks one after another.
    Sequential,
    /// Execute tasks concurrently.
    Parallel,
}

/// Parsed spawn instruction from an LLM response.
///
/// Contains the spawn mode and a list of task descriptions
/// for sub-agents to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnInstruction {
    pub mode: SpawnMode,
    pub tasks: Vec<String>,
}

/// Status of a sub-agent during execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Result of a sub-agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    pub agent_id: Uuid,
    pub task: String,
    pub status: AgentStatus,
    pub response: Option<String>,
    pub error: Option<String>,
    pub tokens_used: u32,
    pub duration_ms: u64,
}

/// Node in the agent execution tree (for UI rendering).
///
/// Represents a single agent in the hierarchy with its children,
/// forming a tree that can be rendered in the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub agent_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub task: String,
    pub depth: u8,
    pub status: AgentStatus,
    pub tokens_used: u32,
    pub duration_ms: u64,
    pub children: Vec<AgentNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_serialize() {
        let config = AgentConfig {
            bot_id: Uuid::now_v7(),
            bot_name: "Luna".to_string(),
            bot_slug: "luna".to_string(),
            bot_emoji: Some("🌙".to_string()),
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"bot_name\":\"Luna\""));
        assert!(json.contains("\"temperature\":0.7"));
    }

    #[test]
    fn test_spawn_mode_serde_roundtrip() {
        for mode in [SpawnMode::Sequential, SpawnMode::Parallel] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: SpawnMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn test_spawn_mode_serde_rename() {
        let json = serde_json::to_string(&SpawnMode::Sequential).unwrap();
        assert_eq!(json, "\"sequential\"");
        let json = serde_json::to_string(&SpawnMode::Parallel).unwrap();
        assert_eq!(json, "\"parallel\"");
    }

    #[test]
    fn test_agent_status_serde_roundtrip() {
        for status in [
            AgentStatus::Pending,
            AgentStatus::Running,
            AgentStatus::Completed,
            AgentStatus::Failed,
            AgentStatus::Cancelled,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: AgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_agent_status_serde_rename() {
        assert_eq!(
            serde_json::to_string(&AgentStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn test_spawn_instruction_serialize() {
        let instruction = SpawnInstruction {
            mode: SpawnMode::Parallel,
            tasks: vec![
                "Research topic A".to_string(),
                "Research topic B".to_string(),
            ],
        };
        let json = serde_json::to_string(&instruction).unwrap();
        assert!(json.contains("\"parallel\""));
        assert!(json.contains("Research topic A"));
        let parsed: SpawnInstruction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mode, SpawnMode::Parallel);
        assert_eq!(parsed.tasks.len(), 2);
    }

    #[test]
    fn test_sub_agent_result_construction() {
        let result = SubAgentResult {
            agent_id: Uuid::now_v7(),
            task: "Summarize document".to_string(),
            status: AgentStatus::Completed,
            response: Some("Summary here".to_string()),
            error: None,
            tokens_used: 1200,
            duration_ms: 2500,
        };
        assert_eq!(result.status, AgentStatus::Completed);
        assert_eq!(result.tokens_used, 1200);
        assert!(result.error.is_none());
        assert!(result.response.is_some());

        let json = serde_json::to_string(&result).unwrap();
        let parsed: SubAgentResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task, "Summarize document");
        assert_eq!(parsed.duration_ms, 2500);
    }

    #[test]
    fn test_sub_agent_result_failed() {
        let result = SubAgentResult {
            agent_id: Uuid::now_v7(),
            task: "Failing task".to_string(),
            status: AgentStatus::Failed,
            response: None,
            error: Some("timeout".to_string()),
            tokens_used: 500,
            duration_ms: 10000,
        };
        assert_eq!(result.status, AgentStatus::Failed);
        assert!(result.response.is_none());
        assert_eq!(result.error.as_deref(), Some("timeout"));
    }

    #[test]
    fn test_agent_node_tree() {
        let child = AgentNode {
            agent_id: Uuid::now_v7(),
            parent_id: Some(Uuid::now_v7()),
            task: "Sub-task".to_string(),
            depth: 1,
            status: AgentStatus::Completed,
            tokens_used: 500,
            duration_ms: 1000,
            children: vec![],
        };
        let root = AgentNode {
            agent_id: Uuid::now_v7(),
            parent_id: None,
            task: "Root task".to_string(),
            depth: 0,
            status: AgentStatus::Running,
            tokens_used: 1500,
            duration_ms: 3000,
            children: vec![child],
        };
        let json = serde_json::to_string(&root).unwrap();
        let parsed: AgentNode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.children.len(), 1);
        assert_eq!(parsed.depth, 0);
        assert!(parsed.parent_id.is_none());
        assert_eq!(parsed.children[0].depth, 1);
    }
}
