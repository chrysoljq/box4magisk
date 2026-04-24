use anyhow::{Result, bail, Context};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::ProxyNode;

use super::tls::{build_optional_tls, build_required_tls};
use super::transport::{build_multiplex, build_transport};
use super::{
    first_string, first_u64, map_get_csv, map_get_csv_numbers, map_get_mapping,
    map_get_string, map_get_u64, optional_bool, optional_string, required_string,
    required_u64,
};

#[derive(Debug, Serialize)]
struct Hysteria2Obfs {
    r#type: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct ShadowsocksOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    method: String,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin_opts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct VmessOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    uuid: String,
    security: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    alter_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct VlessOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packet_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct TrojanOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct SocksOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct HttpOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct WireguardOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    private_key: String,
    peer_public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pre_shared_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reserved: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mtu: Option<u64>,
}

#[derive(Debug, Serialize)]
struct AnyTlsOutbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_session_check_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_session_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_idle_session: Option<u64>,
    tls: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multiplex: Option<Map<String, Value>>,
}

#[derive(Debug, Serialize)]
struct Hysteria2Outbound {
    r#type: String,
    tag: String,
    server: String,
    server_port: u64,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    up_mbps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    down_mbps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    obfs: Option<Hysteria2Obfs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<String>,
    tls: Map<String, Value>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    brutal_debug: bool,
}

pub(super) fn convert_node(node: &ProxyNode, warnings: &mut Vec<String>) -> Result<Option<Value>> {
    match node.ty.as_str() {
        "direct" | "dns" => {
            warnings.push(format!(
                "built-in mihomo proxy `{}` with type `{}` was skipped because base sing-box config usually already provides it",
                node.name, node.ty
            ));
            Ok(None)
        }
        "ss" => convert_ss(node, warnings).with_context(|| format!("failed to convert shadowsocks node `{}`", node.name)),
        "vmess" => convert_vmess(node).with_context(|| format!("failed to convert vmess node `{}`", node.name)),
        "vless" => convert_vless(node).with_context(|| format!("failed to convert vless node `{}`", node.name)),
        "trojan" => convert_trojan(node).with_context(|| format!("failed to convert trojan node `{}`", node.name)),
        "socks5" | "socks" => convert_socks(node).with_context(|| format!("failed to convert socks node `{}`", node.name)),
        "http" => convert_http(node).with_context(|| format!("failed to convert http node `{}`", node.name)),
        "wireguard" => convert_wireguard(node).with_context(|| format!("failed to convert wireguard node `{}`", node.name)),
        "hysteria2" | "hy2" => convert_hysteria2(node).with_context(|| format!("failed to convert hysteria2 node `{}`", node.name)),
        "anytls" => convert_anytls(node).with_context(|| format!("failed to convert anytls node `{}`", node.name)),
        other => bail!("protocol `{other}` is not supported yet"),
    }
}

fn convert_ss(node: &ProxyNode, warnings: &mut Vec<String>) -> Result<Option<Value>> {
    // TODO: ShadowTLS support — requires a dedicated struct with nested tls config,
    // version/handshake fields, etc. Not a simple plugin_opts passthrough.
    let (plugin, plugin_opts) = match map_get_string(&node.data, "plugin") {
        Some(raw) => {
            if let Some(opts_map) = map_get_mapping(&node.data, "plugin-opts") {
                (Some(raw), Some(yaml_mapping_to_opts(opts_map, &node.name, warnings)))
            } else if let Some((name, opts)) = raw.split_once(';') {
                (Some(name.to_string()), Some(opts.to_string()))
            } else {
                (Some(raw), None)
            }
        }
        None => (None, None),
    };

    let outbound = ShadowsocksOutbound {
        r#type: "shadowsocks".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        method: first_string(&node.data, &["cipher", "method"])?,
        password: required_string(&node.data, "password")?,
        plugin,
        plugin_opts,
        detour: map_get_string(&node.data, "dialer-proxy"),
        multiplex: build_multiplex(node),
    };
    Ok(Some(serde_json::to_value(outbound)?))
}

