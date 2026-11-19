use dashmap::DashMap;
use rand::seq::SliceRandom;
use rand::Rng;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct QuizQuestion {
    pub category: String,
    pub question: String,
    pub options: [String; 4],
    pub correct_index: usize, // 0=A, 1=B, 2=C, 3=D
}

const GENERAL_QUIZ: &[(&str, &str, [&str; 4], usize)] = &[
    (
        "Địa lý",
        "Thủ đô của Việt Nam là gì?",
        ["TP.HCM", "Hà Nội", "Đà Nẵng", "Huế"],
        1,
    ),
    (
        "Khoa học",
        "Công thức hóa học của nước là gì?",
        ["CO2", "H2O", "O2", "NaCl"],
        1,
    ),
    (
        "Vũ trụ",
        "Hành tinh nào gần Mặt Trời nhất?",
        ["Sao Kim", "Trái Đất", "Sao Thủy", "Sao Hỏa"],
        2,
    ),
    (
        "Công nghệ",
        "Ngôn ngữ lập trình nào nổi tiếng với cơ chế Ownership không cần Garbage Collector?",
        ["Python", "Java", "Rust", "Go"],
        2,
    ),
    (
        "Sinh học",
        "Động vật nào chạy nhanh nhất trên cạn?",
        ["Sư tử", "Báo săn", "Ngựa vằn", "Linh dương"],
        1,
    ),
    (
        "Toán học",
        "Tổng các góc trong một tam giác bằng bao nhiêu độ?",
        ["90°", "180°", "270°", "360°"],
        1,
    ),
    (
        "Vật lý",
        "Đơn vị đo cường độ dòng điện là gì?",
        ["Vôn (V)", "Ôm (Ω)", "Ampe (A)", "Oát (W)"],
        2,
    ),
    (
        "Lịch sử",
        "Năm nào con người lần đầu tiên đặt chân lên Mặt Trăng?",
        ["1965", "1969", "1972", "1975"],
        1,
    ),
];

#[derive(Clone, Default)]
pub struct QuizManager {
    active: Arc<DashMap<u64, QuizQuestion>>,
}

impl QuizManager {
    pub fn start_general(&self, channel_id: u64) -> QuizQuestion {
        let (cat, q, opts, idx) = GENERAL_QUIZ
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(GENERAL_QUIZ[0]);
        let qq = QuizQuestion {
            category: cat.to_string(),
            question: q.to_string(),
            options: opts.map(|s| s.to_string()),
            correct_index: idx,
        };
        self.active.insert(channel_id, qq.clone());
        qq
    }

    pub fn start_math(&self, channel_id: u64) -> QuizQuestion {
        let mut rng = rand::thread_rng();
        let a = rng.gen_range(10..=99);
        let b = rng.gen_range(2..=30);
        let op = rng.gen_range(0..3);
        let (q, ans) = match op {
            0 => (format!("{} + {} = ?", a, b), a + b),
            1 => (format!("{} - {} = ?", a + b, b), a),
            _ => (
                format!("{} × {} = ?", a % 20 + 2, b % 12 + 2),
                (a % 20 + 2) * (b % 12 + 2),
            ),
        };
        let mut opts = [ans, ans + 2, ans - 3, ans + 10];
        opts.shuffle(&mut rng);
        let correct_index = opts.iter().position(|&x| x == ans).unwrap_or(0);
        let qq = QuizQuestion {
            category: "Toán học".to_string(),
            question: q,
            options: opts.map(|x| x.to_string()),
            correct_index,
        };
        self.active.insert(channel_id, qq.clone());
        qq
    }

    pub fn answer(&self, channel_id: u64, choice: &str) -> Option<(bool, String)> {
        let qq = self.active.get(&channel_id)?;
        let c = choice.trim().to_uppercase();
        let idx = match c.as_str() {
            "A" | "1" => Some(0),
            "B" | "2" => Some(1),
            "C" | "3" => Some(2),
            "D" | "4" => Some(3),
            _ => None,
        };
        let correct_letter = ["A", "B", "C", "D"][qq.correct_index];
        let ans_text = format!("{}. {}", correct_letter, qq.options[qq.correct_index]);
        if let Some(i) = idx {
            let ok = i == qq.correct_index;
            drop(qq);
            if ok {
                self.active.remove(&channel_id);
            }
            Some((ok, ans_text))
        } else if choice
            .trim()
            .eq_ignore_ascii_case(&qq.options[qq.correct_index])
        {
            drop(qq);
            self.active.remove(&channel_id);
            Some((true, ans_text))
        } else {
            Some((false, ans_text))
        }
    }
}
