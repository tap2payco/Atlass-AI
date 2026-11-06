use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use async_trait::async_trait;
use platform_contracts::manifest::RuntimeManifest;
use platform_contracts::dependencies::DependencyDescriptor;
use platform_contracts::health::HealthState;
use runtime_kernel::config::ConfigLoader;
use runtime_kernel::services::ManifestService;
use runtime_kernel::health::HealthRegistry;
use runtime_kernel::metrics::MetricsRegistry;
use runtime_kernel::lifecycle::{KernelSupervisor, RuntimeLifecycle};

struct DummyRuntime {
    name: String,
    started: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

#[async_trait]
impl RuntimeLifecycle for DummyRuntime {
    fn name(&self) -> &str {
        &self.name
    }

    async fn startup(&self) -> Result<(), String> {
        self.started.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), String> {
        self.stopped.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn mock_manifest(id: &str, deps: Vec<&str>) -> RuntimeManifest {
    RuntimeManifest {
        name: id.to_string(),
        runtime_id: id.to_string(),
        version: "1.0.0".to_string(),
        api_version: "1.0.0".to_string(),
        schema_version: "1.0.0".to_string(),
        build_hash: "abc".to_string(),
        minimum_kernel_version: "1.0.0".to_string(),
        capabilities: vec!["test".to_string()],
        dependencies: deps.into_iter().map(|d| DependencyDescriptor::Simple(d.to_string())).collect(),
        health_endpoint: format!("/health/{}", id),
        metrics_prefix: format!("atlas_{}", id),
        supported_events: vec![],
        auto_start: true,
        hot_reload: false,
        enabled: true,
    }
}

#[tokio::test]
async fn test_manifest_parsing() {
    let yaml = r#"
name: runtime-context
runtime_id: runtime-context
version: "1.0.0"
api_version: "1.0.0"
schema_version: "1.0.0"
build_hash: "hash123"
minimum_kernel_version: "1.0.0"
capabilities:
  - ContextResolution
dependencies: []
health_endpoint: "/health"
metrics_prefix: "atlas_context"
auto_start: true
enabled: true
"#;

    let manifest = ConfigLoader::load_manifest_str(yaml).unwrap();
    assert_eq!(manifest.runtime_id, "runtime-context");
    assert_eq!(manifest.metrics_prefix, "atlas_context");
}

#[tokio::test]
async fn test_manifest_service_directory_scan() {
    let temp_dir = std::env::temp_dir().join("atlas_test_kernel_scan");
    let _ = std::fs::create_dir_all(&temp_dir);

    let manifest_path = temp_dir.join("runtime.yaml");
    let yaml = r#"
name: test-runtime
runtime_id: test-runtime
version: "1.0.0"
api_version: "1.0.0"
schema_version: "1.0.0"
build_hash: "hash"
minimum_kernel_version: "1.0.0"
capabilities: []
dependencies: []
health_endpoint: "/health"
metrics_prefix: "test"
"#;
    std::fs::write(&manifest_path, yaml).unwrap();

    let service = ManifestService::new();
    let manifests = service.scan_directories(&temp_dir);
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].runtime_id, "test-runtime");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_health_and_metrics_registries() {
    let health = HealthRegistry::new();
    health.register_endpoint("repo", "/health/repo");
    assert_eq!(health.get_status("repo"), Some(HealthState::Healthy));

    health.update_state("repo", HealthState::Degraded);
    assert_eq!(health.get_status("repo"), Some(HealthState::Degraded));
    assert!(!health.is_all_healthy());

    let metrics = MetricsRegistry::new();
    metrics.register_prefix("repo", "atlas_repo");
    assert_eq!(metrics.get_prefix("repo"), Some("atlas_repo".to_string()));

    metrics.increment("requests_total", 5);
    assert_eq!(metrics.get_counter("requests_total"), 5);
}

#[tokio::test]
async fn test_kernel_supervisor_topological_boot() {
    let mut supervisor = KernelSupervisor::new();

    // Base runtime (no deps)
    let base_started = Arc::new(AtomicBool::new(false));
    let base_stopped = Arc::new(AtomicBool::new(false));
    let base_rt = Arc::new(DummyRuntime {
        name: "base".to_string(),
        started: base_started.clone(),
        stopped: base_stopped.clone(),
    });
    supervisor.register(mock_manifest("base", vec![]), base_rt);

    // Dependent runtime (depends on base)
    let dep_started = Arc::new(AtomicBool::new(false));
    let dep_stopped = Arc::new(AtomicBool::new(false));
    let dep_rt = Arc::new(DummyRuntime {
        name: "dependent".to_string(),
        started: dep_started.clone(),
        stopped: dep_stopped.clone(),
    });
    supervisor.register(mock_manifest("dependent", vec!["base"]), dep_rt);

    // Boot all
    let boot_res = supervisor.boot_all().await;
    assert!(boot_res.is_ok());

    assert!(base_started.load(Ordering::SeqCst));
    assert!(dep_started.load(Ordering::SeqCst));
    assert!(supervisor.health_registry.is_all_healthy());

    // Shutdown all
    let shutdown_res = supervisor.shutdown_all().await;
    assert!(shutdown_res.is_ok());
    assert!(base_stopped.load(Ordering::SeqCst));
    assert!(dep_stopped.load(Ordering::SeqCst));
}
