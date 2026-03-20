use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformChannel {
    Qqbot,
    Telegram,
    Discord,
    Slack,
    Webhook,
    Custom(String),
}

impl PlatformChannel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Qqbot => "qqbot",
            Self::Telegram => "telegram",
            Self::Discord => "discord",
            Self::Slack => "slack",
            Self::Webhook => "webhook",
            Self::Custom(value) => value.as_str(),
        }
    }
}

impl From<&str> for PlatformChannel {
    fn from(value: &str) -> Self {
        match value {
            "qqbot" => Self::Qqbot,
            "telegram" => Self::Telegram,
            "discord" => Self::Discord,
            "slack" => Self::Slack,
            "webhook" => Self::Webhook,
            other => Self::Custom(other.to_string()),
        }
    }
}
