mod uri;
mod utils;
mod yaml;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_yaml::Mapping;

use self::uri::parse_uri_node;
use self::utils::decode_subscription;
use self::yaml::parse_yaml_nodes;

#[derive(Debug, Clone)]
pub struct ProxyNode {
    pub name: String,
    pub ty: String,
    pub data: Mapping,
}

#[derive(Debug, Default)]
pub struct ParsedInput {
    pub nodes: Vec<ProxyNode>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VmessLink {
    ps: Option<String>,
    add: String,
    port: String,
    id: String,
    aid: Option<String>,
    scy: Option<String>,
    net: Option<String>,
    #[serde(rename = "type")]
    type_name: Option<String>,
    host: Option<String>,
    path: Option<String>,
    tls: Option<String>,
    sni: Option<String>,
    alpn: Option<String>,
    fp: Option<String>,
}

pub fn parse_input(content: &str) -> Result<ParsedInput> {
    if let Ok(parsed) = parse_yaml_nodes(content) {
        if !parsed.nodes.is_empty() {
            return Ok(parsed);
        }
    }

    if let Some(decoded) = decode_subscription(content) {
        let nested =
            parse_input(&decoded).context("failed to parse decoded subscription content")?;
        if !nested.nodes.is_empty() {
            return Ok(nested);
        }
    }

    parse_uri_lines(content)
}

fn parse_uri_lines(content: &str) -> Result<ParsedInput> {
    let mut parsed = ParsedInput::default();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match parse_uri_node(line) {
            Ok(node) => parsed.nodes.push(node),
            Err(error) => parsed
                .warnings
                .push(format!("skipped line `{line}`: {error}")),
        }
    }

    if parsed.nodes.is_empty() {
        bail!("input is neither yaml proxies nor supported subscription links");
    }

    Ok(parsed)
}
