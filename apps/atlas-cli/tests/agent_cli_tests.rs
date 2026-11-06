//! End-to-end test of the CLI agent with a scripted model: the model asks to
//! read a real file in the workspace, sees its contents, then answers.

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use atlas_cli::agent::{Access, CliAgent};
use platform_contracts::health::HealthState;
use runtime_agent::StopCause;
use serde_json::json;
use shared::error::InferenceError;
use shared::model::{
    CircuitState, ExecutionMetadata, ModelRequest, ModelResponse, ModelTier, StopReason, ToolCall,
};
use tokio::sync::mpsc;

#[derive(Default)]
struct ReadThenAnswer {
    requests: Mutex<Vec<ModelRequest>>,
}

#[async_trait]
impl sdk::model::ModelRuntime for ReadThenAnswer {
    async fn execute_inference(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        let first = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request.clone());
            requests.len() == 1
        };
        let (content, tool_calls, stop_reason) = if first {
            let call = ToolCall {
                id: "c1".into(),
                name: "read_file".into(),
                arguments: json!({ "path": "Cargo.toml" }),
            };
            (String::new(), vec![call], StopReason::ToolUse)
        } else {
            ("This crate is atlas-cli.".to_string(), vec![], StopReason::EndTurn)
        };
        Ok(ModelResponse {
            content,
            tool_calls,
            stop_reason,
            capability: request.capability,
            metadata: ExecutionMetadata {
                provider_id: "scripted".into(),
                model_id: "scripted".into(),
                tier: ModelTier::Local,
                latency_ms: 0,
                tokens_used: 0,
                cache_hit: false,
                fallback_used: false,
                retry_count: 0,
                circuit_state: CircuitState::Closed,
            },
            request_id: request.request_id,
        })
    }

    async fn execute_stream(
        &self,
        _request: ModelRequest,
        _tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        unimplemented!()
    }

    async fn health_check(&self) -> HealthState {
        HealthState::Healthy
    }
}

#[tokio::test]
async fn cli_agent_reads_a_workspace_file_and_answers() {
    let model = Arc::new(ReadThenAnswer::default());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let agent = CliAgent::with_model(root, 5, Access::Read, model.clone(), "http://localhost:11434".into())
        .await
        .unwrap();

    let outcome = agent.ask("What is this crate called?").await.unwrap();
    assert_eq!(outcome.cause, StopCause::EndTurn);
    assert_eq!(outcome.reply.as_deref(), Some("This crate is atlas-cli."));

    let requests = model.requests.lock().unwrap();
    let tool_names: Vec<_> = requests[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tool_names, vec!["list_files", "read_file", "search_text"]);
    assert!(requests[0].system_prompt.as_deref().unwrap().contains("list_files"));

    let result = requests[1].messages.last().unwrap().tool_result_blocks().next().unwrap().clone();
    assert!(!result.is_error, "{}", result.content);
    assert!(result.content.contains("name = \"atlas-cli\""));
}
