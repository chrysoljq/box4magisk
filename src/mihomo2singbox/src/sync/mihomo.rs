use std::fs;
use std::ops::Range;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_yaml::{Mapping, Value};

use super::shared::{ProviderEntry, validate_provider_name, validate_provider_url};

#[derive(Debug, Clone)]
struct ProviderBlock {
    name: String,
    key_line: usize,
    line_range: Range<usize>,
    indent: usize,
}

pub fn sync_mihomo_proxy_providers(config_path: &Path, state_path: &Path) -> Result<()> {
    let state = fs::read_to_string(state_path)
        .with_context(|| format!("failed to read subscription state {}", state_path.display()))?;
    let mut content = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read mihomo config {}", config_path.display()))?;
    validate_mihomo_config(&content)?;

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
        content = set_provider_in_text(&content, name, name, url)?;
    }

    fs::write(config_path, content)
        .with_context(|| format!("failed to write mihomo config {}", config_path.display()))?;
    Ok(())
}

pub fn list_mihomo_providers(config_path: &Path) -> Result<Vec<ProviderEntry>> {
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read mihomo config {}", config_path.display()))?;
    validate_mihomo_config(&content)?;

    let lines = content.lines().collect::<Vec<_>>();
    let section = find_proxy_providers_section(&lines)?;
    let providers = parse_provider_blocks(&lines, section)?;
    let mut entries = Vec::new();

    for provider in providers {
        if let Some(url) = find_provider_url(&lines, &provider)? {
            entries.push(ProviderEntry {
                name: provider.name,
                url,
            });
        }
    }

    Ok(entries)
}

pub fn upsert_mihomo_provider(
    config_path: &Path,
    current_name: Option<&str>,
    next_name: &str,
    url: &str,
) -> Result<()> {
    validate_provider_name(next_name)?;
    validate_provider_url(url)?;
    if let Some(current_name) = current_name {
        validate_provider_name(current_name)?;
    }

    let content = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read mihomo config {}", config_path.display()))?;
    validate_mihomo_config(&content)?;

    let next = set_provider_in_text(&content, current_name.unwrap_or(next_name), next_name, url)?;
    fs::write(config_path, next)
        .with_context(|| format!("failed to write mihomo config {}", config_path.display()))?;
    Ok(())
}

pub fn remove_mihomo_provider(config_path: &Path, name: &str) -> Result<()> {
    validate_provider_name(name)?;

    let content = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read mihomo config {}", config_path.display()))?;
    validate_mihomo_config(&content)?;

    let next = remove_provider_from_text(&content, name)?;
    fs::write(config_path, next)
        .with_context(|| format!("failed to write mihomo config {}", config_path.display()))?;
    Ok(())
}

fn validate_mihomo_config(content: &str) -> Result<()> {
    let root: Value = serde_yaml::from_str(content).context("mihomo config yaml parse failed")?;
    let Value::Mapping(root_map) = root else {
        bail!("mihomo config root must be a mapping");
    };

    let providers = root_map
        .get(Value::String("proxy-providers".to_string()))
        .ok_or_else(|| anyhow!("mihomo config missing `proxy-providers` section"))?;

    if !matches!(providers, Value::Mapping(_)) {
        bail!("mihomo config `proxy-providers` must be a mapping");
    }

    Ok(())
}

fn set_provider_in_text(
    content: &str,
    current_name: &str,
    next_name: &str,
    url: &str,
) -> Result<String> {
    let lines = content.lines().collect::<Vec<_>>();
    let section = find_proxy_providers_section(&lines)?;
    let providers = parse_provider_blocks(&lines, section.clone())?;

    if let Some(provider) = providers
        .iter()
        .find(|provider| provider.name == current_name)
        .cloned()
    {
        update_provider_block(&lines, &providers, &provider, next_name, url)
    } else {
        let template = detect_provider_template(content)?;
        insert_provider_block(
            &lines,
            section,
            providers,
            next_name,
            url,
            template.as_ref(),
        )
    }
}

fn remove_provider_from_text(content: &str, name: &str) -> Result<String> {
    let lines = content.lines().collect::<Vec<_>>();
    let section = find_proxy_providers_section(&lines)?;
    let providers = parse_provider_blocks(&lines, section)?;

    let Some(provider) = providers.iter().find(|provider| provider.name == name) else {
        return Ok(content.to_string());
    };

    let mut out = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        if provider.line_range.contains(&index) {
            continue;
        }
        out.push((*line).to_string());
    }
    Ok(finish_lines(out))
}

