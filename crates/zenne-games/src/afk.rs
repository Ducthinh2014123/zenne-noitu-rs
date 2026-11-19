use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AfkEntry {
    pub user_id: u64,
    pub guild_id: u64,
    pub reason: String,
    pub since_ts: i64,
}

#[derive(Clone, Default)]
pub struct AfkManager {
    // (guild_id, user_id) -> AfkEntry
    entries: Arc<DashMap<(u64, u64), AfkEntry>>,
}

impl AfkManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_afk(&self, guild_id: u64, user_id: u64, reason: &str) -> AfkEntry {
        let entry = AfkEntry {
            user_id,
            guild_id,
            reason: if reason.trim().is_empty() {
                "Không có lý do".to_string()
            } else {
                reason.trim().to_string()
            },
            since_ts: Utc::now().timestamp(),
        };
        self.entries.insert((guild_id, user_id), entry.clone());
        entry
    }

    pub fn remove_afk(&self, guild_id: u64, user_id: u64) -> Option<AfkEntry> {
        self.entries.remove(&(guild_id, user_id)).map(|(_, v)| v)
    }

    pub fn get_afk(&self, guild_id: u64, user_id: u64) -> Option<AfkEntry> {
        self.entries.get(&(guild_id, user_id)).map(|v| v.clone())
    }

    pub fn format_duration(since_ts: i64) -> String {
        let secs = (Utc::now().timestamp() - since_ts).max(0);
        let (h, rem) = (secs / 3600, secs % 3600);
        let (m, s) = (rem / 60, rem % 60);
        if h > 0 {
            format!("{} giờ {} phút {} giây", h, m, s)
        } else if m > 0 {
            format!("{} phút {} giây", m, s)
        } else {
            format!("{} giây", s)
        }
    }
}
