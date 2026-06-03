use std::env;
use std::fmt;
use std::net::SocketAddr;

use globset::{Glob, GlobSet, GlobSetBuilder};

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub log_level: LogLevel,
    pub exclude: ExcludeMatcher,
}

/// Compiled container-exclusion matcher.
///
/// Built once at config load from the comma-separated `EXCLUDE_CONTAINERS`
/// value. Each entry is compiled as a glob pattern against the container
/// name. Glob semantics are whole-string anchored, so a literal entry with
/// no metacharacters (`foo`) matches that name exactly and never a
/// superstring (`foobar`) — preserving the original exact-match behaviour
/// with zero migration. Metacharacters (`prefix-*`, `*-cache`) extend it.
#[derive(Debug, Clone)]
pub struct ExcludeMatcher {
    /// `None` when no patterns were configured — matches nothing.
    set: Option<GlobSet>,
    /// Retained for logging/diagnostics (the original entries, trimmed).
    patterns: Vec<String>,
}

impl ExcludeMatcher {
    /// Parse a raw `EXCLUDE_CONTAINERS` value (comma-separated entries).
    ///
    /// Entries are trimmed and empty ones dropped (preserving the prior
    /// parse semantics). A malformed glob is a hard error: it surfaces as
    /// `ConfigError::InvalidValue` rather than being silently ignored.
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        let patterns: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();

        if patterns.is_empty() {
            return Ok(Self {
                set: None,
                patterns,
            });
        }

        let mut builder = GlobSetBuilder::new();
        for pattern in &patterns {
            let glob = Glob::new(pattern).map_err(|e| ConfigError::InvalidValue {
                name: "EXCLUDE_CONTAINERS".into(),
                value: pattern.clone(),
                reason: e.to_string(),
            })?;
            builder.add(glob);
        }
        let set = builder.build().map_err(|e| ConfigError::InvalidValue {
            name: "EXCLUDE_CONTAINERS".into(),
            value: raw.into(),
            reason: e.to_string(),
        })?;

