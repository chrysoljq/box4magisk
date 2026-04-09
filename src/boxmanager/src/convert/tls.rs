use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::ProxyNode;

use super::{
    map_get_bool, map_get_mapping, map_get_string, map_get_string_list, object, required_string,
};

#[derive(Debug, Serialize)]
struct TlsConfig {
    enabled: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    disable_sni: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_name: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
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
    let config = TlsConfig {
        enabled: true,
        disable_sni: map_get_bool(&node.data, "disable-sni")
            || map_get_bool(&node.data, "disable_sni"),
        server_name: map_get_string(&node.data, "servername")
            .or_else(|| map_get_string(&node.data, "sni")),
        insecure: map_get_bool(&node.data, "skip-cert-verify"),
        alpn: map_get_string_list(&node.data, "alpn"),
        min_version: map_get_string(&node.data, "tls-min-version")
            .or_else(|| map_get_string(&node.data, "min_version")),
        max_version: map_get_string(&node.data, "tls-max-version")
            .or_else(|| map_get_string(&node.data, "max_version")),
        cipher_suites: map_get_string_list(&node.data, "cipher-suites")
            .or_else(|| map_get_string_list(&node.data, "cipher_suites")),
        curve_preferences: map_get_string_list(&node.data, "curve-preferences")
            .or_else(|| map_get_string_list(&node.data, "curve_preferences")),
        certificate: map_get_string(&node.data, "certificate"),
        certificate_path: map_get_string(&node.data, "certificate-path")
            .or_else(|| map_get_string(&node.data, "certificate_path")),
        certificate_public_key_sha256: map_get_string_list(
            &node.data,
            "certificate-public-key-sha256",
        )
        .or_else(|| map_get_string_list(&node.data, "certificate_public_key_sha256")),
        client_certificate: map_get_string_list(&node.data, "client-certificate")
            .or_else(|| map_get_string_list(&node.data, "client_certificate")),
        client_certificate_path: map_get_string(&node.data, "client-certificate-path")
            .or_else(|| map_get_string(&node.data, "client_certificate_path")),
        client_key: map_get_string_list(&node.data, "client-key")
            .or_else(|| map_get_string_list(&node.data, "client_key")),
        client_key_path: map_get_string(&node.data, "client-key-path")
            .or_else(|| map_get_string(&node.data, "client_key_path")),
        utls: build_utls(node),
        reality: build_reality(node)?,
    };

    let value = serde_json::to_value(config)?;
    Ok(value
        .as_object()
        .cloned()
        .unwrap_or_else(|| object(Vec::new())))
}

fn build_utls(node: &ProxyNode) -> Option<TlsUtlsConfig> {
    map_get_string(&node.data, "client-fingerprint").map(|fingerprint| TlsUtlsConfig {
        enabled: true,
        fingerprint,
    })
}

fn build_reality(node: &ProxyNode) -> Result<Option<TlsRealityConfig>> {
    let Some(reality) = map_get_mapping(&node.data, "reality-opts") else {
        return Ok(None);
    };

    Ok(Some(TlsRealityConfig {
        enabled: true,
        public_key: required_string(reality, "public-key")
            .context("missing reality-opts.public-key")?,
        short_id: map_get_string(reality, "short-id").unwrap_or_default(),
    }))
}
