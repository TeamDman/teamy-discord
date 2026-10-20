use super::ConsumerError;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, facet::Facet)]
#[facet(transparent)]
pub struct ConsumerName(String);

impl ConsumerName {
    /// # Errors
    /// Names are 1–64 ASCII characters, start with a letter/digit, and otherwise
    /// contain only letters, digits, underscores or hyphens.
    pub fn new(value: impl Into<String>) -> eyre::Result<Self> {
        let value = value.into();
        eyre::ensure!(
            (1..=64).contains(&value.len())
                && value
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'),
            ConsumerError::InvalidIdentifier
        );
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConsumerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ConsumerName {
    type Err = eyre::Report;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}
