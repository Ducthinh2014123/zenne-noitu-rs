use dashmap::DashMap;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

struct MemEntry {
    value: String,
    expires_at: Option<Instant>,
}

pub struct CacheManager {
    redis: Option<Mutex<ConnectionManager>>,
    mem: DashMap<String, MemEntry>,
}

impl CacheManager {
    pub async fn new(redis_url: &str) -> Self {
        let redis_conn = if !redis_url.is_empty() {
            match redis::Client::open(redis_url) {
                Ok(client) => match ConnectionManager::new(client).await {
                    Ok(mgr) => {
                        info!("[Redis] Ket noi thanh cong toi Redis Server!");
                        Some(Mutex::new(mgr))
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

        if let Some(ref mgr_mutex) = self.redis {
            let mut mgr = mgr_mutex.lock().await;
            if let Ok(val) = mgr.get::<_, Option<String>>(key).await {
                if let Some(v) = val {
                    self.mem.insert(
                        key.to_string(),
                        MemEntry {
                            value: v.clone(),
                            expires_at: Some(Instant::now() + Duration::from_secs(300)),
                        },
                    );
                    return Some(v);
                }
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
        // Giới hạn kích thước mem cache tối đa 50.000 key để tuyệt đối không bao giờ tràn RAM
        if self.mem.len() > 50_000 {
            self.mem.clear();
        }
        self.mem.insert(
            key.to_string(),
            MemEntry {
                value: value.to_string(),
                expires_at: exp,
            },
        );

        if let Some(ref mgr_mutex) = self.redis {
            let mut mgr = mgr_mutex.lock().await;
            let _: Result<(), _> = if ttl_secs > 0 {
                mgr.set_ex(key, value, ttl_secs).await
            } else {
                mgr.set(key, value).await
            };
        }
    }

    pub async fn delete(&self, key: &str) {
        self.mem.remove(key);
        if let Some(ref mgr_mutex) = self.redis {
            let mut mgr = mgr_mutex.lock().await;
            let _: Result<(), _> = mgr.del(key).await;
        }
    }
}
