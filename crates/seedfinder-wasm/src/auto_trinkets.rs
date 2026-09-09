//! Browser-only transport of the performance setting. Branching lives in core.
use shpd_seedfinder_core::query::SearchQuery;

pub fn decode(json: &str) -> Result<(SearchQuery, bool), String> {
    let mut value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let enabled = value
        .as_object_mut()
        .and_then(|v| v.remove("auto_apply_trinkets"));
    let enabled = match enabled {
        None => false,
        Some(serde_json::Value::Bool(enabled)) => enabled,
        _ => return Err("auto_apply_trinkets must be a boolean".to_owned()),
    };
    let query = shpd_seedfinder_core::json_query::decode(&value.to_string())?;
    let enabled = shpd_seedfinder_core::auto_trinkets::enabled(&query, enabled);
    Ok((query, enabled))
}
