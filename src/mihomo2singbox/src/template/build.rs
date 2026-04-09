use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::{list_singbox_subscriptions, parse_input, render_outbounds};

#[derive(Debug, Clone, Serialize)]
pub struct SingboxBuildSummary {
    pub subscriptions_total: usize,
    pub subscriptions_built: usize,
    pub subscriptions_failed: usize,
    pub outbounds_added: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SingboxSubscriptionNodeView {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SingboxSubscriptionView {
    pub name: String,
    pub url: String,
    pub cache_file: String,
    pub cache_exists: bool,
    pub cache_size: u64,
    pub status: String,
    pub warnings: Vec<String>,
    pub nodes: Vec<SingboxSubscriptionNodeView>,
}

pub fn build_singbox_config(
    template_path: &Path,
    subscriptions_dir: &Path,
    output_path: &Path,
) -> Result<SingboxBuildSummary> {
    let template_content = fs::read_to_string(template_path).with_context(|| {
        format!(
            "failed to read sing-box template {}",
            template_path.display()
        )
    })?;
    let mut root: Value =
        serde_json::from_str(&template_content).context("sing-box template json parse failed")?;
    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("sing-box template root must be an object"))?;

    let subscriptions = list_singbox_subscriptions(template_path)?;
    let mut warnings = Vec::new();
    let mut dynamic_outbounds = Vec::new();
    let mut dynamic_tags = Vec::new();
    let mut built_count = 0usize;
    let mut failed_count = 0usize;

    for subscription in &subscriptions {
        let cache_file = subscription_cache_file(subscriptions_dir, &subscription.name);
        let content = match fs::read_to_string(&cache_file) {
            Ok(content) => content,
            Err(error) => {
                failed_count += 1;
                warnings.push(format!(
                    "subscription `{}` cache missing at {}: {}",
                    subscription.name,
                    cache_file.display(),
                    error
                ));
                continue;
            }
        };

        match parse_input(&content) {
            Ok(parsed) => {
                warnings.extend(
                    parsed
                        .warnings
                        .iter()
                        .map(|warning| format!("subscription `{}`: {warning}", subscription.name)),
                );

                let filtered_nodes = parsed
                    .nodes
                    .into_iter()
                    .filter(|node| !is_subscription_metadata_tag(&node.name))
                    .collect::<Vec<_>>();

                if filtered_nodes.is_empty() {
                    failed_count += 1;
                    warnings.push(format!(
                        "subscription `{}` produced no usable proxy nodes",
                        subscription.name
                    ));
                    continue;
                }

                let rendered = render_outbounds(filtered_nodes, "", false, "");
                warnings.extend(
                    rendered
                        .warnings
                        .iter()
                        .map(|warning| format!("subscription `{}`: {warning}", subscription.name)),
                );

                for outbound in rendered.outbounds {
                    if let Some(tag) = outbound.get("tag").and_then(Value::as_str) {
                        dynamic_tags.push(tag.to_string());
                    }
                    dynamic_outbounds.push(outbound);
                }
                built_count += 1;
            }
            Err(error) => {
                failed_count += 1;
                warnings.push(format!(
                    "subscription `{}` parse failed: {error}",
                    subscription.name
                ));
            }
        }
    }

    root_obj.remove("box_subscriptions");
    let outbounds = root_obj
        .get_mut("outbounds")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("sing-box template missing `outbounds` array"))?;

    for outbound in outbounds.iter_mut() {
        expand_template_outbound(outbound, &dynamic_tags, &mut warnings)?;
    }

    outbounds.extend(dynamic_outbounds);

    let pretty = serde_json::to_string_pretty(&root)
        .context("failed to serialize generated sing-box config")?;
    fs::write(output_path, pretty)
        .with_context(|| format!("failed to write sing-box config {}", output_path.display()))?;

    Ok(SingboxBuildSummary {
        subscriptions_total: subscriptions.len(),
        subscriptions_built: built_count,
        subscriptions_failed: failed_count,
        outbounds_added: dynamic_tags.len(),
        warnings,
    })
}

