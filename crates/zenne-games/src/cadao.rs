use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const CADAO_BANK: &[(&str, &str)] = &[
    (
        "Công cha như núi Thái Sơn, Nghĩa mẹ như nước trong nguồn ___",
        "chảy ra",
    ),
    (
        "Bầu ơi thương lấy bí cùng, Tuy rằng khác giống nhưng chung một ___",
        "giàn",
    ),
    ("Gần mực thì đen, gần đèn thì ___", "sáng"),
    ("Có công mài sắt, có ngày nên ___", "kim"),
    ("Ăn quả nhớ kẻ ___", "trồng cây"),
    ("Uống nước nhớ ___", "nguồn"),
    ("Lá lành đùm lá ___", "rách"),
    (
        "Một cây làm chẳng nên non, Ba cây chụm lại nên hòn ___",
        "núi cao",
    ),
    ("Đi một ngày đàng, học một sàng ___", "khôn"),
    ("Thất bại là mẹ ___", "thành công"),
    ("Học thầy không tày ___", "học bạn"),
    ("Tốt gỗ hơn tốt ___", "nước sơn"),
];

#[derive(Debug, Clone)]
pub struct CaDaoSession {
    pub prompt: String,
    pub answer: String,
}

#[derive(Clone, Default)]
pub struct CaDaoManager {
    sessions: Arc<DashMap<u64, CaDaoSession>>,
}

impl CaDaoManager {
    pub fn start(&self, channel_id: u64) -> CaDaoSession {
        let (prompt, ans) = CADAO_BANK
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or((CADAO_BANK[0].0, CADAO_BANK[0].1));
        let sess = CaDaoSession {
            prompt: prompt.to_string(),
            answer: ans.to_string(),
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