        Ok(Self {
            set: Some(set),
            patterns,
        })
    }

    /// Returns `true` if the container name matches any configured pattern.
    /// Always `false` when no patterns were configured.
    #[must_use]
    pub fn is_match(&self, name: &str) -> bool {
        self.set.as_ref().is_some_and(|set| set.is_match(name))
    }

    /// Returns `true` when no exclusion patterns are configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.set.is_none()
    }

    /// The configured patterns, trimmed (for logging/diagnostics).
    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl LogLevel {
    fn parse(s: &str) -> Result<Self, ConfigError> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(ConfigError::InvalidValue {
                name: "LOG_LEVEL".into(),
                value: s.into(),
                reason: "must be one of: trace, debug, info, warn, error".into(),
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid value for {name}={value}: {reason}")]
    InvalidValue {
        name: String,
        value: String,
        reason: String,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let listen_addr = parse_optional("LISTEN_ADDR", "0.0.0.0:9713".to_owned())?
            .parse::<SocketAddr>()
            .map_err(|e| ConfigError::InvalidValue {
                name: "LISTEN_ADDR".into(),
                value: env::var("LISTEN_ADDR").unwrap_or_default(),
                reason: e.to_string(),
            })?;

        let log_level = match env::var("LOG_LEVEL") {
            Ok(val) => LogLevel::parse(&val)?,
            Err(_) => LogLevel::Info,
        };

        let exclude = ExcludeMatcher::parse(&env::var("EXCLUDE_CONTAINERS").unwrap_or_default())?;

        Ok(Self {
            listen_addr,
            log_level,
            exclude,
        })
    }
}

fn parse_optional<T>(name: &str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    match env::var(name) {
        Ok(val) if !val.is_empty() => val.parse::<T>().map_err(|e| ConfigError::InvalidValue {
            name: name.into(),
            value: val,
            reason: e.to_string(),
        }),
        _ => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "trace");
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn log_level_parse_case_insensitive() {
        assert_eq!(LogLevel::parse("DEBUG").unwrap(), LogLevel::Debug);
        assert_eq!(LogLevel::parse("info").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::parse("WARN").unwrap(), LogLevel::Warn);
    }

    #[test]
    fn log_level_parse_invalid() {
        let err = LogLevel::parse("verbose").unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue { ref name, .. } if name == "LOG_LEVEL"));
    }

    #[test]
    fn parse_optional_uses_default() {
        // Env var not set — should return default
        let result = parse_optional::<u16>("__TEST_NONEXISTENT_VAR__", 42).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn listen_addr_default() {
        let addr: SocketAddr = "0.0.0.0:9713".parse().unwrap();
        assert_eq!(addr.port(), 9713);
    }

    #[test]
    fn exclude_containers_parsing() {
        // Simulate parsing the same way Config::from_env does
        let input = "cadvisor, prometheus , grafana";
        let result: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(result, vec!["cadvisor", "prometheus", "grafana"]);
    }

    #[test]
    fn exclude_containers_empty() {
        let input = "";
        let result: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        assert!(result.is_empty());
    }

    // ── ExcludeMatcher: glob-aware container exclusion ──────────────────

    #[test]
    fn exclude_matcher_exact_match_still_works() {
        let matcher = ExcludeMatcher::parse("foo").unwrap();
        assert!(matcher.is_match("foo"));
    }

    #[test]
    fn exclude_matcher_exact_does_not_over_match() {
        // A literal entry must be whole-string anchored: `foo` excludes only
        // `foo`, never a superstring. This is the backwards-compat guarantee.
        let matcher = ExcludeMatcher::parse("foo").unwrap();
        assert!(!matcher.is_match("foobar"));
        assert!(!matcher.is_match("xfoo"));
        assert!(!matcher.is_match("foo-bar"));
    }

    #[test]
    fn exclude_matcher_single_glob() {
        let matcher = ExcludeMatcher::parse("prefix-*").unwrap();
        assert!(matcher.is_match("prefix-a"));
        assert!(matcher.is_match("prefix-b"));
        assert!(!matcher.is_match("other"));
        // Anchored: a leading char before the literal prefix must not match.
        assert!(!matcher.is_match("xprefix-a"));
    }

    #[test]
    fn exclude_matcher_suffix_glob() {
        let matcher = ExcludeMatcher::parse("*-cache").unwrap();
        assert!(matcher.is_match("redis-cache"));
        assert!(matcher.is_match("web-cache"));
        assert!(!matcher.is_match("cache-redis"));
    }

    #[test]
    fn exclude_matcher_multi_pattern_glob_groups() {
        let matcher = ExcludeMatcher::parse("a-*,b-*").unwrap();
        assert!(matcher.is_match("a-one"));
        assert!(matcher.is_match("b-two"));
        assert!(!matcher.is_match("c-three"));
    }

    #[test]
    fn exclude_matcher_multi_pattern_mixed_exact_and_glob() {
        // Mixed literal + glob entries in one EXCLUDE_CONTAINERS value.
        let matcher = ExcludeMatcher::parse("web,cache-*").unwrap();
        assert!(matcher.is_match("web")); // exact
        assert!(matcher.is_match("cache-redis")); // glob
        assert!(!matcher.is_match("website")); // exact must not over-match
        assert!(!matcher.is_match("api")); // unrelated
    }

    #[test]
    fn exclude_matcher_empty_excludes_nothing() {
        let matcher = ExcludeMatcher::parse("").unwrap();
        assert!(matcher.is_empty());
        assert!(!matcher.is_match("anything"));
        assert!(!matcher.is_match(""));
    }

    #[test]
    fn exclude_matcher_whitespace_only_excludes_nothing() {
        // Preserve existing trim+filter semantics: blank entries are dropped.
        let matcher = ExcludeMatcher::parse(" , ,  ").unwrap();
        assert!(matcher.is_empty());
        assert!(!matcher.is_match("anything"));
    }

    #[test]
    fn exclude_matcher_preserves_trim_semantics() {
        let matcher = ExcludeMatcher::parse("cadvisor, prometheus , grafana").unwrap();
        assert!(matcher.is_match("cadvisor"));
        assert!(matcher.is_match("prometheus"));
        assert!(matcher.is_match("grafana"));
        assert!(!matcher.is_match("nginx"));
    }

    #[test]
    fn exclude_matcher_invalid_glob_fails_loudly() {
        // An unmatched `[` is a malformed glob — it MUST surface as a parse
        // error at config load, never be silently dropped (silent-handler
        // doctrine: a bad pattern fails loudly).
        let err = ExcludeMatcher::parse("foo,bad[").unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidValue { ref name, .. } if name == "EXCLUDE_CONTAINERS")
        );
    }
}