fn update_provider_block(
    lines: &[&str],
    providers: &[ProviderBlock],
    provider: &ProviderBlock,
    next_name: &str,
    url: &str,
) -> Result<String> {
    let mut out = Vec::with_capacity(lines.len());
    let yaml_url = yaml_string(url);
    let replacement = format!("{}url: {}", " ".repeat(provider.indent + 2), yaml_url);

    let duplicate_name = providers
        .iter()
        .any(|entry| entry.name == next_name && entry.name != provider.name);

    let mut inserted_url = false;
    let mut renamed = false;

    for (index, line) in lines.iter().enumerate() {
        if !provider.line_range.contains(&index) {
            out.push((*line).to_string());
            continue;
        }

        if index == provider.key_line {
            if duplicate_name && next_name != provider.name {
                continue;
            }
            out.push(format!("{}{}:", " ".repeat(provider.indent), next_name));
            renamed = true;
            continue;
        }

        if let Some(url_indent) = url_line_indent(line) {
            if url_indent > provider.indent {
                out.push(replacement.clone());
                inserted_url = true;
                continue;
            }
        }

        if !inserted_url && is_next_provider_boundary(line, provider.indent) {
            out.push(replacement.clone());
            inserted_url = true;
        }

        out.push((*line).to_string());
    }

    if !renamed {
        bail!("failed to update provider `{}`", provider.name);
    }

    if !inserted_url {
        let insert_at = provider.line_range.end.min(out.len());
        out.insert(insert_at, replacement);
    }

    Ok(finish_lines(out))
}

fn insert_provider_block(
    lines: &[&str],
    section: Range<usize>,
    _providers: Vec<ProviderBlock>,
    name: &str,
    url: &str,
    template: Option<&Mapping>,
) -> Result<String> {
    let mut out = Vec::with_capacity(lines.len() + 4);
    let insert_at = section.end;
    let block = render_provider_block(name, url, template)?;

    for (index, line) in lines.iter().enumerate() {
        if index == insert_at {
            out.extend(block.iter().cloned());
        }
        out.push((*line).to_string());
    }

    if insert_at == lines.len() {
        out.extend(block);
    }

    Ok(finish_lines(out))
}

fn render_provider_block(name: &str, url: &str, template: Option<&Mapping>) -> Result<Vec<String>> {
    let Some(template) = template.cloned() else {
        return Ok(vec![
            format!("  {}:", name),
            "    <<: *p".to_string(),
            format!("    url: {}", yaml_string(url)),
        ]);
    };

    let mut provider = template;
    provider.insert(
        Value::String("url".to_string()),
        Value::String(url.to_string()),
    );

    let mut root = Mapping::new();
    root.insert(Value::String(name.to_string()), Value::Mapping(provider));
    let rendered = serde_yaml::to_string(&root).context("failed to serialize provider template")?;

    let block = rendered
        .lines()
        .filter(|line| !line.starts_with("---"))
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>();

    Ok(block)
}

fn detect_provider_template(content: &str) -> Result<Option<Mapping>> {
    let root: Value = serde_yaml::from_str(content).context("mihomo config yaml parse failed")?;
    let Value::Mapping(root_map) = root else {
        return Ok(None);
    };

    let Some(Value::Mapping(providers)) =
        root_map.get(Value::String("proxy-providers".to_string()))
    else {
        return Ok(None);
    };

    for value in providers.values() {
        let Value::Mapping(provider) = value else {
            continue;
        };
        if !provider.contains_key(Value::String("url".to_string())) {
            continue;
        }

        let mut template = provider.clone();
        template.remove(Value::String("url".to_string()));
        return Ok(Some(template));
    }

    Ok(None)
}

fn yaml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn finish_lines(lines: Vec<String>) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn find_proxy_providers_section(lines: &[&str]) -> Result<Range<usize>> {
    let Some(start) = lines
        .iter()
        .position(|line| line.trim() == "proxy-providers:")
    else {
        bail!("mihomo config missing `proxy-providers:` section");
    };

    let mut end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if is_top_level_key(line) {
            end = index;
            break;
        }
    }

    Ok((start + 1)..end)
}

