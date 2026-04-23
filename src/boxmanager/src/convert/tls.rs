use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::ProxyNode;

use super::{
    map_get_bool, map_get_mapping, map_get_string, map_get_string_list, object, required_string,
};

fn is_false(b: &bool) -> bool {
    !(*b)
}

fn get_first_string(data: &serde_yaml::Mapping, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|&k| map_get_string(data, k))
}

fn get_first_string_list(data: &serde_yaml::Mapping, keys: &[&str]) -> Option<Vec<String>> {
    keys.iter().find_map(|&k| map_get_string_list(data, k))
}

fn get_first_bool(data: &serde_yaml::Mapping, keys: &[&str]) -> bool {
    keys.iter().any(|&k| map_get_bool(data, k))
}

#[derive(Debug, Serialize)]
struct TlsConfig {
    enabled: bool,
    #[serde(skip_serializing_if = "is_false")]
    disable_sni: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_name: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    insecure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cipher_suites: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    curve_preferences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    certificate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    certificate_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    certificate_public_key_sha256: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_certificate: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_certificate_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_key: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    utls: Option<TlsUtlsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reality: Option<TlsRealityConfig>,
}

#[derive(Debug, Serialize)]
struct TlsUtlsConfig {
    enabled: bool,
    fingerprint: String,
}

#[derive(Debug, Serialize)]
struct TlsRealityConfig {
    enabled: bool,
    public_key: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    short_id: String,
}

pub(super) fn build_optional_tls(node: &ProxyNode) -> Result<Option<Map<String, Value>>> {
    let enabled = map_get_bool(&node.data, "tls")
        || map_get_bool(&node.data, "udp-over-tcp")
        || map_get_mapping(&node.data, "reality-opts").is_some();
    if !enabled {
        return Ok(None);
    }

    Ok(Some(build_required_tls(node)?))
}

pub(super) fn build_required_tls(node: &ProxyNode) -> Result<Map<String, Value>> {
    build_tls_from_data(&node.data, None)
}

fn build_tls_from_data(
    data: &serde_yaml::Mapping,
    sni_override: Option<&str>, // 移除了无用的 &serde_yaml::Mapping 引用
) -> Result<Map<String, Value>> {
    let config = TlsConfig {
        enabled: true,
        // 使用辅助函数大幅简化了中划线/下划线兼容的样板代码
        disable_sni: get_first_bool(data, &["disable-sni", "disable_sni"]),
        server_name: sni_override
            .map(|sni| sni.to_string())
            .or_else(|| get_first_string(data, &["servername", "sni"])),
        insecure: map_get_bool(data, "skip-cert-verify"),
        alpn: map_get_string_list(data, "alpn"),
        min_version: get_first_string(data, &["tls-min-version", "min_version"]),
        max_version: get_first_string(data, &["tls-max-version", "max_version"]),
        cipher_suites: get_first_string_list(data, &["cipher-suites", "cipher_suites"]),
        curve_preferences: get_first_string_list(data, &["curve-preferences", "curve_preferences"]),
        certificate: map_get_string(data, "certificate"),
        certificate_path: get_first_string(data, &["certificate-path", "certificate_path"]),
        certificate_public_key_sha256: get_first_string_list(data, &["certificate-public-key-sha256", "certificate_public_key_sha256"]),
        client_certificate: get_first_string_list(data, &["client-certificate", "client_certificate"]),
        client_certificate_path: get_first_string(data, &["client-certificate-path", "client_certificate_path"]),
        client_key: get_first_string_list(data, &["client-key", "client_key"]),
        client_key_path: get_first_string(data, &["client-key-path", "client_key_path"]),
        utls: build_utls_from_data(data),
        reality: build_reality_from_data(data)?,
    };

    let value = serde_json::to_value(config)?;
    Ok(value
        .as_object()
        .cloned()
        .unwrap_or_else(|| object(Vec::new())))
}

fn build_utls_from_data(data: &serde_yaml::Mapping) -> Option<TlsUtlsConfig> {
    map_get_string(data, "client-fingerprint").map(|fingerprint| TlsUtlsConfig {
        enabled: true,
        fingerprint,
    })
}

fn build_reality_from_data(data: &serde_yaml::Mapping) -> Result<Option<TlsRealityConfig>> {
    let Some(reality) = map_get_mapping(data, "reality-opts") else {
        return Ok(None);
    };

    Ok(Some(TlsRealityConfig {
        enabled: true,
        public_key: required_string(reality, "public-key")
            .context("missing reality-opts.public-key")?,
        short_id: map_get_string(reality, "short-id").unwrap_or_default(),
    }))
}