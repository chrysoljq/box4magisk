mod cli;
mod convert;
mod input;
mod sync;
mod template;

pub use cli::run as run_cli;
pub use convert::{RenderedOutbounds, render_outbounds};
pub use input::{ParsedInput, ProxyNode, parse_input};
pub use sync::{
    ProviderEntry, list_mihomo_providers, list_singbox_subscriptions, remove_mihomo_provider,
    remove_singbox_subscription, sync_mihomo_proxy_providers, sync_singbox_subscriptions,
    upsert_mihomo_provider, upsert_singbox_subscription,
};
pub use template::{
    SingboxBuildSummary, SingboxSubscriptionNodeView, SingboxSubscriptionView,
    build_singbox_config, inspect_singbox_subscriptions,
};
