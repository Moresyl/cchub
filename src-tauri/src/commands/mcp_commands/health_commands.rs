use super::*;

fn check(server: &McpServer) -> health::HealthCheckResult {
    if server.status != "active" || server.transport != "stdio" {
        return health::HealthCheckResult {
            server_id: server.id.clone(), server_name: server.name.clone(),
            status: "unknown".into(), command_exists: false, can_start: false,
            error_message: Some(if server.transport != "stdio" {
                "Remote MCP connections require a protocol health check; local command checks do not apply."
            } else { "Resolve the missing, disabled or conflicting source before checking health." }.into()),
            latency_ms: None, checked_at: chrono::Utc::now().to_rfc3339(),
        };
    }
    health::check_server_health(
        &server.id,
        &server.name,
        server.command.as_deref().unwrap_or_default(),
        &server.args,
        &server.env,
    )
}

pub fn check_mcp_server_health(
    name: String,
    db: State<'_, DbState>,
) -> Result<health::HealthCheckResult, String> {
    let server = {
        let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
        let id = service::resolve_id(&conn, &name)?;
        service::list(&conn)?
            .into_iter()
            .find(|row| row.server.id == id)
            .ok_or("MCP server was not found")?
            .server
    };
    Ok(check(&server))
}

pub fn check_all_mcp_health(
    db: State<'_, DbState>,
) -> Result<Vec<health::HealthCheckResult>, String> {
    let servers = {
        let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
        service::list(&conn)?
    };
    let results: Vec<_> = servers
        .iter()
        .filter(|row| row.server.status != "disabled")
        .map(|row| check(&row.server))
        .collect();
    let conn = db.0.lock().map_err(|_| "MCP settings are unavailable")?;
    for result in &results {
        let status = match result.status.as_str() {
            "healthy" => "success",
            "unhealthy" => "error",
            _ => "unknown",
        };
        crate::db::record_activity(
            &conn,
            &result.server_id,
            "health_check",
            status,
            result.latency_ms.map(|value| value as i64),
        );
    }
    Ok(results)
}
