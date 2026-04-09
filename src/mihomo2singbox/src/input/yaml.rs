use anyhow::{Context, Result};
use serde_yaml::{Mapping, Value};

use super::utils::{extract_yaml_sequence, map_get_string};
use super::{ParsedInput, ProxyNode};

pub(super) fn parse_yaml_nodes(content: &str) -> Result<ParsedInput> {
    let value: Value = serde_yaml::from_str(content).context("yaml parse failed")?;
    let mut parsed = ParsedInput::default();
    let nodes = match &value {
        Value::Mapping(map) => extract_yaml_sequence(map, "proxies"),
        Value::Sequence(seq) => Some(seq.clone()),
        _ => None,
    };

    let Some(nodes) = nodes else {
        return Ok(parsed);
    };

    for item in nodes {
        let Value::Mapping(map) = item else {
            parsed
                .warnings
                .push("skipped non-mapping proxy entry in yaml".to_string());
            continue;
        };
        push_yaml_proxy_node(&mut parsed, map);
    }

    Ok(parsed)
}

fn push_yaml_proxy_node(parsed: &mut ParsedInput, map: Mapping) {
    let ty = map_get_string(&map, "type").unwrap_or_default();
    let name = map_get_string(&map, "name")
        .unwrap_or_else(|| format!("unnamed-{}", parsed.nodes.len() + 1));
    if ty.is_empty() {
        parsed
            .warnings
            .push(format!("skipped proxy `{name}` because type is missing"));
        return;
    }
    parsed.nodes.push(ProxyNode {
        name,
        ty,
        data: map,
    });
}
