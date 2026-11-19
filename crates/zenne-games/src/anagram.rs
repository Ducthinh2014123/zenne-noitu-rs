use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const ANAGRAM_BANK: &[(&str, &str)] = &[
    ("hòa bình", "Trạng thái không có chiến tranh"),
    ("mặt trời", "Ngôi sao trung tâm của hệ Mặt Trời"),
    ("đồng hồ", "Dụng cụ dùng để xem giờ"),
    ("xe đạp", "Phương tiện hai bánh chạy bằng sức người"),
    ("máy tính", "Thiết bị điện tử xử lý dữ liệu"),
    ("bánh mì", "Món ăn quen thuộc làm từ bột mì"),
    ("cầu vồng", "Hiện tượng 7 sắc xuất hiện sau cơn mưa"),
    ("trường học", "Nơi học sinh đến học tập"),
    ("bệnh viện", "Nơi khám và chữa bệnh"),
    ("thư viện", "Nơi lưu giữ rất nhiều sách"),
    ("phở bò", "Món ăn truyền thống nổi tiếng của Việt Nam"),
    ("áo dài", "Trang phục truyền thống của người Việt"),
];

#[derive(Debug, Clone)]
pub struct AnagramSession {
    pub answer: String,
    pub scrambled: String,
    pub hint: String,
}

#[derive(Clone, Default)]
pub struct AnagramManager {
    sessions: Arc<DashMap<u64, AnagramSession>>,
}

impl AnagramManager {
    pub fn start(&self, channel_id: u64) -> AnagramSession {
        let mut rng = rand::thread_rng();
        let (ans, hint) = ANAGRAM_BANK
            .choose(&mut rng)
            .copied()
            .unwrap_or(("hòa bình", "Trạng thái yên bình"));
        let mut chars: Vec<char> = ans.chars().filter(|c| !c.is_whitespace()).collect();
        for _ in 0..5 {
            chars.shuffle(&mut rng);
            let s: String = chars.iter().collect();
            if s != ans.replace(' ', "") {
                break;
            }
        }
        let scrambled = chars
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let sess = AnagramSession {
            answer: ans.to_string(),
            scrambled,
            hint: hint.to_string(),
        };
        self.sessions.insert(channel_id, sess.clone());
        sess
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, String)> {
        let sess = self.sessions.get(&channel_id)?;
        let g = guess.trim().to_lowercase();
        if g == sess.answer.to_lowercase() {
            let ans = sess.answer.clone();
            drop(sess);
            self.sessions.remove(&channel_id);
            Some((true, ans))
        } else {
            Some((false, sess.answer.clone()))
        }
    }

    pub fn get_hint(&self, channel_id: u64) -> Option<String> {
        self.sessions.get(&channel_id).map(|s| s.hint.clone())
    }
}
