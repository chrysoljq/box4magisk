use anyhow::bail;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderEntry {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<String>,
}

pub(super) const SINGBOX_PROVIDERS_KEY: &str = "outbound_providers";

pub(super) fn validate_provider_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.contains(['\n', '\r', '\t']) {
        bail!("provider name is invalid");
    }
    Ok(())
}

pub(super) fn validate_provider_url(url: &str) -> anyhow::Result<()> {
    if url.is_empty() {
        bail!("provider url must not be empty");
    }
    Ok(())
}

pub(super) fn upsert_entry(
    entries: &mut Vec<ProviderEntry>,
    current_name: &str,
    next_name: &str,
    url: &str,
) {
    let mut updated = false;
    let mut next_entries = Vec::with_capacity(entries.len() + 1);

    for entry in entries.iter() {
        if entry.name == current_name {
            if !updated {
                next_entries.push(ProviderEntry {
                    name: next_name.to_string(),
                    url: url.to_string(),
                    provider_type: entry.provider_type.clone(),
                    update_time: entry.update_time.clone(),
                });
                updated = true;
            }
            continue;
        }
        if current_name != next_name && entry.name == next_name {
            continue;
        }
        next_entries.push(entry.clone());
    }

    if !updated {
        next_entries.push(ProviderEntry {
            name: next_name.to_string(),
            url: url.to_string(),
            provider_type: Some("remote".to_string()),
            update_time: None,
        });
    }

    *entries = next_entries;
}
