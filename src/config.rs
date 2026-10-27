//! Configuration from `CISA_*` environment variables.

use std::{net::SocketAddr, str::FromStr};

use crate::autocomplete::SuggestLimits;

pub const ADDR_VAR: &str = "CISA_ADDR";
pub const MAX_QUERY_BYTES_VAR: &str = "CISA_MAX_QUERY_BYTES";
pub const DEFAULT_LIMIT_VAR: &str = "CISA_DEFAULT_LIMIT";
pub const MAX_LIMIT_VAR: &str = "CISA_MAX_LIMIT";
pub const DEFAULT_LOCALE_VAR: &str = "CISA_DEFAULT_LOCALE";

const EXPECTED_ADDR: &str = "a socket address like 127.0.0.1:8080";
const EXPECTED_COUNT: &str = "a non-negative integer";
const EXPECTED_LIMIT_ORDER: &str = "a value not greater than CISA_MAX_LIMIT";
// Room for the envelope and the non-query fields of a SuggestRequest.
const MESSAGE_OVERHEAD_BYTES: usize = 1024;

/// Server configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub addr: SocketAddr,
    pub limits: SuggestLimits,
}

/// A configuration variable holding an unusable value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{var}={value:?} is invalid; expected {expected}")]
pub struct ConfigError {
    pub var: &'static str,
    pub value: String,
    pub expected: &'static str,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
            limits: SuggestLimits::default(),
        }
    }
}

impl Config {
    /// Reads the process environment, falling back to defaults for unset variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Same as [`Config::from_env`] with an injectable lookup, so tests never touch the environment.
    ///
    /// # Examples
    ///
    /// ```
    /// use cisa::config::Config;
    ///
    /// let config = Config::from_lookup(|name| (name == "CISA_MAX_LIMIT").then(|| "20".to_owned())).unwrap();
    /// assert_eq!(config.limits.max_limit, 20);
    /// ```
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let defaults = Self::default();
        let limits: SuggestLimits = limits_from(&lookup, defaults.limits)?;
        check_limit_order(&limits)?;
        Ok(Self {
            addr: parse_var(&lookup, ADDR_VAR, defaults.addr, EXPECTED_ADDR)?,
            limits,
        })
    }

    /// Largest protobuf message accepted from a client, on any transport.
    pub fn max_message_bytes(&self) -> usize {
        self.limits.max_query_bytes + MESSAGE_OVERHEAD_BYTES
    }
}

fn limits_from(
    lookup: &impl Fn(&str) -> Option<String>,
    defaults: SuggestLimits,
) -> Result<SuggestLimits, ConfigError> {
    let count = |var: &'static str, default: usize| parse_var(lookup, var, default, EXPECTED_COUNT);
    Ok(SuggestLimits {
        max_query_bytes: count(MAX_QUERY_BYTES_VAR, defaults.max_query_bytes)?,
        default_limit: count(DEFAULT_LIMIT_VAR, defaults.default_limit)?,
        max_limit: count(MAX_LIMIT_VAR, defaults.max_limit)?,
        default_locale: lookup(DEFAULT_LOCALE_VAR).unwrap_or(defaults.default_locale),
    })
}

fn parse_var<T: FromStr>(
    lookup: &impl Fn(&str) -> Option<String>,
    var: &'static str,
    default: T,
    expected: &'static str,
) -> Result<T, ConfigError> {
    let Some(value) = lookup(var) else {
        return Ok(default);
    };
    value.parse().map_err(|_| ConfigError {
        var,
        value,
        expected,
    })
}

fn check_limit_order(limits: &SuggestLimits) -> Result<(), ConfigError> {
    if limits.default_limit <= limits.max_limit {
        return Ok(());
    }
    Err(ConfigError {
        var: DEFAULT_LIMIT_VAR,
        value: limits.default_limit.to_string(),
        expected: EXPECTED_LIMIT_ORDER,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup_from(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(&str, &str)> = pairs.to_vec();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn from_lookup_without_variables_returns_defaults() {
        let config = Config::from_lookup(lookup_from(&[])).unwrap();

        assert_eq!(config, Config::default());
    }

    #[test]
    fn from_lookup_with_variables_overrides_defaults() {
        // Arrange
        let lookup = lookup_from(&[
            (ADDR_VAR, "0.0.0.0:9000"),
            (MAX_QUERY_BYTES_VAR, "256"),
            (DEFAULT_LIMIT_VAR, "5"),
            (MAX_LIMIT_VAR, "6"),
            (DEFAULT_LOCALE_VAR, "en-US"),
        ]);

        // Act
        let config = Config::from_lookup(lookup).unwrap();

        // Assert
        assert_eq!(config.addr, SocketAddr::from(([0, 0, 0, 0], 9000)));
        assert_eq!(config.limits.max_query_bytes, 256);
        assert_eq!(config.limits.default_limit, 5);
        assert_eq!(config.limits.max_limit, 6);
        assert_eq!(config.limits.default_locale, "en-US");
    }

    #[test]
    fn from_lookup_invalid_addr_returns_error_naming_var_and_value() {
        let result = Config::from_lookup(lookup_from(&[(ADDR_VAR, "localhost")]));

        let error = result.unwrap_err();
        assert_eq!(
            error.to_string(),
            "CISA_ADDR=\"localhost\" is invalid; expected a socket address like 127.0.0.1:8080"
        );
    }

    #[test]
    fn from_lookup_non_numeric_limit_returns_error() {
        let result = Config::from_lookup(lookup_from(&[(MAX_LIMIT_VAR, "ten")]));

        assert_eq!(result.unwrap_err().var, MAX_LIMIT_VAR);
    }

    #[test]
    fn from_lookup_default_limit_above_max_returns_error() {
        let result = Config::from_lookup(lookup_from(&[(DEFAULT_LIMIT_VAR, "11")]));

        assert_eq!(result.unwrap_err().var, DEFAULT_LIMIT_VAR);
    }

    #[test]
    fn max_message_bytes_adds_envelope_overhead_to_query_limit() {
        let config = Config::default();

        assert_eq!(config.max_message_bytes(), 512 + MESSAGE_OVERHEAD_BYTES);
    }
}
