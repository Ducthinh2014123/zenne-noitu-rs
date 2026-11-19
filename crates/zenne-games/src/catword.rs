use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const CATWORD_BANK: &[(&str, &str)] = &[
    ("trường học", "Học đường / nơi học tập"),
    ("mặt trời", "Ngôi sao trung tâm hệ Mặt Trời"),
    ("đồng hồ", "Dụng cụ đo thời gian"),
    ("xe đạp", "Phương tiện hai bánh chạy bằng sức người"),
    ("cầu vồng", "Hiện tượng quang học 7 màu sau mưa"),
    ("hoa hồng", "Loài hoa tượng trưng cho tình yêu"),
    ("bánh chưng", "Bánh truyền thống ngày Tết"),
    ("sách vở", "Đồ dùng học tập quen thuộc"),
];

#[derive(Debug, Clone)]
pub struct CatWordSession {
    pub masked: String,
    pub hint: String,
    pub answer: String,
}

#[derive(Clone, Default)]
pub struct CatWordManager {
    sessions: Arc<DashMap<u64, CatWordSession>>,
}

impl CatWordManager {
    pub fn start(&self, channel_id: u64) -> CatWordSession {
        let (word, hint) = CATWORD_BANK
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(CATWORD_BANK[0]);
        let masked: String = word
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if c == ' ' || i == 0 || i % 3 == 0 {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let sess = CatWordSession {
            masked,
            hint: hint.to_string(),
            answer: word.to_string(),
        };
        self.sessions.insert(channel_id, sess.clone());
        sess
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, String)> {
        let sess = self.sessions.get(&channel_id)?;
        if guess.trim().to_lowercase() == sess.answer.to_lowercase() {
            let ans = sess.answer.clone();
            drop(sess);
            self.sessions.remove(&channel_id);
            Some((true, ans))
        } else {
            Some((false, sess.answer.clone()))
        }
    }
}
