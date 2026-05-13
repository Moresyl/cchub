use tauri::State;

use crate::db::DbState;

use super::log_command_timing;
use super::types::*;

// ── Activity Logs ──

#[tauri::command]
pub fn get_activity_logs(
    date: String,
    db: State<'_, DbState>,
) -> Result<Vec<ActivityItem>, String> {
    let started_at = std::time::Instant::now();
    let result = (|| {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.server_id, COALESCE(s.name, a.server_id), a.request_type, a.status, a.latency_ms, a.recorded_at
                 FROM activity_logs a LEFT JOIN mcp_servers s ON a.server_id = s.id
                 WHERE a.recorded_at LIKE ?1
                 ORDER BY a.recorded_at DESC LIMIT 200",
            )
            .map_err(|e| e.to_string())?;

        let items = stmt
            .query_map([format!("{}%", date)], |row| {
                Ok(ActivityItem {
                    id: row.get(0)?,
                    server_id: row.get(1)?,
                    server_name: row.get(2)?,
                    request_type: row.get(3)?,
                    status: row.get(4)?,
                    latency_ms: row.get(5)?,
                    recorded_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    })();
    log_command_timing("get_activity_logs", started_at);
    result
}

#[tauri::command]
pub fn get_activity_heatmap(days: i64, db: State<'_, DbState>) -> Result<Vec<HeatmapDay>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT substr(recorded_at, 1, 10) as day, COUNT(*) as cnt
             FROM activity_logs
             WHERE recorded_at >= date('now', ?1)
             GROUP BY day ORDER BY day",
        )
        .map_err(|e| e.to_string())?;

    let offset = format!("-{} days", days);
    let heatmap = stmt
        .query_map([offset], |row| {
            Ok(HeatmapDay {
                date: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(heatmap)
}
