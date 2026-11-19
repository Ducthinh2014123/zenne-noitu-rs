pub mod auth;
pub mod cache;
pub mod config;
pub mod db;
pub mod i18n;
pub mod kira;
pub mod wiki;

use cache::CacheManager;
use config::AppConfig;
use db::DbManager;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildSummary {
    pub id: String,
    pub name: String,
    pub member_count: u64,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BotLiveStatus {
    pub online: bool,
    pub guild_count: usize,
    pub user_count: u64,
    pub latency_ms: Option<f64>,
    pub guilds: Vec<GuildSummary>,
}

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub db: Arc<DbManager>,
    pub cache: Arc<CacheManager>,
    pub http: Client,
    pub live: Arc<RwLock<BotLiveStatus>>,
}

impl AppContext {
    pub async fn init() -> anyhow::Result<Self> {
        let config = Arc::new(AppConfig::from_env());
        let db = Arc::new(DbManager::new(&config).await?);
        let cache = Arc::new(CacheManager::new(&config.redis_url).await);
        let http = Client::builder()
            .user_agent("ZenneNoituBot/2.0 (Rust)")
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        Ok(Self {
            config,
            db,
            cache,
            http,
            live: Arc::new(RwLock::new(BotLiveStatus::default())),
        })
    }
}
