//! The provider credential (PRD §23.6): only from `RIPWIRE_BROKER_JEV_API_KEY` in the server's
//! environment, never from the command line, never serialized, never persisted.

use secrecy::{ExposeSecret, SecretString};

pub const ENV_VAR: &str = "RIPWIRE_BROKER_JEV_API_KEY";

/// A redacted token: `Debug` never shows it, and there is no `Serialize` or `Display`.
#[derive(Debug)]
pub struct Credential(SecretString);

/// Why a credential was refused; the messages never contain any part of the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// Unset, empty or only whitespace.
    Missing,
    InternalWhitespace,
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "--online needs {ENV_VAR} in the server's environment"),
            Self::InternalWhitespace => write!(f, "{ENV_VAR} contains whitespace inside the value"),
        }
    }
}

impl std::error::Error for CredentialError {}

impl Credential {
    /// Trims outer whitespace; an empty value counts as absent.
    pub fn from_env_value(value: Option<&str>) -> Result<Self, CredentialError> {
        let value = value.map(str::trim).unwrap_or_default();
        if value.is_empty() {
            return Err(CredentialError::Missing);
        }
        if value.chars().any(char::is_whitespace) {
            return Err(CredentialError::InternalWhitespace);
        }
        Ok(Self(SecretString::from(value.to_string())))
    }

    /// Reads [`ENV_VAR`]; a value that is not UTF-8 counts as absent.
    pub fn from_env() -> Result<Self, CredentialError> {
        Self::from_env_value(std::env::var(ENV_VAR).ok().as_deref())
    }

    /// For the `Authorization` header only.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}
