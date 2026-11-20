use dashmap::DashMap;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::time::{Duration, Instant};
use tracing::{info, warn};

struct MemEntry {
    value: String,
    expires_at: Option<Instant>,
}

pub struct CacheManager {
    redis: Option<ConnectionManager>,
    mem: DashMap<String, MemEntry>,
}

impl CacheManager {
    pub async fn new(redis_url: &str) -> Self {
        let redis_conn = if !redis_url.is_empty() {
            match redis::Client::open(redis_url) {
                Ok(client) => match ConnectionManager::new(client).await {
                    Ok(mgr) => {
                        info!("[Redis] Ket noi thanh cong toi Redis Server!");
                        Some(mgr)
                    }
                    Err(e) => {
                        warn!(
                            "[Redis] Khong the ket noi Redis ({}), dung bo nho dem cuc bo.",
                            e
                        );
                        None
                    }
                },
                Err(e) => {
                    warn!("[Redis] URL khong hop le ({}), dung bo nho dem cuc bo.", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            redis: redis_conn,
            mem: DashMap::new(),
        }
    }

    fn evict_if_full(&self) {
        if self.mem.len() > 50_000 {
            let now = Instant::now();
            self.mem
                .retain(|_, v| v.expires_at.map(|e| now < e).unwrap_or(true));
            if self.mem.len() > 50_000 {
                self.mem.clear();
            }
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        if let Some(entry) = self.mem.get(key) {
            if let Some(exp) = entry.expires_at {
                if Instant::now() < exp {
                    return Some(entry.value.clone());
                }
            } else {
                return Some(entry.value.clone());
            }
        }
        self.mem.remove(key);

        if let Some(ref mgr_orig) = self.redis {
            let mut mgr = mgr_orig.clone();
            if let Ok(Some(v)) = mgr.get::<_, Option<String>>(key).await {
                let ttl_secs = mgr
                    .ttl::<_, i64>(key)
                    .await
                    .ok()
                    .filter(|&t| t > 0)
                    .map(|t| (t as u64).min(300))
                    .unwrap_or(300);
                self.evict_if_full();
                self.mem.insert(
                    key.to_string(),
                    MemEntry {
                        value: v.clone(),
                        expires_at: Some(Instant::now() + Duration::from_secs(ttl_secs)),
                    },
                );
                return Some(v);
            }
        }
        None
    }

    pub async fn set(&self, key: &str, value: &str, ttl_secs: u64) {
        let exp = if ttl_secs > 0 {
            Some(Instant::now() + Duration::from_secs(ttl_secs))
        } else {
            None
        };
        self.evict_if_full();
        self.mem.insert(
            key.to_string(),
            MemEntry {
                value: value.to_string(),
                expires_at: exp,
            },
        );

        if let Some(ref mgr_orig) = self.redis {
            let mut mgr = mgr_orig.clone();
            let _: Result<(), _> = if ttl_secs > 0 {
                mgr.set_ex(key, value, ttl_secs).await
            } else {
                mgr.set(key, value).await
            };
        }
    }

    pub async fn delete(&self, key: &str) {
        self.mem.remove(key);
        if let Some(ref mgr_orig) = self.redis {
            let mut mgr = mgr_orig.clone();
            let _: Result<(), _> = mgr.del(key).await;
        }
    }

    /// Fixed-window rate limiter matching Python's `_rate_limited` (`rl:{key}`).
    pub async fn rate_limited(&self, key: &str, max_hits: u64, window_secs: u64) -> bool {
        let rl_key = format!("rl:{}", key);
        if let Some(ref mgr_orig) = self.redis {
            let mut mgr = mgr_orig.clone();
            if let Ok(cnt) = mgr.incr::<_, _, u64>(&rl_key, 1).await {
                if cnt == 1 {
                    let _: Result<(), _> = mgr.expire(&rl_key, window_secs as i64).await;
                }
                return cnt > max_hits;
            }
        }
        let now = Instant::now();
        let (next_cnt, exp) = match self.mem.get(&rl_key) {
            Some(entry) if entry.expires_at.map(|e| now < e).unwrap_or(true) => (
                entry.value.parse::<u64>().unwrap_or(0) + 1,
                entry
                    .expires_at
                    .unwrap_or_else(|| now + Duration::from_secs(window_secs)),
            ),
            _ => (1, now + Duration::from_secs(window_secs)),
        };
        self.evict_if_full();
        self.mem.insert(
            rl_key,
            MemEntry {
                value: next_cnt.to_string(),
                expires_at: Some(exp),
            },
        );
        next_cnt > max_hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limited_fixed_window_and_cache() {
        let cache = CacheManager::new("").await;
        assert!(!cache.rate_limited("k1", 2, 10).await);
        assert!(!cache.rate_limited("k1", 2, 10).await);
        assert!(cache.rate_limited("k1", 2, 10).await);

        cache.set("hello", "world", 60).await;
        assert_eq!(cache.get("hello").await.as_deref(), Some("world"));
        cache.delete("hello").await;
        assert_eq!(cache.get("hello").await, None);
    }
}
