use anyhow::{Context, Result, anyhow, bail};
use serde_yaml::{Mapping, Value};
use url::Url;

use super::utils::{
    decode_base64_text, decode_url_component, map_get_string, query_value, set_bool, set_string,
    set_u64, split_host_port,
};
use super::{ProxyNode, VmessLink};

pub(super) fn parse_uri_node(line: &str) -> Result<ProxyNode> {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("vmess://") {
        return parse_vmess_uri(line);
    }
    if lower.starts_with("ss://") {
        return parse_ss_uri(line);
    }
    if lower.starts_with("vless://")
        || lower.starts_with("trojan://")
        || lower.starts_with("socks://")
        || lower.starts_with("socks5://")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("hysteria2://")
        || lower.starts_with("hy2://")
        || lower.starts_with("wireguard://")
        || lower.starts_with("wg://")
    {
        return parse_standard_uri(line);
    }

    bail!("unsupported URI scheme")
}

fn parse_standard_uri(line: &str) -> Result<ProxyNode> {
    let normalized = line
        .replace("hy2://", "hysteria2://")
        .replace("wg://", "wireguard://")
        .replace("socks://", "socks5://");
    let url = Url::parse(&normalized).context("invalid url")?;
    let scheme = url.scheme().to_string();
    let mut data = Mapping::new();

    let decoded_name = url
        .fragment()
        .filter(|value| !value.is_empty())
        .map(decode_url_component)
        .transpose()?
        .unwrap_or_else(|| url.host_str().unwrap_or("proxy").to_string());

    set_string(&mut data, "type", &scheme_alias(&scheme));
    set_string(&mut data, "name", &decoded_name);
    set_string(&mut data, "server", url.host_str().unwrap_or_default());
    set_u64(
        &mut data,
        "port",
        url.port().ok_or_else(|| anyhow!("missing port in uri"))? as u64,
    );

    match scheme.as_str() {
        "vless" => {
            set_string(&mut data, "uuid", url.username());
            set_string(
                &mut data,
                "cipher",
                query_value(&url, "encryption").as_deref().unwrap_or("none"),
            );
        }
        "trojan" | "hysteria2" => {
            set_string(&mut data, "password", url.username());
            set_bool(&mut data, "tls", true);
        }
        "socks5" | "http" | "https" => {
            if !url.username().is_empty() {
                set_string(&mut data, "username", url.username());
            }
            if let Some(password) = url.password() {
                set_string(&mut data, "password", password);
            }
            if scheme == "https" {
                set_bool(&mut data, "tls", true);
            }
        }
        "wireguard" => {
            set_string(&mut data, "private-key", url.username());
        }
        _ => {}
    }

    populate_common_query_fields(&url, &mut data);
    Ok(ProxyNode {
        name: map_get_string(&data, "name").unwrap_or_else(|| "proxy".to_string()),
        ty: map_get_string(&data, "type").unwrap_or_else(|| scheme_alias(&scheme)),
        data,
    })
}

fn parse_ss_uri(line: &str) -> Result<ProxyNode> {
    let trimmed = line.trim_start_matches("ss://");
    let mut main = trimmed;
    let mut fragment = "";
    if let Some((left, right)) = trimmed.split_once('#') {
        main = left;
        fragment = right;
    }
    let decoded_fragment = urlencoding::decode(fragment)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| fragment.to_string());

    let (head, query) = if let Some((left, right)) = main.split_once('?') {
        (left, Some(right))
    } else {
        (main, None)
    };

    let userinfo_host = if head.contains('@') {
        head.to_string()
    } else {
        let decoded = decode_base64_text(head).context("invalid shadowsocks legacy payload")?;
        decoded
    };

    let (userinfo, host_part) = userinfo_host
        .rsplit_once('@')
        .ok_or_else(|| anyhow!("invalid shadowsocks server section"))?;
    let userinfo_decoded = if userinfo.contains(':') {
        userinfo.to_string()
    } else {
        decode_base64_text(userinfo).context("invalid shadowsocks credentials")?
    };
    let (method, password) = userinfo_decoded
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid shadowsocks method/password"))?;
    let (server, port) = split_host_port(host_part)?;

    let mut data = Mapping::new();
    set_string(&mut data, "type", "ss");
    set_string(
        &mut data,
        "name",
        if decoded_fragment.is_empty() {
            server
        } else {
            &decoded_fragment
        },
    );
    set_string(&mut data, "server", server);
    set_u64(&mut data, "port", port as u64);
    set_string(&mut data, "cipher", method);
    set_string(&mut data, "password", password);

    if let Some(query) = query {
        let full = format!("ss://placeholder@host?{query}");
        let url = Url::parse(&full).context("invalid shadowsocks plugin query")?;
        if let Some(plugin) = query_value(&url, "plugin") {
            set_string(&mut data, "plugin", &plugin);
        }
    }

    Ok(ProxyNode {
        name: map_get_string(&data, "name").unwrap_or_else(|| server.to_string()),
        ty: "ss".to_string(),
        data,
    })
}

