use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::ProxyNode;

use super::{
    map_get_bool, map_get_json_map, map_get_mapping, map_get_string, map_get_string_list,
    map_get_u64, object,
};

#[derive(Debug, Serialize)]
struct WsTransport {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct GrpcTransport {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authority: Option<String>,
}

#[derive(Debug, Serialize)]
struct HttpTransport {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

pub(super) fn build_transport(node: &ProxyNode) -> Result<Option<Map<String, Value>>> {
    let network = map_get_string(&node.data, "network").unwrap_or_else(|| "tcp".to_string());

    match network.as_str() {
        "tcp" => Ok(None),
        "ws" => Ok(Some(
            serde_json::to_value(build_ws_transport(node)?)?
                .as_object()
                .cloned()
                .unwrap_or_else(|| object(Vec::new())),
        )),
        "grpc" => Ok(Some(
            serde_json::to_value(build_grpc_transport(node))?
                .as_object()
                .cloned()
                .unwrap_or_else(|| object(Vec::new())),
        )),
        "http" | "h2" => Ok(Some(
            serde_json::to_value(build_http_transport(node))?
                .as_object()
                .cloned()
                .unwrap_or_else(|| object(Vec::new())),
        )),
        unsupported => bail!("transport `{unsupported}` is not supported yet"),
    }
}

fn build_ws_transport(node: &ProxyNode) -> Result<WsTransport> {
    let (path, headers) = if let Some(ws_opts) = map_get_mapping(&node.data, "ws-opts") {
        (
            map_get_string(ws_opts, "path"),
            map_get_json_map(ws_opts, "headers").filter(|headers| !headers.is_empty()),
        )
    } else {
        (
            map_get_string(&node.data, "path"),
            map_get_string(&node.data, "host")
                .map(|host| Map::from_iter([("Host".to_string(), Value::String(host))])),
        )
    };

    Ok(WsTransport {
        r#type: "ws".to_string(),
        path,
        headers,
    })
}

fn build_grpc_transport(node: &ProxyNode) -> GrpcTransport {
    let service_name = map_get_mapping(&node.data, "grpc-opts")
        .and_then(|grpc_opts| map_get_string(grpc_opts, "grpc-service-name"))
        .or_else(|| map_get_string(&node.data, "serviceName"));

    GrpcTransport {
        r#type: "grpc".to_string(),
        service_name,
        authority: map_get_string(&node.data, "authority"),
    }
}

fn build_http_transport(node: &ProxyNode) -> HttpTransport {
    let (host, path) = if let Some(http_opts) =
        map_get_mapping(&node.data, "h2-opts").or_else(|| map_get_mapping(&node.data, "http-opts"))
    {
        (
            map_get_string_list(http_opts, "host"),
            map_get_string(http_opts, "path"),
        )
    } else {
        (
            map_get_string(&node.data, "host").map(|host| vec![host]),
            map_get_string(&node.data, "path"),
        )
    };

    HttpTransport {
        r#type: "http".to_string(),
        host,
        path,
    }
}

#[derive(Debug, Serialize)]
struct MultiplexBrutalConfig {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    up_mbps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    down_mbps: Option<u64>,
}

#[derive(Debug, Serialize)]
struct MultiplexConfig {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_connections: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_streams: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_streams: Option<u64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    padding: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    brutal: Option<MultiplexBrutalConfig>,
}

pub(super) fn build_multiplex(node: &ProxyNode) -> Option<Map<String, Value>> {
    let smux = map_get_mapping(&node.data, "smux")?;
    if !map_get_bool(smux, "enabled") {
        return None;
    }

    let brutal = map_get_mapping(smux, "brutal-opts").and_then(|opts| {
        if map_get_bool(opts, "enabled") {
            Some(MultiplexBrutalConfig {
                enabled: true,
                up_mbps: map_get_u64(opts, "up"),
                down_mbps: map_get_u64(opts, "down"),
            })
        } else {
            None
        }
    });

    let config = MultiplexConfig {
        enabled: true,
        protocol: map_get_string(smux, "protocol"),
        max_connections: map_get_u64(smux, "max-connections"),
        min_streams: map_get_u64(smux, "min-streams"),
        max_streams: map_get_u64(smux, "max-streams"),
        padding: map_get_bool(smux, "padding"),
        brutal,
    };

    serde_json::to_value(config)
        .ok()
        .and_then(|v| v.as_object().cloned())
}
