use std::path::PathBuf;
use std::time::SystemTime;
use runtime_repository::discovery::{DiscoveryConfig, RepositoryDiscovery};
use runtime_context::AtlasContextRuntime;
use runtime_context::retrieval::planning::RetrievalPlanner;
use runtime_model::{ModelRuntime, ModelRuntimeConfig};
use runtime_memory::api::MemoryRuntime;
use runtime_memory::storage::file_adapter::FileMemoryStore;
use runtime_memory::storage::repository::MemoryRepository;
use runtime_memory::domain::{
    MemoryRecord, MemoryIdentity, MemoryContent, MemoryMetadata,
    MemoryType, MemoryClass, MemoryTraits, MemoryProvenance, ExpirationPolicy,
};
use sdk::context::{ContextRequest, ContextRuntime};
use sdk::context::intent::RetrievalIntent;
use sdk::context::policy::{ContextPolicy, ProviderPriority, ScoringWeights, TokenBudget};
use sdk::model::{ModelRequest, ModelCapability, CapabilityProfile, ExecutionPolicy, Message};

struct DefaultPriority;
impl ProviderPriority for DefaultPriority {
    fn priority(&self, _provider_name: &str) -> u32 {
        1
    }
}

struct DemoPolicy;
impl ContextPolicy for DemoPolicy {
    fn name(&self) -> &str { "DemoPolicy" }
    fn scoring_weights(&self) -> ScoringWeights {
        ScoringWeights {
            relevance: 1.0,
            recency: 1.0,
            structural_distance: 1.0,
            execution_importance: 1.0,
            confidence: 1.0,
            token_cost: 1.0,
        }
    }
    fn optimization_strategy(&self) -> Box<dyn runtime_context::optimization::strategy::OptimizationStrategy> {
        Box::new(runtime_context::optimization::strategy::NoOpStrategy)
    }
    fn provider_priority(&self) -> Box<dyn ProviderPriority> {
        Box::new(DefaultPriority)
    }
    fn token_budget(&self) -> TokenBudget {
        TokenBudget {
            max_tokens: 4096,
            reserved_for_system: 512,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Atlas SDK: End-to-End Autonomous Agent Execution Flow ===");
    let prompt = "Analyze system architecture and model provider fallbacks";

    // 1. Repository Ingestion
    let repo_config = DiscoveryConfig {
        root_path: PathBuf::from("."),
        max_depth: Some(2),
        respect_gitignore: true,
    };
    let discovery = RepositoryDiscovery::new(repo_config);
    let discovery_res = discovery.discover()?;
    println!("[1/5] Ingested {} files. Git: {}", discovery_res.files.len(), discovery_res.is_git);

    // 2. Context Resolution
    let context_runtime = AtlasContextRuntime::new();
    let ctx_req = ContextRequest {
        request_id: "demo-req-1".to_string(),
        intent: RetrievalIntent::ArchitectureReview,
        max_tokens: Some(4096),
        required_entities: vec![],
    };
    let ctx_package = context_runtime.resolve_context(ctx_req.clone(), Box::new(DemoPolicy)).await?;
    println!("[2/5] Context package assembled: {} fragments", ctx_package.fragments.len());

    // 3. Planning
    let plan = RetrievalPlanner::build_plan(&ctx_req);
    println!("[3/5] Execution plan '{}' generated: {} steps", plan.id, plan.steps.len());

    // 4. Model Inference
    let model_config = ModelRuntimeConfig::from_env();
    let model_runtime = ModelRuntime::build(model_config).await;
    let model_req = ModelRequest {
        capability: ModelCapability::Reasoning,
        profile: CapabilityProfile::Balanced,
        policy: ExecutionPolicy::PreferLocal,
        messages: vec![Message::user(prompt)],
        system_prompt: Some("You are Atlas AI assistant.".to_string()),
        tools: Vec::new(),
        max_tokens: 512,
        temperature: 0.2,
        request_id: "demo-model-req".to_string(),
        trace_id: "demo-trace".to_string(),
        stream: false,
    };
    let response = model_runtime.execute_inference(model_req).await?;
    println!("[4/5] Model response generated via {}", response.metadata.provider_id);

    // 5. Cognitive Memory Storage
    let temp_dir = std::env::temp_dir().join("atlas_demo_memory");
    let file_store = FileMemoryStore::new(temp_dir);
    let memory_repo = MemoryRepository::with_file_store(file_store);
    let memory_runtime = MemoryRuntime::with_repository(memory_repo);

    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
    let rec_id = format!("demo-exec-{}", now);
    let rec = MemoryRecord {
        identity: MemoryIdentity {
            id: rec_id.clone(),
            memory_type: MemoryType::Episodic,
            class: MemoryClass::Persistent,
            traits: MemoryTraits {
                supports_search: true,
                supports_versioning: true,
                supports_decay: false,
                supports_archival: true,
                supports_reinforcement: true,
            },
            source: "demo-runner".to_string(),
            version: 1,
            parent_version: None,
            derived_from_version: None,
            provenance: MemoryProvenance {
                source_runtime: "sdk-example".to_string(),
                original_request_id: Some("demo-model-req".to_string()),
                created_by: "demo".to_string(),
            },
            created_at: now,
            updated_at: now,
        },
        content: MemoryContent {
            payload: response.content,
            embedding_references: vec![],
            entities: vec![],
            relationships: vec![],
            summaries: vec![format!("Demo trace: {}", prompt)],
            attachments: vec![],
        },
        metadata: MemoryMetadata {
            confidence: 1.0,
            quality: 1.0,
            stability: 1.0,
            importance: 1.0,
            decay: 0.0,
            reinforcement: 1.0,
            access_count: 1,
            last_accessed: now,
            expiration_policy: ExpirationPolicy::Never,
        },
    };

    memory_runtime.store(&rec).await?;
    println!("[5/5] Record '{}' stored to cognitive memory.", rec_id);

    println!("\nSUCCESS: End-to-End Autonomous Agent Execution Flow Complete!");
    Ok(())
}
