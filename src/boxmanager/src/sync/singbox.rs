use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::shared::{
    ProviderEntry, SINGBOX_PROVIDERS_KEY, upsert_entry, validate_provider_name,
    validate_provider_type, validate_provider_url,
};

pub fn sync_singbox_subscriptions(template_path: &Path, state_path: &Path) -> Result<()> {
    let state = fs::read_to_string(state_path)
        .with_context(|| format!("failed to read subscription state {}", state_path.display()))?;
    let mut entries = Vec::new();

    for (index, line) in state.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((name, url)) = line.split_once('\t') else {
            bail!(
                "invalid subscription state at line {}: expected tab-separated name and url",
                index + 1
            );
        };
        validate_provider_name(name)?;
        validate_provider_url(url)?;
        upsert_entry(&mut entries, name, name, url, "remote", None);
    }

    write_singbox_providers(template_path, &entries)
}

pub fn list_singbox_subscriptions(template_path: &Path) -> Result<Vec<ProviderEntry>> {
    let root = read_singbox_template_json(template_path)?;
    parse_singbox_provider_entries(&root)
}

pub fn upsert_singbox_subscription(
    template_path: &Path,
    current_name: Option<&str>,
    next_name: &str,
    url: &str,
    provider_type: &str,
    update_time: Option<String>,
) -> Result<()> {
    validate_provider_name(next_name)?;
    validate_provider_url(url)?;
    validate_provider_type(provider_type)?;
    if let Some(current_name) = current_name {
        validate_provider_name(current_name)?;
    }

    let mut entries = list_singbox_subscriptions(template_path)?;
    upsert_entry(
        &mut entries,
        current_name.unwrap_or(next_name),
        next_name,
        url,
        provider_type,
        update_time,
    );
    write_singbox_providers(template_path, &entries)
}

pub fn remove_singbox_subscription(template_path: &Path, name: &str) -> Result<()> {
    validate_provider_name(name)?;

    let mut entries = list_singbox_subscriptions(template_path)?;
    entries.retain(|entry| entry.name != name);
    write_singbox_providers(template_path, &entries)
}

fn read_singbox_template_json(template_path: &Path) -> Result<JsonValue> {
    let content = fs::read_to_string(template_path).with_context(|| {
        format!(
            "failed to read sing-box template {}",
            template_path.display()
        )
    })?;
    let root: JsonValue =
        serde_json::from_str(&content).context("sing-box template json parse failed")?;
    if !root.is_object() {
        bail!("sing-box template root must be an object");
    }
    Ok(root)
}

/// Parse provider entries from `outbound_providers` array.
/// Each entry: { "tag": "...", "type": "...", "url": "...", "update_time": "..." }
fn parse_singbox_provider_entries(root: &JsonValue) -> Result<Vec<ProviderEntry>> {
    let Some(map) = root.as_object() else {
        bail!("sing-box template root must be an object");
    };

    let Some(providers) = map.get(SINGBOX_PROVIDERS_KEY) else {
        return Ok(Vec::new());
    };

    let Some(items) = providers.as_array() else {
        bail!("sing-box template `outbound_providers` must be an array");
    };

    let mut entries = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            bail!(
                "sing-box template provider at index {} must be an object",
                index
            );
        };

        let tag = obj.get("tag").and_then(JsonValue::as_str).ok_or_else(|| {
            anyhow!(
                "sing-box template provider at index {} missing `tag`",
                index
            )
        })?;
        let url = obj.get("url").and_then(JsonValue::as_str).ok_or_else(|| {
            anyhow!(
                "sing-box template provider at index {} missing `url`",
                index
            )
        })?;
        let provider_type = obj.get("type").and_then(JsonValue::as_str);
        let update_time = obj.get("update_time").and_then(JsonValue::as_str);

        validate_provider_name(tag)?;
        validate_provider_url(url)?;
        entries.push(ProviderEntry {
            name: tag.to_string(),
            url: url.to_string(),
            provider_type: provider_type.map(str::to_string),
            update_time: update_time.map(str::to_string),
        });
    }

    Ok(entries)
}

