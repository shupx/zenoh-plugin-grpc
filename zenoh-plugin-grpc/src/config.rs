use std::{fmt, time::Duration};

use schemars::JsonSchema;
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize};

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 7335;
pub const DEFAULT_UDS_PATH: &str = "/tmp/zenoh-grpc.sock";
pub const DEFAULT_MAX_RECV_MESSAGE_SIZE: usize = 4 * 1024 * 1024;
pub const DEFAULT_MAX_SEND_MESSAGE_SIZE: usize = 4 * 1024 * 1024;
pub const DEFAULT_SHUTDOWN_GRACE_PERIOD_MS: u64 = 3_000;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub listen: Option<String>,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_uds_path")]
    pub uds_path: Option<String>,
    #[serde(default = "default_max_recv_message_size")]
    pub max_recv_message_size: usize,
    #[serde(default = "default_max_send_message_size")]
    pub max_send_message_size: usize,
    #[serde(
        default = "default_shutdown_grace_period",
        deserialize_with = "deserialize_duration_ms"
    )]
    pub shutdown_grace_period: Duration,
    #[serde(default, deserialize_with = "deserialize_path")]
    pub __path__: Option<Vec<String>>,
    #[serde(default)]
    pub __required__: Option<bool>,
    #[serde(default)]
    pub __config__: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenEndpoint {
    Tcp(String),
    Unix(String),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: None,
            host: default_host(),
            port: default_port(),
            uds_path: default_uds_path(),
            max_recv_message_size: default_max_recv_message_size(),
            max_send_message_size: default_max_send_message_size(),
            shutdown_grace_period: Duration::from_millis(default_shutdown_grace_period_ms()),
            __path__: None,
            __required__: None,
            __config__: None,
        }
    }
}

impl Config {
    pub fn listeners(&self) -> Result<Vec<ListenEndpoint>, String> {
        let mut listeners = Vec::new();
        if let Some(listen) = &self.listen {
            if let Some(addr) = listen.strip_prefix("tcp://") {
                listeners.push(ListenEndpoint::Tcp(addr.to_string()));
            } else if let Some(path) = listen.strip_prefix("unix://") {
                listeners.push(ListenEndpoint::Unix(path.to_string()));
            } else {
                return Err(format!("unsupported listen endpoint `{listen}`"));
            }
        } else {
            listeners.push(ListenEndpoint::Tcp(format!("{}:{}", self.host, self.port)));
            if let Some(path) = &self.uds_path {
                listeners.push(ListenEndpoint::Unix(path.clone()));
            }
        }
        Ok(listeners)
    }
}

fn default_host() -> String {
    DEFAULT_HOST.to_string()
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_uds_path() -> Option<String> {
    Some(DEFAULT_UDS_PATH.to_string())
}

fn default_max_recv_message_size() -> usize {
    DEFAULT_MAX_RECV_MESSAGE_SIZE
}

fn default_max_send_message_size() -> usize {
    DEFAULT_MAX_SEND_MESSAGE_SIZE
}

fn default_shutdown_grace_period_ms() -> u64 {
    DEFAULT_SHUTDOWN_GRACE_PERIOD_MS
}

fn default_shutdown_grace_period() -> Duration {
    Duration::from_millis(DEFAULT_SHUTDOWN_GRACE_PERIOD_MS)
}

fn deserialize_duration_ms<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Duration::from_millis(u64::deserialize(deserializer)?))
}

fn deserialize_path<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(OptPathVisitor)
}

struct OptPathVisitor;

impl<'de> Visitor<'de> for OptPathVisitor {
    type Value = Option<Vec<String>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "none or a string or an array of strings")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(PathVisitor).map(Some)
    }
}

struct PathVisitor;

impl<'de> Visitor<'de> for PathVisitor {
    type Value = Vec<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string or array of strings")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(vec![value.to_string()])
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element()? {
            values.push(value);
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ListenEndpoint, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_UDS_PATH};

    #[test]
    fn default_listeners_include_tcp_and_uds() {
        let config = Config::default();
        assert_eq!(
            config.listeners().unwrap(),
            vec![
                ListenEndpoint::Tcp(format!("{DEFAULT_HOST}:{DEFAULT_PORT}")),
                ListenEndpoint::Unix(DEFAULT_UDS_PATH.into())
            ]
        );
    }

    #[test]
    fn explicit_listen_tcp() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "listen": "tcp://localhost:7444"
        }))
        .unwrap();
        assert_eq!(
            config.listeners().unwrap(),
            vec![ListenEndpoint::Tcp("localhost:7444".into())]
        );
    }

    #[test]
    fn uds_added_when_present() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "uds_path": "/tmp/zenoh-grpc.sock"
        }))
        .unwrap();
        assert_eq!(config.listeners().unwrap().len(), 2);
    }

    #[test]
    fn invalid_listen_rejected() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "listen": "ws://localhost:1234"
        }))
        .unwrap();
        assert!(config.listeners().is_err());
    }
}
