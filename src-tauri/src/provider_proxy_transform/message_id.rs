pub(crate) fn anthropic_message_id(upstream_id: Option<&str>) -> String {
    if let Some(id) = upstream_id.filter(|id| !id.trim().is_empty()) {
        if id.starts_with("msg_") && id.len() > 4 {
            return id.to_owned();
        }
        let suffix = id
            .strip_prefix("chatcmpl-")
            .or_else(|| id.strip_prefix("resp_"))
            .unwrap_or(id);
        if !suffix.is_empty() && id != "msg_" {
            return format!("msg_{suffix}");
        }
    }
    format!("msg_{}", uuid::Uuid::new_v4().simple())
}

#[cfg(test)]
mod tests;
