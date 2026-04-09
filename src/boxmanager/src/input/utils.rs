use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde_yaml::{Mapping, Value};
use url::Url;

pub(super) fn extract_yaml_sequence(map: &Mapping, key: &str) -> Option<Vec<Value>> {
    map.get(Value::String(key.to_string()))
        .and_then(|value| match value {
            Value::Sequence(seq) => Some(seq.clone()),
            _ => None,
        })
}

pub(super) fn query_value(url: &Url, key: &str) -> Option<String> {
    for (query_key, query_value) in url.query_pairs() {
        if query_key == key {
            return Some(query_value.into_owned());
        }
    }
    None
}

pub(super) fn decode_subscription(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return None;
    }
    let decoded = decode_base64_text(trimmed).ok()?;
    let looks_like_subscription = decoded.contains("://") || decoded.contains("proxies:");
    if looks_like_subscription {
        Some(decoded)
    } else {
        None
    }
}

pub(super) fn decode_base64_text(input: &str) -> Result<String> {
    let sanitized = input.trim().replace('-', "+").replace('_', "/");
    let mut candidates = vec![sanitized.clone()];
    let rem = sanitized.len() % 4;
    if rem != 0 {
        candidates.push(format!("{sanitized}{}", "=".repeat(4 - rem)));
    }

    for candidate in candidates {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(candidate.as_bytes()) {
            if let Ok(decoded) = String::from_utf8(bytes) {
                return Ok(decoded);
            }
        }
    }

    bail!("base64 decode failed")
}

pub(super) fn decode_url_component(input: &str) -> Result<String> {
    urlencoding::decode(input)
        .map(|value| value.to_string())
        .map_err(|error| anyhow!("url decode failed: {error}"))
}

pub(super) fn split_host_port(value: &str) -> Result<(&str, u16)> {
    if let Some(stripped) = value.strip_prefix('[') {
        let (host, right) = stripped
            .split_once(']')
            .ok_or_else(|| anyhow!("invalid ipv6 host"))?;
        let port = right
            .strip_prefix(':')
            .ok_or_else(|| anyhow!("missing port"))?
            .parse::<u16>()
            .context("invalid port")?;
        return Ok((host, port));
    }

    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("missing port"))?;
    Ok((host, port.parse::<u16>().context("invalid port")?))
}

pub(super) fn map_get_string(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        })
}

pub(super) fn set_string(map: &mut Mapping, key: &str, value: &str) {
    map.insert(
        Value::String(key.to_string()),
        Value::String(value.to_string()),
    );
}

pub(super) fn set_u64(map: &mut Mapping, key: &str, value: u64) {
    map.insert(
        Value::String(key.to_string()),
        Value::Number(serde_yaml::Number::from(value)),
    );
}

pub(super) fn set_bool(map: &mut Mapping, key: &str, value: bool) {
    map.insert(Value::String(key.to_string()), Value::Bool(value));
}
