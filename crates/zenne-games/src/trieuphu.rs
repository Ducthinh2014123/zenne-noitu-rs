use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TrieuPhuQuestion {
    pub question: &'static str,
    pub options: [&'static str; 4],
    pub correct: usize,
}

#[derive(Debug, Clone)]
pub struct TrieuPhuSession {
    pub player_id: u64,
    pub level: usize,
    pub current: TrieuPhuQuestion,
}

const QUESTIONS: &[TrieuPhuQuestion] = &[
    TrieuPhuQuestion {
        question: "Thủ đô của Việt Nam là thành phố nào?",
        options: ["TP.HCM", "Hà Nội", "Đà Nẵng", "Huế"],
        correct: 1,
    },
    TrieuPhuQuestion {
        question: "Công thức hóa học của nước là gì?",
        options: ["CO2", "NaCl", "H2O", "O2"],
        correct: 2,
    },
    TrieuPhuQuestion {
        question: "Đỉnh núi nào cao nhất Việt Nam?",
        options: ["Fansipan", "Bà Đen", "Ngọc Linh", "Langbiang"],
        correct: 0,
    },
    TrieuPhuQuestion {
        question: "Tác giả Truyện Kiều là ai?",
        options: ["Nguyễn Trãi", "Nguyễn Du", "Hồ Xuân Hương", "Xuân Diệu"],
        correct: 1,
    },
    TrieuPhuQuestion {
        question: "Đơn vị đo cường độ dòng điện là gì?",
        options: ["Vôn (V)", "Ôm (Ω)", "Ampe (A)", "Oát (W)"],
        correct: 2,
    },
    TrieuPhuQuestion {
        question: "Chiến thắng Điện Biên Phủ diễn ra vào năm nào?",
        options: ["1945", "1954", "1968", "1975"],
        correct: 1,
    },
    TrieuPhuQuestion {
        question: "Kim loại nào dẫn điện tốt nhất ở điều kiện thường?",
        options: ["Vàng", "Đồng", "Bạc", "Nhôm"],
        correct: 2,
    },
    TrieuPhuQuestion {
        question: "Đại dương nào lớn nhất Trái Đất?",
        options: ["Đại Tây Dương", "Ấn Độ Dương", "Bắc Băng Dương", "Thái Bình Dương"],
        correct: 3,
    },
    TrieuPhuQuestion {
        question: "Nguyên tố nào có ký hiệu hóa học là Fe?",
        options: ["Sắt", "Đồng", "Kẽm", "Chì"],
        correct: 0,
    },
    TrieuPhuQuestion {
        question: "Ai là người đầu tiên đặt chân lên Mặt Trăng năm 1969?",
        options: ["Yuri Gagarin", "Neil Armstrong", "Buzz Aldrin", "Michael Collins"],
        correct: 1,
    },
];

pub const MONEY_LADDER: [&str; 15] = [
    "200.000", "400.000", "600.000", "1.000.000", "2.000.000",
    "3.000.000", "6.000.000", "10.000.000", "14.000.000", "22.000.000",
    "30.000.000", "40.000.000", "60.000.000", "85.000.000", "150.000.000",
];

#[derive(Clone, Default)]
pub struct TrieuPhuManager {
    sessions: Arc<DashMap<u64, TrieuPhuSession>>,
}

impl TrieuPhuManager {
    pub fn start(&self, channel_id: u64, player_id: u64) -> TrieuPhuSession {
        let q = QUESTIONS
            .choose(&mut rand::thread_rng())
            .cloned()
            .unwrap_or_else(|| QUESTIONS[0].clone());
        let sess = TrieuPhuSession {
            player_id,
            level: 1,
            current: q,
        };
        self.sessions.insert(channel_id, sess.clone());
        sess
    }

    pub fn answer(
        &self,
        channel_id: u64,
        ans_idx: usize,
    ) -> Option<(bool, usize, &'static str, Option<TrieuPhuSession>)> {
        let mut entry = self.sessions.get_mut(&channel_id)?;
        let correct = entry.current.correct;
        let curr_lvl = entry.level;
        if ans_idx == correct {
            if curr_lvl >= 15 {
                drop(entry);
                self.sessions.remove(&channel_id);
                return Some((true, 15, MONEY_LADDER[14], None));
            }
            entry.level += 1;
            entry.current = QUESTIONS
                .choose(&mut rand::thread_rng())
                .cloned()
                .unwrap_or_else(|| QUESTIONS[0].clone());
            let next = entry.clone();
            return Some((true, curr_lvl, MONEY_LADDER[curr_lvl - 1], Some(next)));
        }
        let safe = if curr_lvl > 10 {
            "22.000.000"
        } else if curr_lvl > 5 {
            "2.000.000"
        } else {
            "0"
        };
        drop(entry);
        self.sessions.remove(&channel_id);
        Some((false, curr_lvl, safe, None))
    }
}
