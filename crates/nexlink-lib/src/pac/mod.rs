use anyhow::{Context, Result};
use dashmap::DashSet;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;
use std::time::SystemTime;

/// 需要走代理的域名集合（自动学习 + 用户自定义）
pub static PROXY_RULES: LazyLock<DashSet<String>> = LazyLock::new(DashSet::new);

/// 规则文件格式
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyRulesFile {
    domains: Vec<String>,
    updated_at: String,
}

fn timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

/// 常见被墙域名种子列表
const SEED_DOMAINS: &[&str] = &[
    "google.com",
    "youtube.com",
    "twitter.com",
    "facebook.com",
    "instagram.com",
    "github.com",
    "githubusercontent.com",
    "wikipedia.org",
    "reddit.com",
    "twitch.tv",
    "discord.com",
    "telegram.org",
    "whatsapp.com",
    "dropbox.com",
    "medium.com",
    "vimeo.com",
    "pinterest.com",
    "tumblr.com",
    "flickr.com",
    "nytimes.com",
    "bbc.com",
    "cloudflare.com",
    "openai.com",
    "anthropic.com",
    "stackoverflow.com",
    "gitlab.com",
    "bitbucket.org",
    "slack.com",
    "zoom.us",
    "notion.so",
];

/// 从文件加载规则到 PROXY_RULES
pub fn load_rules(path: &Path) -> Result<()> {
    if !path.exists() {
        // 首次启动，写入种子列表
        let seed_rules = ProxyRulesFile {
            domains: SEED_DOMAINS.iter().map(|s| s.to_string()).collect(),
            updated_at: timestamp_now(),
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&seed_rules)?;
        std::fs::write(path, json)?;

        for domain in SEED_DOMAINS {
            PROXY_RULES.insert(domain.to_string());
        }

        tracing::info!(
            count = SEED_DOMAINS.len(),
            "Initialized proxy rules with seed domains"
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(path).context("Failed to read proxy rules file")?;

    let rules: ProxyRulesFile =
        serde_json::from_str(&content).context("Failed to parse proxy rules file")?;

    PROXY_RULES.clear();
    for domain in rules.domains {
        PROXY_RULES.insert(domain);
    }

    tracing::info!(count = PROXY_RULES.len(), "Loaded proxy rules from file");
    Ok(())
}

/// 持久化 PROXY_RULES 到文件
pub fn save_rules(path: &Path) -> Result<()> {
    let domains: Vec<String> = PROXY_RULES.iter().map(|r| r.clone()).collect();

    let rules = ProxyRulesFile {
        domains,
        updated_at: timestamp_now(),
    };

    let json = serde_json::to_string_pretty(&rules)?;
    std::fs::write(path, json)?;

    tracing::debug!(count = PROXY_RULES.len(), "Saved proxy rules to file");
    Ok(())
}

/// 判断 host 是否需要走代理（支持子域名匹配）
/// 例如：rules 中有 google.com 则 www.google.com 也匹配
pub fn needs_proxy(host: &str) -> bool {
    matching_proxy_rule(host).is_some()
}

/// 返回命中的代理规则（精确匹配或父域名规则）
pub fn matching_proxy_rule(host: &str) -> Option<String> {
    // 直接匹配
    if PROXY_RULES.contains(host) {
        return Some(host.to_string());
    }

    // 子域名匹配：检查 host 是否以 ".domain" 结尾
    for rule in PROXY_RULES.iter() {
        let domain = rule.key();
        if host.ends_with(&format!(".{}", domain)) {
            return Some(domain.to_string());
        }
    }

    None
}

/// 记录一个域名需要走代理
pub fn add_proxy_rule(host: &str) {
    if !PROXY_RULES.contains(host) {
        PROXY_RULES.insert(host.to_string());
        tracing::info!(host, "Added new proxy rule");
    }
}

/// 获取当前规则数量
pub fn rule_count() -> usize {
    PROXY_RULES.len()
}

/// 获取所有规则（用于前端展示）
pub fn get_all_rules() -> Vec<String> {
    PROXY_RULES.iter().map(|r| r.clone()).collect()
}

/// 批量更新规则（用户手动添加/删除）
pub fn update_rules(domains: Vec<String>) {
    PROXY_RULES.clear();
    for domain in domains {
        PROXY_RULES.insert(domain);
    }
    tracing::info!(count = PROXY_RULES.len(), "Updated proxy rules");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_proxy() {
        PROXY_RULES.clear();
        PROXY_RULES.insert("google.com".to_string());

        assert!(needs_proxy("google.com"));
        assert!(needs_proxy("www.google.com"));
        assert!(needs_proxy("mail.google.com"));
        assert!(!needs_proxy("google.cn"));
        assert!(!needs_proxy("baidu.com"));
    }
}
