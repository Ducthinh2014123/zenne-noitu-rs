use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const HISTORY_BANK: &[(&str, &str)] = &[
    ("Chiến thắng Bạch Đằng năm 938 do ai lãnh đạo?", "ngô quyền"),
    ("Chiến thắng Điện Biên Phủ diễn ra vào năm nào?", "1954"),
    ("Ai là vị vua đầu tiên của triều đại nhà Lý?", "lý thái tổ"),
    (
        "Bác Hồ đọc Tuyên ngôn Độc lập vào ngày nào? (dd/mm/yyyy)",
        "02/09/1945",
    ),
    ("Ai là người lãnh đạo cuộc khởi nghĩa Lam Sơn?", "lê lợi"),
    (
        "Trận Ngọc Hồi - Đống Đa năm 1789 gắn liền với vị anh hùng nào?",
        "quang trung",
    ),
    (
        "Nhà Trần ba lần đánh thắng quân xâm lược nào?",
        "nguyên mông",
    ),
    ("Chiến dịch Hồ Chí Minh toàn thắng vào năm nào?", "1975"),
];

#[derive(Clone, Default)]
pub struct HistoryGuessManager {
    sessions: Arc<DashMap<u64, (String, String)>>,
}

impl HistoryGuessManager {
    pub fn start(&self, channel_id: u64) -> (String, String) {
        let (q, a) = HISTORY_BANK
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(HISTORY_BANK[0]);
        self.sessions
            .insert(channel_id, (q.to_string(), a.to_string()));
        (q.to_string(), a.to_string())
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, String)> {
        let entry = self.sessions.get(&channel_id)?;
        let ans = entry.1.clone();
        if guess.trim().to_lowercase().contains(&ans) {
            drop(entry);
            self.sessions.remove(&channel_id);
            Some((true, ans))
        } else {
            Some((false, ans))
        }
    }
}
