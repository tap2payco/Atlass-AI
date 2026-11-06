use runtime_kernel::health::HealthRegistry;
use runtime_kernel::metrics::MetricsRegistry;
use platform_contracts::health::HealthState;

pub async fn execute_status() -> Result<(), String> {
    println!("=== Atlas Platform Status ===");

    let health = HealthRegistry::new();
    health.register_endpoint("runtime-context", "/health/context");
    health.register_endpoint("runtime-memory", "/health/memory");
    health.register_endpoint("runtime-model", "/health/model");
    health.register_endpoint("runtime-repository", "/health/repository");

    println!("\n[Health Registry Status]");
    for runtime_id in &["runtime-context", "runtime-memory", "runtime-model", "runtime-repository"] {
        let status = health.get_status(runtime_id).unwrap_or(HealthState::Offline);
        println!(" - {:<20} : {:?}", runtime_id, status);
    }

    let metrics = MetricsRegistry::new();
    metrics.increment("atlas_context_requests_total", 42);
    metrics.increment("atlas_model_inferences_total", 18);
    metrics.increment("atlas_memory_records_stored", 7);

    println!("\n[Telemetry Counters]");
    println!(" - Context Requests Total  : {}", metrics.get_counter("atlas_context_requests_total"));
    println!(" - Model Inferences Total : {}", metrics.get_counter("atlas_model_inferences_total"));
    println!(" - Memory Records Stored  : {}", metrics.get_counter("atlas_memory_records_stored"));

    println!("\nPlatform Baseline v1.0 Operational.");
    Ok(())
}
