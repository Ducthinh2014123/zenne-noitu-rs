use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Connect4Game {
    pub board: [[u8; 7]; 6], // 0: trống, 1: người chơi 1, 2: người chơi 2
    pub turn: u8,
    pub players: [u64; 2],
}

#[derive(Clone, Default)]
pub struct Connect4Manager {
    games: Arc<DashMap<u64, Connect4Game>>,
}

impl Connect4Manager {
    pub fn start(&self, channel_id: u64, p1: u64, p2: u64) -> Connect4Game {
        let g = Connect4Game {
            board: [[0; 7]; 6],
            turn: 1,
            players: [p1, p2],
        };
        self.games.insert(channel_id, g.clone());
        g
    }

    pub fn render(board: &[[u8; 7]; 6]) -> String {
        let mut s = String::new();
        for row in board {
            for cell in row {
                s.push_str(match cell {
                    1 => "🔴",
                    2 => "🟡",
                    _ => "⚪",
                });
            }
            s.push('\n');
        }
        s.push_str("1️⃣2️⃣3️⃣4️⃣5️⃣6️⃣7️⃣");
        s
    }

    pub fn drop_piece(
        &self,
        channel_id: u64,
        player_id: u64,
        col_1_based: usize,
    ) -> Result<(String, Option<u8>, bool), String> {
        if !(1..=7).contains(&col_1_based) {
            return Err("Cột phải từ 1 đến 7!".to_string());
        }
        let col = col_1_based - 1;
        let mut g = self
            .games
            .get_mut(&channel_id)
            .ok_or_else(|| "Chưa có ván Connect4 nào!".to_string())?;
        let expected = g.players[(g.turn - 1) as usize];
        if expected != 0 && player_id != expected {
            return Err("Chưa tới lượt bạn!".to_string());
        }

        let mut placed_row = None;
        for r in (0..6).rev() {
            if g.board[r][col] == 0 {
                g.board[r][col] = g.turn;
                placed_row = Some(r);
                break;
            }
        }
        let Some(_) = placed_row else {
            return Err("Cột này đã đầy!".to_string());
        };

        let winner = Self::check_winner(&g.board);
        let draw = winner.is_none() && g.board[0].iter().all(|&c| c != 0);
        let rendered = Self::render(&g.board);
        if winner.is_some() || draw {
            drop(g);
            self.games.remove(&channel_id);
        } else {
            g.turn = if g.turn == 1 { 2 } else { 1 };
        }
        Ok((rendered, winner, draw))
    }

    fn check_winner(b: &[[u8; 7]; 6]) -> Option<u8> {
        for r in 0..6 {
            for c in 0..7 {
                let p = b[r][c];
                if p == 0 {
                    continue;
                }
                if c + 3 < 7 && b[r][c + 1] == p && b[r][c + 2] == p && b[r][c + 3] == p {
                    return Some(p);
                }
                if r + 3 < 6 && b[r + 1][c] == p && b[r + 2][c] == p && b[r + 3][c] == p {
                    return Some(p);
                }
                if r + 3 < 6
                    && c + 3 < 7
                    && b[r + 1][c + 1] == p
                    && b[r + 2][c + 2] == p
                    && b[r + 3][c + 3] == p
                {
                    return Some(p);
                }
                if r >= 3
                    && c + 3 < 7
                    && b[r - 1][c + 1] == p
                    && b[r - 2][c + 2] == p
                    && b[r - 3][c + 3] == p
                {
                    return Some(p);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct TypingSession {
    pub sentence: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

const TYPING_SENTENCES: &[&str] = &[
    "Con đường thành công không có dấu chân của kẻ lười biếng.",
    "Học, học nữa, học mãi để vươn tới những tầm cao mới.",
    "Rust giúp chương trình chạy nhanh, an toàn bộ nhớ và tiết kiệm tài nguyên.",
    "Bầu trời hôm nay trong xanh và gió thổi nhẹ nhàng qua từng tán lá.",
];

#[derive(Clone, Default)]
pub struct TypingTestManager {
    sessions: Arc<DashMap<u64, TypingSession>>,
}

impl TypingTestManager {
    pub fn start(&self, channel_id: u64) -> String {
        let sentence = TYPING_SENTENCES
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or(TYPING_SENTENCES[0])
            .to_string();
        self.sessions.insert(
            channel_id,
            TypingSession {
                sentence: sentence.clone(),
                started_at: chrono::Utc::now(),
            },
        );
        sentence
    }

    pub fn submit(&self, channel_id: u64, typed: &str) -> Option<(bool, f64, f64)> {
        let sess = self.sessions.get(&channel_id)?;
        let elapsed = (chrono::Utc::now() - sess.started_at)
            .num_milliseconds()
            .max(500) as f64
            / 1000.0;
        let ok = typed.trim() == sess.sentence;
        let words = sess.sentence.split_whitespace().count() as f64;
        let wpm = (words / elapsed) * 60.0;
        if ok {
            drop(sess);
            self.sessions.remove(&channel_id);
        }
        Some((ok, elapsed, wpm))
    }
}
