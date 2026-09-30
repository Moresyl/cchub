use rusqlite::{Connection, OptionalExtension};

pub(crate) fn model_rates(conn: &Connection, model: &str) -> Result<Option<[f64; 4]>, String> {
    let model = model.trim();
    let rates = conn
        .query_row(
            "SELECT input_cost_per_million, output_cost_per_million,
                cache_read_cost_per_million, cache_write_cost_per_million
         FROM model_pricing WHERE model_id = ?1 OR normalized_model_id = ?2
         ORDER BY CASE WHEN model_id = ?1 THEN 0 ELSE 1 END LIMIT 1",
            rusqlite::params![model, model.to_ascii_lowercase()],
            |row| {
                Ok([
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ])
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    rates
        .map(|rates| {
            let mut result = [0.0; 4];
            for (index, rate) in rates.iter().enumerate() {
                result[index] = rate
                    .parse::<f64>()
                    .ok()
                    .filter(|rate| rate.is_finite() && *rate >= 0.0)
                    .ok_or("Model pricing must contain finite non-negative rates")?;
            }
            Ok(result)
        })
        .transpose()
}

pub(crate) fn estimate_cost(
    conn: &Connection,
    model: &str,
    counts: [u64; 4],
) -> Result<f64, String> {
    let rates = model_rates(conn, model)?.unwrap_or([0.0; 4]);
    let cost: f64 = counts
        .iter()
        .zip(rates)
        .map(|(count, rate)| *count as f64 * rate / 1_000_000.0)
        .sum();
    cost.is_finite()
        .then_some(cost)
        .ok_or_else(|| "Calculated usage cost exceeds the supported range".into())
}