fn parse_provider_blocks(lines: &[&str], section: Range<usize>) -> Result<Vec<ProviderBlock>> {
    let mut providers = Vec::new();
    let mut current: Option<ProviderBlock> = None;
    let base_indent = section
        .clone()
        .filter_map(|index| {
            let line = lines[index];
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                return None;
            }
            let indent = indentation(line);
            (indent > 0).then_some(indent)
        })
        .min()
        .unwrap_or(2);

    for index in section.clone() {
        let line = lines[index];
        if let Some((indent, name)) = parse_provider_key_line(line) {
            if indent != base_indent {
                continue;
            }
            if let Some(mut provider) = current.take() {
                provider.line_range.end = index;
                providers.push(provider);
            }
            current = Some(ProviderBlock {
                name,
                key_line: index,
                line_range: index..section.end,
                indent,
            });
        }
    }

    if let Some(provider) = current.take() {
        providers.push(provider);
    }

    Ok(providers)
}

fn parse_provider_key_line(line: &str) -> Option<(usize, String)> {
    if line.trim().is_empty() || line.trim_start().starts_with('#') {
        return None;
    }
    let indent = indentation(line);
    if indent == 0 {
        return None;
    }
    let trimmed = line.trim_end();
    if !trimmed.ends_with(':') {
        return None;
    }
    let name = trimmed.trim().strip_suffix(':')?.trim();
    if name.is_empty() || name.contains(' ') {
        return None;
    }
    Some((indent, name.to_string()))
}

fn find_provider_url(lines: &[&str], provider: &ProviderBlock) -> Result<Option<String>> {
    for index in (provider.key_line + 1)..provider.line_range.end {
        let line = lines[index];
        let Some(url_indent) = url_line_indent(line) else {
            continue;
        };
        if url_indent <= provider.indent {
            continue;
        }
        let raw = line.trim().strip_prefix("url:").unwrap_or("").trim();
        let parsed: String = serde_yaml::from_str(raw)
            .with_context(|| format!("failed to parse provider url for `{}`", provider.name))?;
        return Ok(Some(parsed));
    }
    Ok(None)
}

fn url_line_indent(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("url:") {
        return None;
    }
    Some(indentation(line))
}

fn is_next_provider_boundary(line: &str, provider_indent: usize) -> bool {
    if line.trim().is_empty() || line.trim_start().starts_with('#') {
        return false;
    }
    if is_top_level_key(line) {
        return true;
    }
    matches!(parse_provider_key_line(line), Some((indent, _)) if indent == provider_indent)
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

fn is_top_level_key(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }
    if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('#') {
        return false;
    }
    line.ends_with(':')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_config() -> &'static str {
        r#"p: &p {type: http, interval: 86400}
proxy-providers:
  provider1:
    <<: *p
    url: "https://one.example"
    health-check:
      enable: true
  local:
    type: file
    path: ./local.yaml
  provider2:
    <<: *p
    url: 'https://two.example'
proxy-groups:
  - name: Proxy
"#
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
    fn parses_provider_keys() {
        assert_eq!(
            parse_provider_key_line("  provider1:"),
            Some((2, "provider1".to_string()))
        );
        assert_eq!(parse_provider_key_line("    url: test"), None);
    }

    #[test]
    fn updates_only_url_and_name() {
        let output = set_provider_in_text(
            sample_config(),
            "provider1",
            "provider1-renamed",
            "https://new.example",
        )
        .unwrap();
        assert!(output.contains("  provider1-renamed:"));
        assert!(output.contains("    health-check:"));
        assert!(output.contains("    url: \"https://new.example\""));
        assert!(!output.contains("  provider1:\n"));
    }

    #[test]
    fn inserts_new_provider_at_end_of_section() {
        let output =
            set_provider_in_text(sample_config(), "added", "added", "https://add.example").unwrap();
        assert!(output.contains("  added:\n"));
        assert!(output.contains("    type: http\n"));
        assert!(output.contains("    interval: 86400\n"));
        assert!(output.contains("    health-check:\n"));
        assert!(output.contains("      enable: true\n"));
        assert!(output.contains("    url: https://add.example\n"));
        assert!(output.contains("proxy-groups:"));
    }

    #[test]
    fn removes_named_provider() {
        let path = unique_path("mihomo-provider-remove", "yaml");
        fs::write(&path, sample_config()).unwrap();
        remove_mihomo_provider(&path, "provider1").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("provider1:"));
        assert!(content.contains("provider2:"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn lists_only_providers_with_url() {
        let path = unique_path("mihomo-provider-list", "yaml");
        fs::write(&path, sample_config()).unwrap();
        let entries = list_mihomo_providers(&path).unwrap();
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
    fn parses_blocks_with_mixed_entries() {
        let lines = sample_config().lines().collect::<Vec<_>>();
        let blocks = parse_provider_blocks(&lines, 2..11).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].name, "provider1");
        assert_eq!(blocks[1].name, "local");
        assert_eq!(blocks[2].name, "provider2");
    }
}
