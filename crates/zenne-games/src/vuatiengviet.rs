use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const VTV_QUESTIONS: &[(&str, &str)] = &[
    ("t/h/ô/n/g/m/i/n/h", "thông minh"),
    ("h/ạ/n/h/p/h/ú/c", "hạnh phúc"),
    ("y/ê/u/t/h/ư/ơ/n/g", "yêu thương"),
    ("k/i/ê/n/n/h/ẫ/n", "kiên nhẫn"),
    ("t/r/u/n/g/t/h/ự/c", "trung thực"),
    ("d/ũ/n/g/c/ả/m", "dũng cảm"),
    ("s/á/n/g/t/ạ/o", "sáng tạo"),
    ("đ/o/à/n/k/ế/t", "đoàn kết"),
    ("h/ọ/c/h/ỏ/i", "học hỏi"),
    ("t/ự/t/i/n", "tự tin"),
    ("q/u/ê/h/ư/ơ/n/g", "quê hương"),
    ("g/i/a/đ/ì/n/h", "gia đình"),
];

#[derive(Debug, Clone)]
pub struct VtvSession {
    pub scrambled: String,
    pub answer: String,
}

#[derive(Clone, Default)]
pub struct VuaTiengVietManager {
    sessions: Arc<DashMap<u64, VtvSession>>,
}

impl VuaTiengVietManager {
    pub fn start(&self, channel_id: u64) -> VtvSession {
        let mut rng = rand::thread_rng();
        let (_, ans) = VTV_QUESTIONS
            .choose(&mut rng)
            .copied()
            .unwrap_or(("", "hạnh phúc"));
        let mut chars: Vec<char> = ans.chars().filter(|c| !c.is_whitespace()).collect();
        chars.shuffle(&mut rng);
        let scrambled = chars
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" / ");
        let sess = VtvSession {
            scrambled,
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
