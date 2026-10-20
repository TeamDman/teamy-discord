use super::ConsumerError;
use std::fmt;
use std::str::FromStr;

/// Nonsecret UUID correlation identity, normalized to lowercase.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, facet::Facet)]
#[facet(transparent)]
pub struct ConsumerRequestId(String);

impl ConsumerRequestId {
    /// # Errors
    /// Accepts canonical 8-4-4-4-12 UUID text with at least one nonzero digit.
    pub fn new(value: impl Into<String>) -> eyre::Result<Self> {
        let value = value.into().to_ascii_lowercase();
        let valid = value.len() == 36
            && value.bytes().enumerate().all(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
            })
            && value
                .bytes()
                .any(|byte| byte.is_ascii_hexdigit() && byte != b'0');
        eyre::ensure!(valid, ConsumerError::InvalidIdentifier);
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConsumerRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ConsumerRequestId {
    type Err = eyre::Report;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}
