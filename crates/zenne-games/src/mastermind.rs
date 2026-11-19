use dashmap::DashMap;
use rand::Rng;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MastermindGame {
    pub secret: [u8; 4], // 4 số từ 1..=6
    pub attempts: Vec<String>,
}

#[derive(Clone, Default)]
pub struct MastermindManager {
    games: Arc<DashMap<u64, MastermindGame>>,
}

impl MastermindManager {
    pub fn start(&self, channel_id: u64) {
        let mut rng = rand::thread_rng();
        let secret = [
            rng.gen_range(1..=6),
            rng.gen_range(1..=6),
            rng.gen_range(1..=6),
            rng.gen_range(1..=6),
        ];
        self.games.insert(
            channel_id,
            MastermindGame {
                secret,
                attempts: Vec::new(),
            },
        );
    }

    pub fn guess(
        &self,
        channel_id: u64,
        raw: &str,
    ) -> Result<(String, bool, bool, String), String> {
        let digits: Vec<u8> = raw
            .chars()
            .filter_map(|c| c.to_digit(10).map(|d| d as u8))
            .collect();
        if digits.len() != 4 || digits.iter().any(|&d| !(1..=6).contains(&d)) {
            return Err("Vui lòng nhập đúng 4 chữ số từ 1 đến 6 (vd: `1234`)!".to_string());
        }
        let mut game = self
            .games
            .get_mut(&channel_id)
            .ok_or_else(|| "Chưa có ván Mastermind nào đang mở!".to_string())?;

        let mut exact = 0;
        let mut partial = 0;
        let mut used_s = [false; 4];
        let mut used_g = [false; 4];

        for i in 0..4 {
            if digits[i] == game.secret[i] {
                exact += 1;
                used_s[i] = true;
                used_g[i] = true;
            }
        }
        for i in 0..4 {
            if used_g[i] {
                continue;
            }
            for j in 0..4 {
                if !used_s[j] && digits[i] == game.secret[j] {
                    partial += 1;
                    used_s[j] = true;
                    break;
                }
            }
        }

        let guess_str = digits.iter().map(|d| d.to_string()).collect::<String>();
        game.attempts.push(format!(
            "`{}` → 🎯 {} đúng vị trí | 🟡 {} sai vị trí",
            guess_str, exact, partial
        ));
        let won = exact == 4;
        let over = won || game.attempts.len() >= 10;
        let history = game.attempts.join("\n");
        let secret_str = game
            .secret
            .iter()
            .map(|d| d.to_string())
            .collect::<String>();
        if over {
            drop(game);
            self.games.remove(&channel_id);
        }
        Ok((history, won, over, secret_str))
    }
}