fn write_singbox_providers(template_path: &Path, entries: &[ProviderEntry]) -> Result<()> {
    let mut root = read_singbox_template_json(template_path)?;
    let map = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("sing-box template root must be an object"))?;

    map.insert(
        SINGBOX_PROVIDERS_KEY.to_string(),
        JsonValue::Array(
            entries
                .iter()
                .map(|entry| {
                    let mut obj = JsonMap::new();
                    obj.insert("tag".to_string(), JsonValue::String(entry.name.clone()));
                    obj.insert(
                        "type".to_string(),
                        JsonValue::String(
                            entry
                                .provider_type
                                .clone()
                                .unwrap_or_else(|| "remote".to_string()),
                        ),
                    );
                    obj.insert("url".to_string(), JsonValue::String(entry.url.clone()));
                    obj.insert(
                        "update_time".to_string(),
                        JsonValue::String(entry.update_time.clone().unwrap_or_default()),
                    );
                    JsonValue::Object(obj)
                })
                .collect(),
        ),
    );

    let pretty = serde_json::to_string_pretty(&root)
        .context("failed to serialize sing-box template json")?;
    fs::write(template_path, pretty).with_context(|| {
        format!(
            "failed to write sing-box template {}",
            template_path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_singbox_template() -> &'static str {
        r#"{
  "log": {
    "level": "info"
  },
  "outbound_providers": [
    {
      "tag": "provider1",
      "type": "remote",
      "url": "https://one.example",
      "update_time": ""
    },
    {
      "tag": "provider2",
      "type": "local",
      "url": "https://two.example",
      "update_time": ""
    }
  ],
  "outbounds": [
    {
      "tag": "direct",
      "type": "direct"
    }
  ]
}"#
    }

    fn unique_path(prefix: &str, ext: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("{prefix}-{nonce}.{ext}"))
    }

    #[test]
    fn lists_singbox_providers() {
        let path = unique_path("singbox-list", "json");
        fs::write(&path, sample_singbox_template()).unwrap();
        let entries = list_singbox_subscriptions(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "provider1");
        assert_eq!(entries[0].provider_type.as_deref(), Some("remote"));
        assert_eq!(entries[1].name, "provider2");
        assert_eq!(entries[1].provider_type.as_deref(), Some("local"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn updates_singbox_provider() {
        let path = unique_path("singbox-update", "json");
        fs::write(&path, sample_singbox_template()).unwrap();
        upsert_singbox_subscription(
            &path,
            Some("provider1"),
            "provider1-renamed",
            "https://new.example",
            "local",
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"tag\": \"provider1-renamed\""));
        assert!(content.contains("\"url\": \"https://new.example\""));
        assert!(content.contains("\"type\": \"local\""));
        assert!(!content.contains("\"tag\": \"provider1\""));
        assert!(content.contains("outbound_providers"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn removes_singbox_provider() {
        let path = unique_path("singbox-remove", "json");
        fs::write(&path, sample_singbox_template()).unwrap();
        remove_singbox_subscription(&path, "provider2").unwrap();
        let entries = list_singbox_subscriptions(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "provider1");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn syncs_singbox_providers_from_state() {
        let template_path = unique_path("singbox-sync-template", "json");
        let state_path = unique_path("singbox-sync-state", "tsv");
        fs::write(&template_path, sample_singbox_template()).unwrap();
        fs::write(
            &state_path,
            "provider3\thttps://three.example\nprovider4\thttps://four.example\n",
        )
        .unwrap();
        sync_singbox_subscriptions(&template_path, &state_path).unwrap();
        let entries = list_singbox_subscriptions(&template_path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "provider3");
        assert_eq!(entries[1].name, "provider4");
        let _ = fs::remove_file(template_path);
        let _ = fs::remove_file(state_path);
    }
}
