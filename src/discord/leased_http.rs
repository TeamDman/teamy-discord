use eyre::Result;
use serenity::all::Http;
#[cfg(windows)]
use teamy_broker_sdk::Secret;
/// Holds one command's cooperative lease. Serenity internally copies the authorization header.
pub struct LeasedHttp {
    http: Http,
    #[cfg(windows)]
    lease: Option<Secret>,
}
impl std::fmt::Debug for LeasedHttp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LeasedHttp([redacted])")
    }
}
impl std::ops::Deref for LeasedHttp {
    type Target = Http;
    fn deref(&self) -> &Http {
        &self.http
    }
}
impl LeasedHttp {
    /// Construct a credential-bearing Gateway HTTP client after checking the original lease.
    /// # Errors
    /// Rejects expired handles. The daemon must also terminate retained Gateway tasks at expiry.
    pub fn gateway_http(&self) -> Result<Http> {
        self.check()?;
        Ok(Http::new(self.http.token()))
    }
    /// The original broker expiry, without sliding or exposing credential bytes.
    #[must_use]
    pub fn expires_unix_ms(&self) -> Option<u64> {
        #[cfg(windows)]
        {
            self.lease.as_ref().map(Secret::expires_unix_ms)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
    /// # Errors
    /// Rejects invalid UTF8 without including credential bytes.
    #[cfg(windows)]
    pub fn broker(lease: Secret) -> Result<Self> {
        let http = lease.expose(|bytes| {
            std::str::from_utf8(bytes)
                .map(Http::new)
                .map_err(|_redacted_error| eyre::eyre!("Invalid credential encoding"))
        })??;
        Ok(Self {
            http,
            lease: Some(lease),
        })
    }
    /// # Errors
    /// Rejects blank credentials.
    pub fn direct(token: &str) -> Result<Self> {
        eyre::ensure!(!token.trim().is_empty(), "Explicit credential is empty");
        Ok(Self {
            http: Http::new(token),
            #[cfg(windows)]
            lease: None,
        })
    }
    /// # Errors
    /// Rejects expired broker handles without activity renewal.
    pub fn check(&self) -> Result<()> {
        #[cfg(windows)]
        if let Some(lease) = &self.lease {
            lease.expose(|_| ())?;
        }
        Ok(())
    }
    /// # Errors
    /// Cancels pending operations at expiry and rejects results completed after expiry.
    pub async fn run<T>(&self, operation: impl Future<Output = Result<T>>) -> Result<T> {
        self.check()?;
        tokio::pin!(operation);
        let mut poll = tokio::time::interval(std::time::Duration::from_millis(10));
        loop {
            tokio::select! {
                result=&mut operation => { self.check()?; return result; }
                _=poll.tick() => self.check()?,
            }
        }
    }
}
