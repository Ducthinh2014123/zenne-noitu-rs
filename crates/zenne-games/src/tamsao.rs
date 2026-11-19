use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const PHRASES: &[&str] = &[
    "con cò bé bé đậu cành tre",
    "học ăn học nói học gói học mở",
    "trăm nghe không bằng một thấy",
    "có công mài sắt có ngày nên kim",
    "uống nước nhớ nguồn ăn quả nhớ kẻ trồng cây",
    "bầu ơi thương lấy bí cùng",
];

#[derive(Debug, Clone)]
pub struct TamSaoSession {
    pub original: String,
}

#[derive(Clone, Default)]
pub struct TamSaoManager {
    sessions: Arc<DashMap<u64, TamSaoSession>>,
}

impl TamSaoManager {
    pub fn start(&self, channel_id: u64) -> String {
        let phrase = PHRASES
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(PHRASES[0]);
        self.sessions.insert(
            channel_id,
            TamSaoSession {
                original: phrase.to_string(),
            },
        );
        phrase.to_string()
    }

    pub fn check(&self, channel_id: u64, typed: &str) -> Option<(bool, String)> {
        let sess = self.sessions.get(&channel_id)?;
        let ok = typed.trim().to_lowercase() == sess.original.to_lowercase();
        let orig = sess.original.clone();
        if ok {
            drop(sess);
            self.sessions.remove(&channel_id);
        }
        Some((ok, orig))
    }
}
