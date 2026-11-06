#[tokio::test]
async fn test_cli_boot_and_status() {
    let boot_res = atlas_cli::commands::boot::execute_boot().await;
    assert!(boot_res.is_ok());

    let status_res = atlas_cli::commands::status::execute_status().await;
    assert!(status_res.is_ok());
}

#[tokio::test]
async fn test_cli_agent_run_and_query() {
    let run_res = atlas_cli::commands::agent_run::execute_agent_run("Test query architecture").await;
    assert!(run_res.is_ok());

    let query_res = atlas_cli::commands::query::execute_query("memory", "session").await;
    assert!(query_res.is_ok());
}
