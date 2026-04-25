use std::env;
use std::fmt;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub log_level: LogLevel,
    pub exclude_containers: Vec<String>,
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

        let exclude_containers = env::var("EXCLUDE_CONTAINERS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self {
            listen_addr,
            log_level,
            exclude_containers,
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
}