fn parse_vmess_uri(line: &str) -> Result<ProxyNode> {
    let payload = line.trim_start_matches("vmess://");
    if let Ok(decoded) = decode_base64_text(payload) {
        if let Ok(link) = serde_json::from_str::<VmessLink>(&decoded) {
            let mut data = Mapping::new();
            set_string(&mut data, "type", "vmess");
            set_string(&mut data, "name", link.ps.as_deref().unwrap_or(&link.add));
            set_string(&mut data, "server", &link.add);
            set_string(&mut data, "port", &link.port);
            set_string(&mut data, "uuid", &link.id);
            set_string(&mut data, "cipher", link.scy.as_deref().unwrap_or("auto"));
            if let Some(aid) = link.aid.as_deref() {
                set_string(&mut data, "alterId", aid);
            }
            if let Some(network) = link.net.as_deref() {
                set_string(&mut data, "network", network);
            }
            if let Some(header) = link.type_name.as_deref() {
                set_string(&mut data, "headerType", header);
            }
            if let Some(host) = link.host.as_deref() {
                if matches!(link.net.as_deref(), Some("ws")) {
                    let mut ws_opts = Mapping::new();
                    let mut headers = Mapping::new();
                    set_string(&mut headers, "Host", host);
                    ws_opts.insert(
                        Value::String("headers".to_string()),
                        Value::Mapping(headers),
                    );
                    if let Some(path) = link.path.as_deref() {
                        set_string(&mut ws_opts, "path", path);
                    }
                    data.insert(
                        Value::String("ws-opts".to_string()),
                        Value::Mapping(ws_opts),
                    );
                } else {
                    set_string(&mut data, "servername", host);
                }
            }
            if let Some(path) = link.path.as_deref() {
                if matches!(link.net.as_deref(), Some("grpc")) {
                    let mut grpc_opts = Mapping::new();
                    set_string(&mut grpc_opts, "grpc-service-name", path);
                    data.insert(
                        Value::String("grpc-opts".to_string()),
                        Value::Mapping(grpc_opts),
                    );
                } else if !data.contains_key(Value::String("ws-opts".to_string())) {
                    set_string(&mut data, "path", path);
                }
            }
            if matches!(link.tls.as_deref(), Some("tls")) {
                set_bool(&mut data, "tls", true);
            }
            if let Some(sni) = link.sni.as_deref() {
                set_string(&mut data, "servername", sni);
            }
            if let Some(alpn) = link.alpn.as_deref() {
                set_string(&mut data, "alpn", alpn);
            }
            if let Some(fp) = link.fp.as_deref() {
                set_string(&mut data, "client-fingerprint", fp);
            }

            return Ok(ProxyNode {
                name: map_get_string(&data, "name").unwrap_or_else(|| link.add),
                ty: "vmess".to_string(),
                data,
            });
        }
    }

    parse_standard_uri(line)
}

