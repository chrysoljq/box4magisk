mod protocols;
mod tls;
mod transport;

use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};
use serde_yaml::{Mapping, Value as YamlValue};

use crate::ProxyNode;

use self::protocols::convert_node;

#[derive(Debug, Default)]
pub struct RenderedOutbounds {
    pub outbounds: Vec<Value>,
    pub warnings: Vec<String>,
}

pub fn render_outbounds(
    nodes: Vec<ProxyNode>,
    selector_tag: &str,
    include_urltest: bool,
    urltest_tag: &str,
) -> RenderedOutbounds {
    let mut rendered = RenderedOutbounds::default();
    let mut tag_counts = std::collections::BTreeMap::<String, usize>::new();

    for node in nodes {
        match convert_node(&node, &mut rendered.warnings) {
            Ok(Some(mut outbound)) => {
                let base_tag = outbound
                    .get("tag")
                    .and_then(Value::as_str)
                    .unwrap_or("proxy")
                    .to_string();
                let unique = unique_tag(&base_tag, &mut tag_counts);
                outbound["tag"] = Value::String(unique);
                rendered.outbounds.push(outbound);
            }
            Ok(None) => {}
            Err(error) => rendered.warnings.push(format!(
                "skipped proxy `{}` ({}): {error}",
                node.name, node.ty
            )),
        }
    }

    if include_urltest && !rendered.outbounds.is_empty() {
        let tags = rendered
            .outbounds
            .iter()
            .filter_map(|item| item.get("tag").and_then(Value::as_str))
            .map(|item| Value::String(item.to_string()))
            .collect::<Vec<_>>();
        rendered.outbounds.push(json!({
            "type": "urltest",
            "tag": urltest_tag,
            "outbounds": tags,
            "url": "https://cp.cloudflare.com/generate_204",
            "interval": "10m",
            "tolerance": 50,
            "interrupt_exist_connections": true,
        }));
    }

    if !selector_tag.is_empty() && !rendered.outbounds.is_empty() {
        let mut selector_members = vec![Value::String("direct".to_string())];
        if include_urltest {
            selector_members.push(Value::String(urltest_tag.to_string()));
        }
        selector_members.extend(
            rendered
                .outbounds
                .iter()
                .filter_map(|item| item.get("tag").and_then(Value::as_str))
                .filter(|tag| *tag != selector_tag && *tag != urltest_tag)
                .map(|tag| Value::String(tag.to_string())),
        );

        let default = selector_members
            .iter()
            .filter_map(Value::as_str)
            .find(|value| *value != "direct")
            .unwrap_or("direct")
            .to_string();

        rendered.outbounds.push(json!({
            "type": "selector",
            "tag": selector_tag,
            "outbounds": selector_members,
            "default": default,
            "interrupt_exist_connections": true,
        }));
    }

    rendered
}
fn unique_tag(tag: &str, tag_counts: &mut std::collections::BTreeMap<String, usize>) -> String {
    let entry = tag_counts.entry(tag.to_string()).or_insert(0);
    *entry += 1;
    if *entry == 1 {
        tag.to_string()
    } else {
        format!("{tag}-{}", *entry)
    }
}

pub(super) fn yaml_map_to_json(map: &Mapping) -> Value {
    let mut object = Map::new();
    for (key, value) in map {
        let Some(key) = key.as_str() else {
            continue;
        };
        object.insert(key.to_string(), yaml_to_json(value));
    }
    Value::Object(object)
}

fn yaml_to_json(value: &YamlValue) -> Value {
    match value {
        YamlValue::Null => Value::Null,
        YamlValue::Bool(flag) => Value::Bool(*flag),
        YamlValue::Number(number) => {
            if let Some(int) = number.as_i64() {
                Value::from(int)
            } else if let Some(float) = number.as_f64() {
                Value::from(float)
            } else {
                Value::Null
            }
        }
        YamlValue::String(text) => Value::String(text.clone()),
        YamlValue::Sequence(values) => Value::Array(values.iter().map(yaml_to_json).collect()),
        YamlValue::Mapping(map) => yaml_map_to_json(map),
        _ => Value::Null,
    }
}

pub(super) fn object(entries: Vec<(&str, Value)>) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

pub(super) fn required_string(map: &Mapping, key: &str) -> Result<String> {
    map_get_string(map, key).ok_or_else(|| anyhow!("missing `{key}`"))
}

pub(super) fn required_u64(map: &Mapping, key: &str) -> Result<u64> {
    map_get_u64(map, key).ok_or_else(|| anyhow!("missing or invalid `{key}`"))
}

pub(super) fn first_u64(map: &Mapping, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| map_get_u64(map, key))
}

pub(super) fn first_string(map: &Mapping, keys: &[&str]) -> Result<String> {
    keys.iter()
        .find_map(|key| map_get_string(map, key))
        .ok_or_else(|| anyhow!("missing one of {}", keys.join(", ")))
}

pub(super) fn optional_string(map: &Mapping, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| map_get_string(map, key))
}

pub(super) fn optional_bool(map: &Mapping, keys: &[&str]) -> bool {
    keys.iter().any(|key| map_get_bool(map, key))
}

pub(super) fn map_get_string(map: &Mapping, key: &str) -> Option<String> {
    map.get(YamlValue::String(key.to_string()))
        .and_then(|value| match value {
            YamlValue::String(text) => Some(text.clone()),
            YamlValue::Number(number) => Some(number.to_string()),
            YamlValue::Bool(flag) => Some(flag.to_string()),
            _ => None,
        })
}

pub(super) fn map_get_u64(map: &Mapping, key: &str) -> Option<u64> {
    map.get(YamlValue::String(key.to_string()))
        .and_then(|value| match value {
            YamlValue::Number(number) => number
                .as_u64()
                .or_else(|| number.as_i64().map(|v| v as u64)),
            YamlValue::String(text) => text.parse::<u64>().ok(),
            _ => None,
        })
}

pub(super) fn map_get_bool(map: &Mapping, key: &str) -> bool {
    map.get(YamlValue::String(key.to_string()))
        .is_some_and(|value| match value {
            YamlValue::Bool(flag) => *flag,
            YamlValue::String(text) => matches!(text.as_str(), "true" | "1"),
            _ => false,
        })
}

pub(super) fn map_get_mapping<'a>(map: &'a Mapping, key: &str) -> Option<&'a Mapping> {
    map.get(YamlValue::String(key.to_string()))
        .and_then(|value| match value {
            YamlValue::Mapping(inner) => Some(inner),
            _ => None,
        })
}

pub(super) fn map_get_string_list(map: &Mapping, key: &str) -> Option<Vec<String>> {
    map.get(YamlValue::String(key.to_string()))
        .and_then(|value| match value {
            YamlValue::Sequence(items) => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        YamlValue::String(text) => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            YamlValue::String(text) => Some(
                text.split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
}

pub(super) fn map_get_csv(map: &Mapping, key: &str) -> Option<Vec<String>> {
    map_get_string_list(map, key)
}

pub(super) fn map_get_csv_numbers(map: &Mapping, key: &str) -> Option<Vec<u64>> {
    map_get_string(map, key).map(|text| {
        text.split(',')
            .map(str::trim)
            .filter_map(|part| part.parse::<u64>().ok())
            .collect::<Vec<_>>()
    })
}

pub(super) fn map_get_json_map(map: &Mapping, key: &str) -> Option<Map<String, Value>> {
    let nested = map_get_mapping(map, key)?;
    yaml_map_to_json(nested).as_object().cloned()
}