pub fn inspect_singbox_subscriptions(
    template_path: &Path,
    subscriptions_dir: &Path,
) -> Result<Vec<SingboxSubscriptionView>> {
    let subscriptions = list_singbox_subscriptions(template_path)?;
    let mut items = Vec::with_capacity(subscriptions.len());

    for subscription in subscriptions {
        let cache_file = subscription_cache_file(subscriptions_dir, &subscription.name);
        let cache_exists = cache_file.exists();
        let cache_size = cache_file
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let (status, warnings, nodes) = if !cache_exists {
            (
                "missing_cache".to_string(),
                vec![format!("cache file missing: {}", cache_file.display())],
                Vec::new(),
            )
        } else {
            let content = fs::read_to_string(&cache_file).with_context(|| {
                format!("failed to read subscription cache {}", cache_file.display())
            })?;
            match parse_input(&content) {
                Ok(parsed) => {
                    let nodes = parsed
                        .nodes
                        .into_iter()
                        .filter(|node| !is_subscription_metadata_tag(&node.name))
                        .map(|node| SingboxSubscriptionNodeView {
                            name: node.name,
                            r#type: node.ty,
                        })
                        .collect::<Vec<_>>();
                    let status = if nodes.is_empty() { "empty" } else { "ok" }.to_string();
                    (status, parsed.warnings, nodes)
                }
                Err(error) => (
                    "parse_error".to_string(),
                    vec![error.to_string()],
                    Vec::new(),
                ),
            }
        };

        items.push(SingboxSubscriptionView {
            name: subscription.name,
            url: subscription.url,
            cache_file: cache_file.display().to_string(),
            cache_exists,
            cache_size,
            status,
            warnings,
            nodes,
        });
    }

    Ok(items)
}

fn expand_template_outbound(
    outbound: &mut Value,
    dynamic_tags: &[String],
    warnings: &mut Vec<String>,
) -> Result<()> {
    let Some(obj) = outbound.as_object_mut() else {
        return Ok(());
    };
    let tag_name = obj.get("tag").and_then(Value::as_str).map(str::to_string);

    let filter = compile_optional_regex(obj.remove("_filter"), "_filter", tag_name.as_deref())?;
    let exclude = compile_optional_regex(obj.remove("_exclude"), "_exclude", tag_name.as_deref())?;

    let Some(outbounds) = obj.get_mut("outbounds").and_then(Value::as_array_mut) else {
        return Ok(());
    };

    let mut next = Vec::new();
    for item in outbounds.iter() {
        let Some(raw) = item.as_str() else {
            next.push(item.clone());
            continue;
        };

        if raw != "{all}" {
            next.push(Value::String(raw.to_string()));
            continue;
        }

        let matched = dynamic_tags
            .iter()
            .filter(|tag| filter.as_ref().is_none_or(|re| re.is_match(tag)))
            .filter(|tag| exclude.as_ref().is_none_or(|re| !re.is_match(tag)))
            .map(|tag| Value::String(tag.clone()))
            .collect::<Vec<_>>();

        next.extend(matched);
    }

    if next.is_empty() {
        next.push(Value::String("direct".to_string()));
        if let Some(tag) = tag_name.as_deref() {
            warnings.push(format!(
                "template outbound `{tag}` had no dynamic matches, fell back to `direct`"
            ));
        }
    }

    *outbounds = next;
    Ok(())
}

fn compile_optional_regex(
    value: Option<Value>,
    field_name: &str,
    tag_name: Option<&str>,
) -> Result<Option<Regex>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(pattern) = value.as_str() else {
        bail!("{field_name} must be a string");
    };
    let regex = Regex::new(pattern).with_context(|| {
        let tag_name = tag_name.unwrap_or("<unknown>");
        format!("invalid regex in {field_name} for outbound `{tag_name}`")
    })?;
    Ok(Some(regex))
}

fn subscription_cache_file(subscriptions_dir: &Path, name: &str) -> PathBuf {
    let mut encoded = String::new();
    let mut last_was_underscore = false;
    for ch in name.chars() {
        let allowed = matches!(ch, 'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-');
        if allowed {
            encoded.push(ch);
            last_was_underscore = false;
            continue;
        }
        if !last_was_underscore {
            encoded.push('_');
            last_was_underscore = true;
        }
    }
    if encoded.is_empty() {
        encoded.push('_');
    }
    subscriptions_dir.join(format!("{encoded}.txt"))
}