fn populate_common_query_fields(url: &Url, data: &mut Mapping) {
    if let Some(network) = query_value(url, "type") {
        set_string(data, "network", &network);
    }
    if let Some(security) = query_value(url, "security") {
        if security == "tls" || security == "reality" {
            set_bool(data, "tls", true);
        }
        if security == "reality" {
            let mut reality_opts = Mapping::new();
            if let Some(public_key) = query_value(url, "pbk") {
                set_string(&mut reality_opts, "public-key", &public_key);
            }
            if let Some(short_id) = query_value(url, "sid") {
                set_string(&mut reality_opts, "short-id", &short_id);
            }
            if !reality_opts.is_empty() {
                data.insert(
                    Value::String("reality-opts".to_string()),
                    Value::Mapping(reality_opts),
                );
            }
        }
    }
    if let Some(sni) = query_value(url, "sni") {
        set_string(data, "servername", &sni);
    }
    if let Some(peer) = query_value(url, "peer") {
        set_string(data, "servername", &peer);
    }
    if let Some(alpn) = query_value(url, "alpn") {
        set_string(data, "alpn", &alpn);
    }
    if let Some(fp) = query_value(url, "fp").or_else(|| query_value(url, "fingerprint")) {
        set_string(data, "client-fingerprint", &fp);
    }
    if matches!(query_value(url, "allowInsecure").as_deref(), Some("1"))
        || matches!(query_value(url, "insecure").as_deref(), Some("1"))
    {
        set_bool(data, "skip-cert-verify", true);
    }
    if let Some(flow) = query_value(url, "flow") {
        set_string(data, "flow", &flow);
    }
    if let Some(host) = query_value(url, "host") {
        match query_value(url, "type").as_deref() {
            Some("ws") => {
                let mut ws_opts = Mapping::new();
                let mut headers = Mapping::new();
                set_string(&mut headers, "Host", &host);
                ws_opts.insert(
                    Value::String("headers".to_string()),
                    Value::Mapping(headers),
                );
                if let Some(path) = query_value(url, "path") {
                    set_string(&mut ws_opts, "path", &path);
                }
                data.insert(
                    Value::String("ws-opts".to_string()),
                    Value::Mapping(ws_opts),
                );
            }
            Some("grpc") => {
                set_string(data, "servername", &host);
            }
            _ => set_string(data, "host", &host),
        }
    }
    if let Some(path) = query_value(url, "path") {
        match query_value(url, "type").as_deref() {
            Some("grpc") => {
                let mut grpc_opts = Mapping::new();
                set_string(&mut grpc_opts, "grpc-service-name", &path);
                data.insert(
                    Value::String("grpc-opts".to_string()),
                    Value::Mapping(grpc_opts),
                );
            }
            Some("ws") => {
                let entry = data
                    .entry(Value::String("ws-opts".to_string()))
                    .or_insert_with(|| Value::Mapping(Mapping::new()));
                if let Value::Mapping(ws_opts) = entry {
                    set_string(ws_opts, "path", &path);
                }
            }
            _ => set_string(data, "path", &path),
        }
    }
    if let Some(service_name) = query_value(url, "serviceName") {
        let entry = data
            .entry(Value::String("grpc-opts".to_string()))
            .or_insert_with(|| Value::Mapping(Mapping::new()));
        if let Value::Mapping(grpc_opts) = entry {
            set_string(grpc_opts, "grpc-service-name", &service_name);
        }
    }
    if let Some(obfs_password) = query_value(url, "obfs-password") {
        set_string(data, "obfs-password", &obfs_password);
    }
    if let Some(pin_sha256) = query_value(url, "pinSHA256") {
        set_string(data, "pinSHA256", &pin_sha256);
    }
    if let Some(local_address) = query_value(url, "address") {
        set_string(data, "ip", &local_address);
    }
    if let Some(public_key) = query_value(url, "publickey").or_else(|| query_value(url, "peer")) {
        set_string(data, "public-key", &public_key);
    }
    if let Some(reserved) = query_value(url, "reserved") {
        set_string(data, "reserved", &reserved);
    }
    if let Some(mtu) = query_value(url, "mtu") {
        set_string(data, "mtu", &mtu);
    }
    if let Some(psk) = query_value(url, "presharedkey") {
        set_string(data, "pre-shared-key", &psk);
    }
}

fn scheme_alias(scheme: &str) -> String {
    match scheme {
        "socks5" => "socks5".to_string(),
        "https" => "http".to_string(),
        other => other.to_string(),
    }
}
