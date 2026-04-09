mod mihomo;
mod shared;
mod singbox;

pub use mihomo::{
    list_mihomo_providers, remove_mihomo_provider, sync_mihomo_proxy_providers,
    upsert_mihomo_provider,
};
pub use shared::ProviderEntry;
pub use singbox::{
    list_singbox_subscriptions, remove_singbox_subscription, sync_singbox_subscriptions,
    upsert_singbox_subscription,
};