fn is_subscription_metadata_tag(name: &str) -> bool {
    const MARKERS: [&str; 6] = [
        "剩余流量",
        "下次重置",
        "套餐到期",
        "温馨提示",
        "订阅",
        "流量",
    ];
    MARKERS.iter().any(|marker| name.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::{build_singbox_config, inspect_singbox_subscriptions, subscription_cache_file};
    use serde_json::Value;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("{prefix}-{nonce}"))
    }

    #[test]
    fn builds_singbox_config_from_template_and_cache() {
        let root = unique_dir("singbox-build");
        let subscriptions_dir = root.join("subscriptions");
        fs::create_dir_all(&subscriptions_dir).unwrap();

        let template_path = root.join("config.template.json");
        let output_path = root.join("config.json");
        fs::write(
            &template_path,
            r#"{
  "box_subscriptions": [
    { "name": "demo-sub", "url": "https://example.com/sub" }
  ],
  "outbounds": [
    { "tag": "direct", "type": "direct" },
    { "tag": "group-a", "type": "selector", "outbounds": ["{all}"], "_filter": "HK|SG" },
    { "tag": "group-b", "type": "selector", "outbounds": ["{all}"], "_exclude": "HK" }
  ]
}"#,
        )
        .unwrap();

        let cache_path = subscription_cache_file(&subscriptions_dir, "demo-sub");
        fs::write(
            &cache_path,
            "ss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLmNvbTo0NDM=#HK-01\nss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLm5ldDo0NDM=#SG-02\nss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLm9yZzo0NDM=#US-03\n",
        )
        .unwrap();

        let summary =
            build_singbox_config(&template_path, &subscriptions_dir, &output_path).unwrap();
        assert_eq!(summary.subscriptions_total, 1);
        assert_eq!(summary.subscriptions_built, 1);
        assert_eq!(summary.subscriptions_failed, 0);

        let generated: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();
        assert!(generated.get("box_subscriptions").is_none());

        let outbounds = generated["outbounds"].as_array().unwrap();
        let group_a = outbounds
            .iter()
            .find(|item| item["tag"] == "group-a")
            .unwrap();
        let group_b = outbounds
            .iter()
            .find(|item| item["tag"] == "group-b")
            .unwrap();
        let group_a_items = group_a["outbounds"].as_array().unwrap();
        let group_b_items = group_b["outbounds"].as_array().unwrap();

        assert_eq!(group_a_items.len(), 2);
        assert!(group_a_items.iter().any(|item| item == "HK-01"));
        assert!(group_a_items.iter().any(|item| item == "SG-02"));
        assert_eq!(group_b_items.len(), 2);
        assert!(group_b_items.iter().any(|item| item == "SG-02"));
        assert!(group_b_items.iter().any(|item| item == "US-03"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspects_singbox_subscriptions_from_cache() {
        let root = unique_dir("singbox-inspect");
        let subscriptions_dir = root.join("subscriptions");
        fs::create_dir_all(&subscriptions_dir).unwrap();

        let template_path = root.join("config.template.json");
        fs::write(
            &template_path,
            r#"{
  "box_subscriptions": [
    { "name": "demo-sub", "url": "https://example.com/sub" }
  ],
  "outbounds": []
}"#,
        )
        .unwrap();

        let cache_path = subscription_cache_file(&subscriptions_dir, "demo-sub");
        fs::write(
            &cache_path,
            "ss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLmNvbTo0NDM=#HK-01\nss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLm5ldDo0NDM=#US-02\n",
        )
        .unwrap();

        let items = inspect_singbox_subscriptions(&template_path, &subscriptions_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "ok");
        assert_eq!(items[0].nodes.len(), 2);
        assert_eq!(items[0].nodes[0].name, "HK-01");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_filename_matches_shell_squeezing_behavior() {
        let dir = std::path::Path::new("/tmp/demo");
        let path = subscription_cache_file(dir, "香港 节点#1");
        assert_eq!(path, std::path::PathBuf::from("/tmp/demo/_1.txt"));
    }
}
