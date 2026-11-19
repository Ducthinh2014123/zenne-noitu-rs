use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const WHOAMI_BANK: &[(&str, &str)] = &[
    (
        "Tôi có vòi dài, tai to và rất thích ăn mía. Tôi là con gì?",
        "con voi",
    ),
    (
        "Tôi kêu meo meo và thích bắt chuột. Tôi là con gì?",
        "con mèo",
    ),
    (
        "Tôi mọc ở hướng Đông, lặn ở hướng Tây, mang lại ánh sáng. Tôi là gì?",
        "mặt trời",
    ),
    (
        "Tôi có 4 chân nhưng không biết đi, dùng để ngồi. Tôi là gì?",
        "cái ghế",
    ),
    ("Tôi càng lau càng ướt. Tôi là gì?", "cái khăn"),
    (
        "Tôi có cổ mà không có đầu, có tay mà không có ngón. Tôi là gì?",
        "cái áo",
    ),
    (
        "Tôi chạy quanh mặt nhưng không bao giờ mệt, chỉ giờ cho mọi người. Tôi là gì?",
        "đồng hồ",
    ),
];

#[derive(Clone, Default)]
pub struct WhoAmIManager {
    sessions: Arc<DashMap<u64, (String, String)>>,
}

impl WhoAmIManager {
    pub fn start(&self, channel_id: u64) -> (String, String) {
        let (clue, ans) = WHOAMI_BANK
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(WHOAMI_BANK[0]);
        self.sessions
            .insert(channel_id, (clue.to_string(), ans.to_string()));
        (clue.to_string(), ans.to_string())
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, String)> {
        let entry = self.sessions.get(&channel_id)?;
        let ans = entry.1.clone();
        let g = guess.trim().to_lowercase();
        if g == ans || ans.ends_with(&g) && g.len() >= 3 {
            drop(entry);
            self.sessions.remove(&channel_id);
            Some((true, ans))
        } else {
            Some((false, ans))
        }
    }
}
