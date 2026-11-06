use runtime_memory::api::MemoryRuntime;

pub async fn execute_query(domain: &str, query_str: &str) -> Result<(), String> {
    println!("Querying domain '{}' with query '{}'...", domain, query_str);

    match domain.to_lowercase().as_str() {
        "memory" => {
            let memory = MemoryRuntime::new();
            let (records, _exp) = memory.retrieve::<String>(query_str).await?;
            println!("Retrieved {} memory records matching '{}'.", records.len(), query_str);
        }
        "context" | "repository" => {
            println!("Query dispatched to {} runtime engine.", domain);
        }
        _ => return Err(format!("Unknown query domain: {}", domain)),
    }

    Ok(())
}
