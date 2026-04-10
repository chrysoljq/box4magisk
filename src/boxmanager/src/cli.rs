use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use serde_json::json;

use crate::{
    build_singbox_config, inspect_singbox_subscriptions, list_mihomo_providers,
    list_singbox_subscriptions, parse_input, remove_mihomo_provider, remove_singbox_subscription,
    render_outbounds, sync_mihomo_proxy_providers, sync_singbox_subscriptions,
    upsert_mihomo_provider, upsert_singbox_subscription,
};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct RenderCli {
    #[arg(long, short)]
    input: PathBuf,

    #[arg(long, short)]
    output: Option<PathBuf>,

    #[arg(long, default_value = "Proxy")]
    selector_tag: String,

    #[arg(long)]
    include_urltest: bool,

    #[arg(long, default_value = "Auto")]
    urltest_tag: String,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager sync-mihomo-providers")]
struct SyncMihomoProvidersCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    state: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager list-mihomo-providers")]
struct ListMihomoProvidersCli {
    #[arg(long)]
    config: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager set-mihomo-provider")]
struct SetMihomoProviderCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    name: String,

    #[arg(long)]
    url: String,

    #[arg(long)]
    current_name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager remove-mihomo-provider")]
struct RemoveMihomoProviderCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    name: String,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager sync-singbox-subscriptions")]
struct SyncSingboxSubscriptionsCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    state: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager list-singbox-subscriptions")]
struct ListSingboxSubscriptionsCli {
    #[arg(long)]
    config: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager set-singbox-subscription")]
struct SetSingboxSubscriptionCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    name: String,

    #[arg(long)]
    url: String,

    #[arg(long)]
    current_name: Option<String>,

    #[arg(long, default_value = "remote")]
    provider_type: String,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager remove-singbox-subscription")]
struct RemoveSingboxSubscriptionCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    name: String,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager build-singbox-config")]
struct BuildSingboxConfigCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    subscriptions_dir: PathBuf,

    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Parser)]
#[command(name = "boxmanager inspect-singbox-subscriptions")]
struct InspectSingboxSubscriptionsCli {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    subscriptions_dir: PathBuf,
}

pub fn run() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if matches!(
        args.get(1).map(String::as_str),
        Some("sync-mihomo-providers")
    ) {
        let cli = SyncMihomoProvidersCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        sync_mihomo_proxy_providers(&cli.config, &cli.state)?;
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("list-mihomo-providers")
    ) {
        let cli = ListMihomoProvidersCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        let entries = list_mihomo_providers(&cli.config)?;
        println!("{}", serde_json::to_string(&entries)?);
        return Ok(());
    }
    if matches!(args.get(1).map(String::as_str), Some("set-mihomo-provider")) {
        let cli = SetMihomoProviderCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        upsert_mihomo_provider(
            &cli.config,
            cli.current_name.as_deref(),
            &cli.name,
            &cli.url,
        )?;
        println!(
            "{}",
            serde_json::to_string(&json!({
                "name": cli.name,
                "url": cli.url,
            }))?
        );
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("remove-mihomo-provider")
    ) {
        let cli = RemoveMihomoProviderCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        remove_mihomo_provider(&cli.config, &cli.name)?;
        println!(
            "{}",
            serde_json::to_string(&json!({
                "name": cli.name,
            }))?
        );
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("sync-singbox-subscriptions")
    ) {
        let cli = SyncSingboxSubscriptionsCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        sync_singbox_subscriptions(&cli.config, &cli.state)?;
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("list-singbox-subscriptions")
    ) {
        let cli = ListSingboxSubscriptionsCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        let entries = list_singbox_subscriptions(&cli.config)?;
        println!("{}", serde_json::to_string(&entries)?);
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("set-singbox-subscription")
    ) {
        let cli = SetSingboxSubscriptionCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        upsert_singbox_subscription(
            &cli.config,
            cli.current_name.as_deref(),
            &cli.name,
            &cli.url,
            &cli.provider_type,
        )?;
        println!(
            "{}",
            serde_json::to_string(&json!({
                "name": cli.name,
                "url": cli.url,
            }))?
        );
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("remove-singbox-subscription")
    ) {
        let cli = RemoveSingboxSubscriptionCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        remove_singbox_subscription(&cli.config, &cli.name)?;
        println!(
            "{}",
            serde_json::to_string(&json!({
                "name": cli.name,
            }))?
        );
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("build-singbox-config")
    ) {
        let cli = BuildSingboxConfigCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        let summary = build_singbox_config(&cli.config, &cli.subscriptions_dir, &cli.output)?;
        println!("{}", serde_json::to_string(&summary)?);
        return Ok(());
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("inspect-singbox-subscriptions")
    ) {
        let cli = InspectSingboxSubscriptionsCli::parse_from(
            std::iter::once(args[0].clone()).chain(args.into_iter().skip(2)),
        );
        let items = inspect_singbox_subscriptions(&cli.config, &cli.subscriptions_dir)?;
        println!("{}", serde_json::to_string(&items)?);
        return Ok(());
    }

    let cli = RenderCli::parse();
    let content = fs::read_to_string(&cli.input)
        .with_context(|| format!("failed to read input file {}", cli.input.display()))?;

    let parsed = parse_input(&content).context("failed to parse mihomo input")?;
    if parsed.nodes.is_empty() {
        anyhow::bail!("no convertible proxy nodes were found in the input");
    }

    let rendered = render_outbounds(
        parsed.nodes,
        &cli.selector_tag,
        cli.include_urltest,
        &cli.urltest_tag,
    );

    for warning in parsed.warnings.iter().chain(rendered.warnings.iter()) {
        eprintln!("warning: {warning}");
    }

    let output_json = json!({
        "outbounds": rendered.outbounds,
    });

    let pretty =
        serde_json::to_string_pretty(&output_json).context("failed to serialize output json")?;
    if let Some(output) = &cli.output {
        fs::write(output, pretty)
            .with_context(|| format!("failed to write output file {}", output.display()))?;
    } else {
        println!("{pretty}");
    }

    Ok(())
}
