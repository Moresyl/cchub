use rusqlite::{params, Connection, OptionalExtension};

use crate::provider_proxy::{ProxyRequestInsights, ProxyUsageMetrics, UpstreamTarget};

pub(super) struct RequestRecord<'a> {
    pub request_id: &'a str,
    pub tool_id: &'a str,
    pub upstream: &'a UpstreamTarget,
    pub insights: &'a ProxyRequestInsights,
    pub usage: &'a ProxyUsageMetrics,
    pub latency_ms: u64,
    pub status_code: u16,
    pub error_message: Option<&'a str>,
    pub created_at: &'a str,
    pub total_cost_usd: f64,
}

#[derive(Default)]
struct Contribution {
    created_at: String,
    tool_id: String,
    success: i64,
    input: i64,
    output: i64,
    cache_read: i64,
    cache_creation: i64,
    cost: f64,
    latency: i64,
}

fn counter(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

pub(super) fn persist_request(
    conn: &Connection,
    record: &RequestRecord<'_>,
) -> rusqlite::Result<()> {
    if !record.total_cost_usd.is_finite() || record.total_cost_usd < 0.0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "Invalid proxy request cost".into(),
        ));
    }
    let cost = format!("{:.6}", record.total_cost_usd)
        .parse::<f64>()
        .map_err(|_| rusqlite::Error::InvalidParameterName("Invalid proxy request cost".into()))?;
    let tx = conn.unchecked_transaction()?;
    let previous = tx.query_row(
        "SELECT created_at,tool_id,status_code,input_tokens,output_tokens,cache_read_tokens,cache_creation_tokens,CAST(total_cost_usd AS REAL),latency_ms FROM proxy_request_logs WHERE request_id=?1",
        [record.request_id],
        |row| Ok(Contribution {
            created_at: row.get(0)?,
            tool_id: row.get(1)?,
            success: i64::from((200..300).contains(&row.get::<_, i64>(2)?)),
            input: row.get(3)?,
            output: row.get(4)?,
            cache_read: row.get(5)?,
            cache_creation: row.get(6)?,
            cost: row.get(7)?,
            latency: row.get(8)?,
        }),
    ).optional()?;
    if previous
        .as_ref()
        .is_some_and(|old| old.tool_id != record.tool_id)
    {
        return Err(rusqlite::Error::InvalidParameterName(
            "Proxy request ID belongs to another tool".into(),
        ));
    }
    let created_at = previous.as_ref().map_or_else(
        || record.created_at.to_string(),
        |old| old.created_at.clone(),
    );
    let day = created_at.get(..10).ok_or_else(|| {
        rusqlite::Error::InvalidParameterName("Invalid proxy accounting date".into())
    })?;
    let present = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM proxy_usage_daily_rollups WHERE day=?1 AND tool_id=?2)",
        params![day, record.tool_id],
        |row| row.get::<_, bool>(0),
    )?;
    let next = Contribution {
        success: i64::from((200..300).contains(&record.status_code)),
        input: counter(record.usage.input_tokens),
        output: counter(record.usage.output_tokens),
        cache_read: counter(record.usage.cache_read_tokens),
        cache_creation: counter(record.usage.cache_creation_tokens),
        cost,
        latency: counter(record.latency_ms),
        ..Default::default()
    };
    tx.execute(
        "INSERT INTO proxy_request_logs (
            request_id,tool_id,profile_id,provider_name,request_model,response_model,
            input_tokens,output_tokens,cache_read_tokens,cache_creation_tokens,total_cost_usd,
            latency_ms,status_code,is_streaming,error_message,created_at,upstream_model
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
        ON CONFLICT(request_id) DO UPDATE SET
            profile_id=excluded.profile_id,provider_name=excluded.provider_name,
            request_model=excluded.request_model,response_model=excluded.response_model,upstream_model=excluded.upstream_model,
            input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,
            cache_read_tokens=excluded.cache_read_tokens,cache_creation_tokens=excluded.cache_creation_tokens,
            total_cost_usd=excluded.total_cost_usd,latency_ms=excluded.latency_ms,
            status_code=excluded.status_code,is_streaming=excluded.is_streaming,error_message=excluded.error_message",
        params![
            record.request_id,record.tool_id,record.upstream.profile_id,record.upstream.profile_name,
            record.insights.request_model,record.usage.response_model,next.input,next.output,next.cache_read,
            next.cache_creation,format!("{:.6}", next.cost),next.latency,record.status_code,
            record.insights.is_streaming,record.error_message,created_at,record.insights.upstream_model,
        ],
    )?;
    if present {
        let count_delta = i64::from(previous.is_none());
        let old = previous.unwrap_or_default();
        tx.execute(
            "UPDATE proxy_usage_daily_rollups SET
                success_requests=success_requests+?3,
                total_input_tokens=total_input_tokens+?4,
                total_output_tokens=total_output_tokens+?5,
                total_cache_read_tokens=total_cache_read_tokens+?6,
                total_cache_creation_tokens=total_cache_creation_tokens+?7,
                total_cost_usd=printf('%.6f',CAST(total_cost_usd AS REAL)+?8),
                avg_latency_ms=(avg_latency_ms*total_requests+?9)/(total_requests+?10),
                total_requests=total_requests+?10,updated_at=?11
             WHERE day=?1 AND tool_id=?2",
            params![
                day,
                record.tool_id,
                next.success - old.success,
                next.input - old.input,
                next.output - old.output,
                next.cache_read - old.cache_read,
                next.cache_creation - old.cache_creation,
                next.cost - old.cost,
                (next.latency - old.latency) as f64,
                count_delta,
                record.created_at
            ],
        )?;
    } else {
        // Rebuild a missing cache from authoritative logs, including legacy or pruned rollups.
        tx.execute(
            "INSERT INTO proxy_usage_daily_rollups (
                day,tool_id,total_requests,success_requests,total_input_tokens,total_output_tokens,
                total_cache_read_tokens,total_cache_creation_tokens,total_cost_usd,avg_latency_ms,updated_at
             ) SELECT ?1,?2,COUNT(*),SUM(CASE WHEN status_code>=200 AND status_code<300 THEN 1 ELSE 0 END),
                SUM(input_tokens),SUM(output_tokens),SUM(cache_read_tokens),SUM(cache_creation_tokens),
                printf('%.6f',SUM(CAST(total_cost_usd AS REAL))),AVG(latency_ms),?3
             FROM proxy_request_logs WHERE tool_id=?2 AND substr(created_at,1,10)=?1",
            params![day,record.tool_id,record.created_at],
        )?;
    }
    tx.commit()
}

#[cfg(test)]
#[path = "accounting_tests.rs"]
mod tests;
