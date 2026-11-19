use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const WORDLE_WORDS: &[&str] = &[
    "APPLE", "BRAVE", "CHAIR", "DREAM", "EAGLE", "FLAME", "GRAPE", "HOUSE", "IMAGE", "JUICE",
    "KNIFE", "LIGHT", "MAGIC", "NIGHT", "OCEAN", "PIANO", "QUEEN", "RIVER", "SMILE", "TIGER",
    "UNITY", "VOICE", "WATER", "YOUTH", "ZEBRA", "BREAD", "CLOUD", "DANCE", "EARTH", "FROST",
    "GHOST", "HEART", "INDEX", "JOKER", "KARMA", "LEMON", "MONEY", "NURSE", "OLIVE", "PEARL",
];

#[derive(Debug, Clone)]
pub struct WordleGame {
    pub target: String,
    pub attempts: Vec<String>,
    pub max_attempts: usize,
}

#[derive(Clone, Default)]
pub struct WordleManager {
    games: Arc<DashMap<u64, WordleGame>>,
}

impl WordleManager {
    pub fn start(&self, channel_id: u64) -> String {
        let target = WORDLE_WORDS
            .choose(&mut rand::thread_rng())
            .unwrap_or(&"APPLE")
            .to_string();
        self.games.insert(
            channel_id,
            WordleGame {
                target: target.clone(),
                attempts: Vec::new(),
                max_attempts: 6,
            },
        );
        target
    }

    pub fn guess(
        &self,
        channel_id: u64,
        raw: &str,
    ) -> Result<(String, bool, bool, usize, String), String> {
        let guess = raw.trim().to_uppercase();
        if guess.len() != 5 || !guess.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err("Từ đoán phải gồm đúng 5 chữ cái tiếng Anh (A-Z)!".to_string());
        }
        let mut entry = self.games.get_mut(&channel_id).ok_or_else(|| {
            "Chưa có ván Wordle nào đang chạy! Dùng lệnh bắt đầu trước.".to_string()
        })?;

        let target_chars: Vec<char> = entry.target.chars().collect();
        let guess_chars: Vec<char> = guess.chars().collect();
        let mut result_emojis = ['⬛'; 5];
        let mut used = [false; 5];

        for i in 0..5 {
            if guess_chars[i] == target_chars[i] {
                result_emojis[i] = '🟩';
                used[i] = true;
            }
        }
        for i in 0..5 {
            if result_emojis[i] == '🟩' {
                continue;
            }
            for j in 0..5 {
                if !used[j] && guess_chars[i] == target_chars[j] {
                    result_emojis[i] = '🟨';
                    used[j] = true;
                    break;
                }
            }
        }

        let feedback: String = result_emojis.iter().collect();
        entry.attempts.push(format!("`{}` {}", guess, feedback));
        let won = guess == entry.target;
        let game_over = won || entry.attempts.len() >= entry.max_attempts;
        let tries = entry.attempts.len();
        let target = entry.target.clone();
        let history = entry.attempts.join("\n");

        if game_over {
            drop(entry);
            self.games.remove(&channel_id);
        }

        Ok((history, won, game_over, tries, target))
    }
}
