use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const EMOJI_BANK: &[(&str, &str)] = &[
    ("🍎📱", "iphone"),
    ("☀️🌻", "hoa hướng dương"),
    ("🌧️🌈", "cầu vồng"),
    ("👻🏠", "nhà ma"),
    ("🔥🦊", "firefox"),
    ("❄️👸", "nữ hoàng băng giá"),
    ("🕷️🧑", "người nhện"),
    ("🦇🧑", "batman"),
    ("🍜🐂", "phở bò"),
    ("☕🥛", "cà phê sữa"),
    ("🐉🏀", "dragon ball"),
    ("⭐⚔️", "star wars"),
];

#[derive(Clone, Default)]
pub struct EmojiGameManager {
    sessions: Arc<DashMap<u64, (String, String)>>,
}

impl EmojiGameManager {
    pub fn start(&self, channel_id: u64) -> (String, String) {
        let (emojis, ans) = EMOJI_BANK
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(EMOJI_BANK[0]);
        self.sessions
            .insert(channel_id, (emojis.to_string(), ans.to_string()));
        (emojis.to_string(), ans.to_string())
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, String)> {
        let entry = self.sessions.get(&channel_id)?;
        let ans = entry.1.clone();
        if guess.trim().to_lowercase() == ans.to_lowercase() {
            drop(entry);
            self.sessions.remove(&channel_id);
            Some((true, ans))
        } else {
            Some((false, ans))
        }
    }
}
