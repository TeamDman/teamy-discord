use super::leased_http::LeasedHttp;
use eyre::Result;
use teamy_broker_sdk::AcquireRequest;
use teamy_broker_sdk::Backend;
use teamy_broker_sdk::Client;
/// # Errors
/// Exact memory-only acquisition; broker denial, incompatibility or expiry never selects another source.
pub async fn acquire(
    client: &Client,
    account: String,
    secret_ref: String,
    operation: &str,
    guild: Option<u64>,
    channel: Option<u64>,
) -> Result<LeasedHttp> {
    let destination = channel.map_or_else(
        || {
            guild.map_or_else(
                || "https://discord.com/api/v10/users/@me/guilds".into(),
                |id| format!("https://discord.com/api/v10/guilds/{id}"),
            )
        },
        |id| format!("https://discord.com/api/v10/channels/{id}"),
    );
    let lease = Box::pin(client.acquire(AcquireRequest {
        account,
        secret_ref,
        purpose: format!("Discord {operation}; guild {guild:?}; channel {channel:?}"),
        requesting_app: "teamy-discord".into(),
        destination,
        backend: Backend::Memory,
    }))
    .await?;
    LeasedHttp::broker(lease)
}
