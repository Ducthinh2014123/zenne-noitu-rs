use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const CHINHTA_BANK: &[(&str, &str, &str)] = &[
    ("Sáng suốt", "Sáng suôt", "sáng suốt"),
    ("Xinh xắn", "Sinh xắn", "xinh xắn"),
    ("Chính tả", "Trính tả", "chính tả"),
    ("Lãng mạn", "Lãng mạng", "lãng mạn"),
    ("Sẵn sàng", "Sẳn sàng", "sẵn sàng"),
    ("Trân trọng", "Chân trọng", "trân trọng"),
    ("Xuất sắc", "Suất sắc", "xuất sắc"),
    ("Bổ sung", "Bổ xung", "bổ sung"),
    ("Chẩn đoán", "Chuẩn đoán", "chẩn đoán"),
    ("Tham quan", "Thăm quan", "tham quan"),
    ("Ngành nghề", "Nghành nghề", "ngành nghề"),
    ("Vô hình trung", "Vô hình chung", "vô hình trung"),
];

#[derive(Debug, Clone)]
pub struct ChinhTaSession {
    pub opt_a: String,
    pub opt_b: String,
    pub correct: String,
}

#[derive(Clone, Default)]
pub struct ChinhTaManager {
    sessions: Arc<DashMap<u64, ChinhTaSession>>,
}

impl ChinhTaManager {
    pub fn start(&self, channel_id: u64) -> ChinhTaSession {
        let mut rng = rand::thread_rng();
        let (right, wrong, correct) = CHINHTA_BANK
            .choose(&mut rng)
            .copied()
            .unwrap_or(CHINHTA_BANK[0]);
        let flip = rand::Rng::gen_bool(&mut rng, 0.5);
        let sess = ChinhTaSession {
            opt_a: if flip { wrong } else { right }.to_string(),
            opt_b: if flip { right } else { wrong }.to_string(),
            correct: correct.to_string(),
        };
        self.sessions.insert(channel_id, sess.clone());
        sess
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, String)> {
        let sess = self.sessions.get(&channel_id)?;
        if guess.trim().to_lowercase() == sess.correct.to_lowercase() {
            let ans = sess.correct.clone();
            drop(sess);
            self.sessions.remove(&channel_id);
            Some((true, ans))
        } else {
            Some((false, sess.correct.clone()))
        }
    }
}
