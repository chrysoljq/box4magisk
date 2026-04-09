use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::{list_singbox_subscriptions, parse_input, render_outbounds};

const PROVIDERS_KEY: &str = "outbound_providers";

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

/// Per-provider resolved data: node tags belonging to this provider.
struct ProviderNodes {
    /// Node tags belonging to this provider.
    tags: Vec<String>,
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
    let mut built_count = 0usize;
    let mut failed_count = 0usize;

    // Build per-provider node maps: provider_tag -> ProviderNodes
    let mut provider_map: HashMap<String, ProviderNodes> = HashMap::new();
    let mut all_dynamic_outbounds: Vec<Value> = Vec::new();
    let mut total_tags = 0usize;

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

                let mut tags = Vec::with_capacity(rendered.outbounds.len());
                for outbound in &rendered.outbounds {
                    if let Some(tag) = outbound.get("tag").and_then(Value::as_str) {
                        tags.push(tag.to_string());
                    }
                }

                total_tags += tags.len();
                provider_map.insert(
                    subscription.name.clone(),
                    ProviderNodes {
                        tags,
                    },
                );
                all_dynamic_outbounds.extend(rendered.outbounds);
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

    root_obj.remove(PROVIDERS_KEY);

    let outbounds = root_obj
        .get_mut("outbounds")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("sing-box template missing `outbounds` array"))?;

    // Expand template outbounds with dynamic nodes
    for outbound in outbounds.iter_mut() {
        expand_template_outbound(outbound, &provider_map, &mut warnings)?;
    }

    // Append all dynamic outbound objects (the actual proxy node definitions)
    outbounds.extend(all_dynamic_outbounds);

    let pretty = serde_json::to_string_pretty(&root)
        .context("failed to serialize generated sing-box config")?;
    fs::write(output_path, pretty)
        .with_context(|| format!("failed to write sing-box config {}", output_path.display()))?;

    Ok(SingboxBuildSummary {
        subscriptions_total: subscriptions.len(),
        subscriptions_built: built_count,
        subscriptions_failed: failed_count,
        outbounds_added: total_tags,
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

/// Expand a template outbound by resolving `include`, `filter`, `exclude_filter`.
///
/// Logic:
/// 1. If the outbound has `include`, `filter`, or `exclude_filter`, it's a template outbound.
/// 2. Determine provider scope from `include` (or all providers if omitted).
/// 3. Collect node tags from those providers.
/// 4. Apply `filter` (keep matching) and `exclude_filter` (remove matching) regexes.
/// 5. Append matched tags to existing `outbounds` array (creating it if needed).
/// 6. Remove template-specific fields from the outbound object.
fn expand_template_outbound(
    outbound: &mut Value,
    provider_map: &HashMap<String, ProviderNodes>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let Some(obj) = outbound.as_object_mut() else {
        return Ok(());
    };
    let tag_name = obj.get("tag").and_then(Value::as_str).map(str::to_string);

    // Extract template-specific fields (remove from output)
    let include_val = obj.remove("include");
    let filter_val = obj.remove("filter");
    let exclude_filter_val = obj.remove("exclude_filter");

    // If none of the template fields exist, this is a plain outbound — skip
    if include_val.is_none() && filter_val.is_none() && exclude_filter_val.is_none() {
        return Ok(());
    }

    // Parse include list
    let include_providers: Option<Vec<String>> = include_val.map(|val| {
        val.as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    });

    // Compile filter regexes
    let filter = compile_optional_regex(filter_val, "filter", tag_name.as_deref())?;
    let exclude_filter =
        compile_optional_regex(exclude_filter_val, "exclude_filter", tag_name.as_deref())?;

    // Collect candidate node tags from specified providers (or all)
    let candidate_tags: Vec<&str> = match &include_providers {
        Some(provider_names) => {
            let mut tags = Vec::new();
            for name in provider_names {
                if let Some(provider) = provider_map.get(name) {
                    tags.extend(provider.tags.iter().map(String::as_str));
                } else {
                    if let Some(tag) = tag_name.as_deref() {
                        warnings.push(format!(
                            "outbound `{tag}`: include references unknown provider `{name}`"
                        ));
                    }
                }
            }
            tags
        }
        None => {
            // No include specified → use all providers
            provider_map
                .values()
                .flat_map(|p| p.tags.iter().map(String::as_str))
                .collect()
        }
    };

    // Apply filter and exclude_filter
    let matched: Vec<Value> = candidate_tags
        .into_iter()
        .filter(|tag| filter.as_ref().is_none_or(|re| re.is_match(tag)))
        .filter(|tag| exclude_filter.as_ref().is_none_or(|re| !re.is_match(tag)))
        .map(|tag| Value::String(tag.to_string()))
        .collect();

    // Get or create the outbounds array, then append matched tags
    let outbounds_arr = obj
        .entry("outbounds")
        .or_insert_with(|| Value::Array(Vec::new()));

    if let Some(arr) = outbounds_arr.as_array_mut() {
        arr.extend(matched);

        // If still empty after expansion, fall back to "direct"
        if arr.is_empty() {
            arr.push(Value::String("direct".to_string()));
            if let Some(tag) = tag_name.as_deref() {
                warnings.push(format!(
                    "template outbound `{tag}` had no dynamic matches, fell back to `direct`"
                ));
            }
        }
    }

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
    fn builds_singbox_config_with_include_and_filter() {
        let root = unique_dir("singbox-build-new");
        let subscriptions_dir = root.join("subscriptions");
        fs::create_dir_all(&subscriptions_dir).unwrap();

        let template_path = root.join("config.template.json");
        let output_path = root.join("config.json");

        // New format template:
        // - group-asia: include only demo-sub, filter HK|SG
        // - group-all: no include (all providers), no filter
        // - group-static: has outbounds ["group-asia"], include demo-sub, filter US
        //   → final outbounds = ["group-asia", ...US nodes]
        fs::write(
            &template_path,
            r#"{
  "outbound_providers": [
    { "tag": "demo-sub", "type": "remote", "url": "https://example.com/sub", "update_time": "" }
  ],
  "outbounds": [
    { "tag": "direct", "type": "direct" },
    { "tag": "group-asia", "type": "urltest", "include": ["demo-sub"], "filter": "HK|SG", "url": "https://www.gstatic.com/generate_204", "interval": "10m" },
    { "tag": "group-exclude", "type": "selector", "exclude_filter": "HK" },
    { "tag": "group-static", "type": "selector", "outbounds": ["group-asia"], "include": ["demo-sub"], "filter": "US" }
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
        // Provider definitions should be removed from output
        assert!(generated.get("outbound_providers").is_none());
        assert!(generated.get("box_subscriptions").is_none());

        let outbounds = generated["outbounds"].as_array().unwrap();

        // group-asia: include demo-sub, filter HK|SG → ["HK-01", "SG-02"]
        let group_asia = outbounds
            .iter()
            .find(|item| item["tag"] == "group-asia")
            .unwrap();
        let asia_items = group_asia["outbounds"].as_array().unwrap();
        assert_eq!(asia_items.len(), 2);
        assert!(asia_items.iter().any(|item| item == "HK-01"));
        assert!(asia_items.iter().any(|item| item == "SG-02"));

        // group-exclude: no include (all), exclude HK → ["SG-02", "US-03"]
        let group_exclude = outbounds
            .iter()
            .find(|item| item["tag"] == "group-exclude")
            .unwrap();
        let exclude_items = group_exclude["outbounds"].as_array().unwrap();
        assert_eq!(exclude_items.len(), 2);
        assert!(exclude_items.iter().any(|item| item == "SG-02"));
        assert!(exclude_items.iter().any(|item| item == "US-03"));

        // group-static: outbounds=["group-asia"] + include demo-sub, filter US → ["group-asia", "US-03"]
        let group_static = outbounds
            .iter()
            .find(|item| item["tag"] == "group-static")
            .unwrap();
        let static_items = group_static["outbounds"].as_array().unwrap();
        assert_eq!(static_items.len(), 2);
        assert_eq!(static_items[0], "group-asia");
        assert!(static_items.iter().any(|item| item == "US-03"));

        // Template-specific fields should be removed
        assert!(group_asia.get("include").is_none());
        assert!(group_asia.get("filter").is_none());
        assert!(group_exclude.get("exclude_filter").is_none());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn builds_with_multiple_providers_and_selective_include() {
        let root = unique_dir("singbox-build-multi");
        let subscriptions_dir = root.join("subscriptions");
        fs::create_dir_all(&subscriptions_dir).unwrap();

        let template_path = root.join("config.template.json");
        let output_path = root.join("config.json");

        fs::write(
            &template_path,
            r#"{
  "outbound_providers": [
    { "tag": "sub-a", "type": "remote", "url": "https://a.example", "update_time": "" },
    { "tag": "sub-b", "type": "remote", "url": "https://b.example", "update_time": "" }
  ],
  "outbounds": [
    { "tag": "direct", "type": "direct" },
    { "tag": "only-a", "type": "urltest", "include": ["sub-a"], "url": "https://www.gstatic.com/generate_204", "interval": "10m" },
    { "tag": "only-b", "type": "urltest", "include": ["sub-b"], "url": "https://www.gstatic.com/generate_204", "interval": "10m" },
    { "tag": "all-subs", "type": "selector", "filter": ".*" }
  ]
}"#,
        )
        .unwrap();

        let cache_a = subscription_cache_file(&subscriptions_dir, "sub-a");
        fs::write(
            &cache_a,
            "ss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLmNvbTo0NDM=#A-Node1\n",
        )
        .unwrap();

        let cache_b = subscription_cache_file(&subscriptions_dir, "sub-b");
        fs::write(
            &cache_b,
            "ss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLm5ldDo0NDM=#B-Node1\nss://YWVzLTI1Ni1nY206cGFzc0BleGFtcGxlLm9yZzo0NDM=#B-Node2\n",
        )
        .unwrap();

        let summary =
            build_singbox_config(&template_path, &subscriptions_dir, &output_path).unwrap();
        assert_eq!(summary.subscriptions_built, 2);

        let generated: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();
        let outbounds = generated["outbounds"].as_array().unwrap();

        // only-a: should have only A-Node1
        let only_a = outbounds
            .iter()
            .find(|item| item["tag"] == "only-a")
            .unwrap();
        let only_a_items = only_a["outbounds"].as_array().unwrap();
        assert_eq!(only_a_items.len(), 1);
        assert_eq!(only_a_items[0], "A-Node1");

        // only-b: should have B-Node1, B-Node2
        let only_b = outbounds
            .iter()
            .find(|item| item["tag"] == "only-b")
            .unwrap();
        let only_b_items = only_b["outbounds"].as_array().unwrap();
        assert_eq!(only_b_items.len(), 2);

        // all-subs: should have all 3 nodes
        let all_subs = outbounds
            .iter()
            .find(|item| item["tag"] == "all-subs")
            .unwrap();
        let all_items = all_subs["outbounds"].as_array().unwrap();
        assert_eq!(all_items.len(), 3);

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
  "outbound_providers": [
    { "tag": "demo-sub", "type": "remote", "url": "https://example.com/sub", "update_time": "" }
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

    #[test]
    fn plain_outbound_without_template_fields_is_untouched() {
        let root = unique_dir("singbox-plain");
        let subscriptions_dir = root.join("subscriptions");
        fs::create_dir_all(&subscriptions_dir).unwrap();

        let template_path = root.join("config.template.json");
        let output_path = root.join("config.json");

        fs::write(
            &template_path,
            r#"{
  "outbound_providers": [],
  "outbounds": [
    { "tag": "direct", "type": "direct" },
    { "tag": "manual", "type": "selector", "outbounds": ["direct"], "default": "direct" }
  ]
}"#,
        )
        .unwrap();

        build_singbox_config(&template_path, &subscriptions_dir, &output_path).unwrap();

        let generated: Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();
        let outbounds = generated["outbounds"].as_array().unwrap();

        let manual = outbounds
            .iter()
            .find(|item| item["tag"] == "manual")
            .unwrap();
        // outbounds should be preserved as-is
        let items = manual["outbounds"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], "direct");

        let _ = fs::remove_dir_all(root);
    }
}