fn yaml_mapping_to_opts(map: &serde_yaml::Mapping, node_name: &str, warnings: &mut Vec<String>) -> String {
    map.iter()
        .filter_map(|(k, v)| {
            let key = k.as_str()?;
            let value = match v {
                serde_yaml::Value::String(s) => s.clone(),
                serde_yaml::Value::Bool(b) => b.to_string(),
                serde_yaml::Value::Number(n) => n.to_string(),
                _ => {
                    warnings.push(format!(
                        "shadowsocks plugin option `{key}` in node `{node_name}` has an unsupported type and was skipped"
                    ));
                    return None;
                }
            };
            Some(format!("{key}={value}"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn convert_vmess(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = VmessOutbound {
        r#type: "vmess".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        uuid: first_string(&node.data, &["uuid", "id"])?,
        security: map_get_string(&node.data, "cipher").unwrap_or_else(|| "auto".to_string()),
        alter_id: map_get_u64(&node.data, "alterId"),
        detour: map_get_string(&node.data, "dialer-proxy"),
        transport: build_transport(node)?,
        tls: build_optional_tls(node)?,
        multiplex: build_multiplex(node),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_vless(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = VlessOutbound {
        r#type: "vless".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        uuid: first_string(&node.data, &["uuid", "id"])?,
        flow: map_get_string(&node.data, "flow"),
        detour: map_get_string(&node.data, "dialer-proxy"),
        transport: build_transport(node)?,
        tls: build_optional_tls(node)?,
        packet_encoding: map_get_string(&node.data, "packet-encoding"),
        multiplex: build_multiplex(node),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_trojan(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = TrojanOutbound {
        r#type: "trojan".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        password: required_string(&node.data, "password")?,
        flow: map_get_string(&node.data, "flow"),
        detour: map_get_string(&node.data, "dialer-proxy"),
        transport: build_transport(node)?,
        tls: build_optional_tls(node)?,
        multiplex: build_multiplex(node),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_socks(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = SocksOutbound {
        r#type: "socks".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        version: "5".to_string(),
        detour: map_get_string(&node.data, "dialer-proxy"),
        username: map_get_string(&node.data, "username"),
        password: map_get_string(&node.data, "password"),
        multiplex: build_multiplex(node),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_http(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = HttpOutbound {
        r#type: "http".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        detour: map_get_string(&node.data, "dialer-proxy"),
        username: map_get_string(&node.data, "username"),
        password: map_get_string(&node.data, "password"),
        tls: build_optional_tls(node)?,
        multiplex: build_multiplex(node),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_wireguard(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = WireguardOutbound {
        r#type: "wireguard".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        private_key: first_string(&node.data, &["private-key", "private_key"])?,
        peer_public_key: first_string(&node.data, &["public-key", "public_key"])?,
        detour: map_get_string(&node.data, "dialer-proxy"),
        local_address: map_get_csv(&node.data, "ip").or_else(|| map_get_csv(&node.data, "address")),
        pre_shared_key: map_get_string(&node.data, "pre-shared-key")
            .or_else(|| map_get_string(&node.data, "pre_shared_key")),
        reserved: map_get_csv_numbers(&node.data, "reserved"),
        mtu: map_get_u64(&node.data, "mtu"),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_anytls(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = AnyTlsOutbound {
        r#type: "anytls".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        password: required_string(&node.data, "password")?,
        detour: map_get_string(&node.data, "dialer-proxy"),
        idle_session_check_interval: map_get_u64(&node.data, "idle-session-check-interval")
            .map(|v| format!("{v}s")),
        idle_session_timeout: map_get_u64(&node.data, "idle-session-timeout")
            .map(|v| format!("{v}s")),
        min_idle_session: map_get_u64(&node.data, "min-idle-session"),
        tls: build_required_tls(node)?,
        multiplex: build_multiplex(node),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}

fn convert_hysteria2(node: &ProxyNode) -> Result<Option<Value>> {
    let outbound = Hysteria2Outbound {
        r#type: "hysteria2".to_string(),
        tag: node.name.clone(),
        server: required_string(&node.data, "server")?,
        server_port: required_u64(&node.data, "port")?,
        password: required_string(&node.data, "password")?,
        detour: map_get_string(&node.data, "dialer-proxy"),
        up_mbps: first_u64(&node.data, &["up_mbps", "up", "up-speed"]),
        down_mbps: first_u64(&node.data, &["down_mbps", "down", "down-speed"]),
        obfs: optional_string(&node.data, &["obfs-password", "obfs_password"]).map(|password| {
            Hysteria2Obfs {
                r#type: "salamander".to_string(),
                password,
            }
        }),
        network: optional_string(&node.data, &["network"]),
        tls: build_required_tls(node)?,
        brutal_debug: optional_bool(&node.data, &["brutal-debug", "brutal_debug"]),
    };

    Ok(Some(serde_json::to_value(outbound)?))
}