use dashmap::DashMap;
use rand::seq::SliceRandom;
use rand::Rng;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BattleshipGame {
    pub grid_size: usize,
    pub ships: Vec<(usize, usize)>,
    pub hits: Vec<(usize, usize)>,
    pub misses: Vec<(usize, usize)>,
    pub shots_left: usize,
}

#[derive(Clone, Default)]
pub struct BattleshipManager {
    games: Arc<DashMap<u64, BattleshipGame>>,
}

impl BattleshipManager {
    pub fn start(&self, channel_id: u64) -> String {
        let mut rng = rand::thread_rng();
        let mut ships = Vec::new();
        while ships.len() < 4 {
            let pos = (rng.gen_range(0..5), rng.gen_range(0..5));
            if !ships.contains(&pos) {
                ships.push(pos);
            }
        }
        let g = BattleshipGame {
            grid_size: 5,
            ships,
            hits: Vec::new(),
            misses: Vec::new(),
            shots_left: 12,
        };
        let board = Self::render(&g);
        self.games.insert(channel_id, g);
        board
    }

    pub fn render(g: &BattleshipGame) -> String {
        let mut out = String::from("  1 2 3 4 5\n");
        let rows = ['A', 'B', 'C', 'D', 'E'];
        for r in 0..5 {
            out.push(rows[r]);
            out.push(' ');
            for c in 0..5 {
                if g.hits.contains(&(r, c)) {
                    out.push_str("💥");
                } else if g.misses.contains(&(r, c)) {
                    out.push_str("💦");
                } else {
                    out.push_str("🟦");
                }
            }
            out.push('\n');
        }
        out
    }

    pub fn shoot(
        &self,
        channel_id: u64,
        coord: &str,
    ) -> Result<(String, bool, bool, usize), String> {
        let c_up = coord.trim().to_uppercase();
        let mut chars = c_up.chars();
        let r_ch = chars.next().ok_or("Tọa độ không hợp lệ (vd: A1, C4)!")?;
        let c_ch = chars.next().ok_or("Tọa độ không hợp lệ (vd: A1, C4)!")?;
        let r = match r_ch {
            'A' => 0,
            'B' => 1,
            'C' => 2,
            'D' => 3,
            'E' => 4,
            _ => return Err("Hàng phải từ A đến E!".to_string()),
        };
        let col = c_ch
            .to_digit(10)
            .and_then(|d| {
                if (1..=5).contains(&d) {
                    Some((d - 1) as usize)
                } else {
                    None
                }
            })
            .ok_or("Cột phải từ 1 đến 5!")?;

        let mut g = self
            .games
            .get_mut(&channel_id)
            .ok_or("Chưa có ván Bắn Tàu nào!")?;

        if g.hits.contains(&(r, col)) || g.misses.contains(&(r, col)) {
            return Err("Ô này đã bắn rồi!".to_string());
        }

        let is_hit = g.ships.contains(&(r, col));
        if is_hit {
            g.hits.push((r, col));
        } else {
            g.misses.push((r, col));
        }
        g.shots_left = g.shots_left.saturating_sub(1);

        let won = g.hits.len() == g.ships.len();
        let over = won || g.shots_left == 0;
        let rendered = Self::render(&g);
        let left = g.shots_left;
        if over {
            drop(g);
            self.games.remove(&channel_id);
        }
        Ok((rendered, is_hit, over, left))
    }
}

#[derive(Clone, Default)]
pub struct SudokuManager {
    puzzles: Arc<DashMap<u64, String>>,
}

impl SudokuManager {
    pub fn start(&self, channel_id: u64) -> (&'static str, &'static str) {
        let puzzles = [
            (
                "5 3 . | . 7 . | . . .\n6 . . | 1 9 5 | . . .\n. 9 8 | . . . | . 6 .\n------+-------+------\n8 . . | . 6 . | . . 3\n4 . . | 8 . 3 | . . 1\n7 . . | . 2 . | . . 6\n------+-------+------\n. 6 . | . . . | 2 8 .\n. . . | 4 1 9 | . . 5\n. . . | . 8 . | . 7 9",
                "534678912672195348198342567859761423426853791713924856961537284287419635345286179",
            ),
        ];
        let (board, sol) = puzzles.choose(&mut rand::thread_rng()).copied().unwrap();
        self.puzzles.insert(channel_id, sol.to_string());
        (board, sol)
    }
}
