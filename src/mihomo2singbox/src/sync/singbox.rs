use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::shared::{
    ProviderEntry, SINGBOX_SUBSCRIPTIONS_KEY, upsert_entry, validate_provider_name,
    validate_provider_url,
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
        upsert_entry(&mut entries, name, name, url);
    }

    write_singbox_subscriptions(template_path, &entries)
}

pub fn list_singbox_subscriptions(template_path: &Path) -> Result<Vec<ProviderEntry>> {
    let root = read_singbox_template_json(template_path)?;
    parse_singbox_subscription_entries(&root)
}

pub fn upsert_singbox_subscription(
    template_path: &Path,
    current_name: Option<&str>,
    next_name: &str,
    url: &str,
) -> Result<()> {
    validate_provider_name(next_name)?;
    validate_provider_url(url)?;
    if let Some(current_name) = current_name {
        validate_provider_name(current_name)?;
    }

    let mut entries = list_singbox_subscriptions(template_path)?;
    upsert_entry(
        &mut entries,
        current_name.unwrap_or(next_name),
        next_name,
        url,
    );
    write_singbox_subscriptions(template_path, &entries)
}

pub fn remove_singbox_subscription(template_path: &Path, name: &str) -> Result<()> {
    validate_provider_name(name)?;

    let mut entries = list_singbox_subscriptions(template_path)?;
    entries.retain(|entry| entry.name != name);
    write_singbox_subscriptions(template_path, &entries)
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

fn parse_singbox_subscription_entries(root: &JsonValue) -> Result<Vec<ProviderEntry>> {
    let Some(map) = root.as_object() else {
        bail!("sing-box template root must be an object");
    };

    let Some(subscriptions) = map.get(SINGBOX_SUBSCRIPTIONS_KEY) else {
        return Ok(Vec::new());
    };

    let Some(items) = subscriptions.as_array() else {
        bail!("sing-box template `box_subscriptions` must be an array");
    };

    let mut entries = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            bail!(
                "sing-box template subscription at index {} must be an object",
                index
            );
        };
        let name = obj.get("name").and_then(JsonValue::as_str).ok_or_else(|| {
            anyhow!(
                "sing-box template subscription at index {} missing `name`",
                index
            )
        })?;
        let url = obj.get("url").and_then(JsonValue::as_str).ok_or_else(|| {
            anyhow!(
                "sing-box template subscription at index {} missing `url`",
                index
            )
        })?;
        validate_provider_name(name)?;
        validate_provider_url(url)?;
        entries.push(ProviderEntry {
            name: name.to_string(),
            url: url.to_string(),
        });
    }

    Ok(entries)
}

fn write_singbox_subscriptions(template_path: &Path, entries: &[ProviderEntry]) -> Result<()> {
    let mut root = read_singbox_template_json(template_path)?;
    let map = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("sing-box template root must be an object"))?;

    map.insert(
        SINGBOX_SUBSCRIPTIONS_KEY.to_string(),
        JsonValue::Array(
            entries
                .iter()
                .map(|entry| {
                    JsonValue::Object(JsonMap::from_iter([
                        ("name".to_string(), JsonValue::String(entry.name.clone())),
                        ("url".to_string(), JsonValue::String(entry.url.clone())),
                    ]))
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
  "box_subscriptions": [
    {
      "name": "provider1",
      "url": "https://one.example"
    },
    {
      "name": "provider2",
      "url": "https://two.example"
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
    fn lists_singbox_subscriptions_from_template() {
        let path = unique_path("singbox-subscription-list", "json");
        fs::write(&path, sample_singbox_template()).unwrap();
        let entries = list_singbox_subscriptions(&path).unwrap();
        assert_eq!(
            entries,
            vec![
                ProviderEntry {
                    name: "provider1".to_string(),
                    url: "https://one.example".to_string(),
                },
                ProviderEntry {
                    name: "provider2".to_string(),
                    url: "https://two.example".to_string(),
                }
            ]
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn updates_singbox_subscription_in_template() {
        let path = unique_path("singbox-subscription-update", "json");
        fs::write(&path, sample_singbox_template()).unwrap();
        upsert_singbox_subscription(
            &path,
            Some("provider1"),
            "provider1-renamed",
            "https://new.example",
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"name\": \"provider1-renamed\""));
        assert!(content.contains("\"url\": \"https://new.example\""));
        assert!(!content.contains("\"name\": \"provider1\""));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn removes_singbox_subscription_from_template() {
        let path = unique_path("singbox-subscription-remove", "json");
        fs::write(&path, sample_singbox_template()).unwrap();
        remove_singbox_subscription(&path, "provider2").unwrap();
        let entries = list_singbox_subscriptions(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "provider1");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn syncs_singbox_subscriptions_from_legacy_state() {
        let template_path = unique_path("singbox-subscription-sync-template", "json");
        let state_path = unique_path("singbox-subscription-sync-state", "tsv");
        fs::write(&template_path, sample_singbox_template()).unwrap();
        fs::write(
            &state_path,
            "provider3\thttps://three.example\nprovider4\thttps://four.example\n",
        )
        .unwrap();
        sync_singbox_subscriptions(&template_path, &state_path).unwrap();
        let entries = list_singbox_subscriptions(&template_path).unwrap();
        assert_eq!(
            entries,
            vec![
                ProviderEntry {
                    name: "provider3".to_string(),
                    url: "https://three.example".to_string(),
                },
                ProviderEntry {
                    name: "provider4".to_string(),
                    url: "https://four.example".to_string(),
                }
            ]
        );
        let _ = fs::remove_file(template_path);
        let _ = fs::remove_file(state_path);
    }
}
