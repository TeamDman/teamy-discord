use arbitrary::Arbitrary;
use eyre::Context;
use eyre::Result;
use facet::Facet;
use figue::{self as args};
use serenity::all::ApplicationId;
use serenity::all::CreateBotAuthParameters;
use serenity::all::Http;
use serenity::all::Scope;

/// Print and optionally open the Discord bot invite URL.
#[derive(Facet, Arbitrary, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct InviteArgs {
    /// Discord bot token. If omitted, requires one explicit token environment variable; no saved preference fallback.
    #[facet(args::named)]
    pub bot_token: Option<String>,

    /// Print the invite URL without opening it in the browser.
    #[facet(args::named, default)]
    pub no_open: bool,
}

#[must_use]
// cli[impl invite.prints-url]
fn build_invite_url(application_id: ApplicationId) -> String {
    CreateBotAuthParameters::new()
        .client_id(application_id)
        .scopes(&[Scope::Bot])
        .build()
}

impl InviteArgs {
    /// # Errors
    ///
    /// This function will return an error if the token cannot be resolved,
    /// the Discord API call fails, or the browser cannot be opened.
    // cli[impl auth.invite-token-resolution]
    // cli[impl invite.opens-browser-by-default]
    pub async fn invoke(self) -> Result<crate::cli::output::CliOutput> {
        let threshold = Some(std::time::Duration::from_secs(if self.no_open {
            30
        } else {
            33
        }));
        self.invoke_work().await?;
        Ok(crate::cli::output::CliOutput::none(threshold))
    }

    async fn invoke_work(self) -> Result<()> {
        let resolved = crate::paths::resolve_bot_token(self.bot_token.as_deref())?;
        let http = Http::new(&resolved.token);
        let application_info = http
            .get_current_application_info()
            .await
            .wrap_err("Failed to fetch current Discord application info for invite URL")?;
        let invite_url = build_invite_url(application_info.id);

        println!("{invite_url}");

        if !self.no_open {
            open::that_detached(&invite_url)
                .wrap_err("Failed to open Discord bot invite URL in the browser")?;
        }

        Ok(())
    }
}

impl std::fmt::Debug for InviteArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InviteArgs")
            .field("bot_token", &self.bot_token.as_ref().map(|_| "[REDACTED]"))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::build_invite_url;
    use serenity::all::ApplicationId;

    #[test]
    // cli[verify invite.prints-url]
    fn invite_url_contains_client_id_and_bot_scope() {
        let invite_url = build_invite_url(ApplicationId::new(123_456_789_012_345_678));

        assert!(invite_url.contains("client_id=123456789012345678"));
        assert!(invite_url.contains("scope=bot"));
    }
}
