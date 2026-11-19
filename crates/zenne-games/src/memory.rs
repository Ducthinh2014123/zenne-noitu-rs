use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MemoryGame {
    pub cards: Vec<&'static str>,
    pub revealed: Vec<bool>,
    pub moves: u32,
}

#[derive(Clone, Default)]
pub struct MemoryManager {
    games: Arc<DashMap<u64, MemoryGame>>,
}

impl MemoryManager {
    pub fn start(&self, channel_id: u64) -> String {
        let mut cards = vec![
            "🍎", "🍎", "🍌", "🍌", "🍇", "🍇", "🍉", "🍉", "🍓", "🍓", "🍒", "🍒", "🥝", "🥝",
            "🍍", "🍍",
        ];
        cards.shuffle(&mut rand::thread_rng());
        let g = MemoryGame {
            cards,
            revealed: vec![false; 16],
            moves: 0,
        };
        let view = Self::render(&g, None);
        self.games.insert(channel_id, g);
        view
    }

    pub fn render(g: &MemoryGame, temp: Option<(usize, usize)>) -> String {
        let mut s = String::new();
        for i in 0..16 {
            let show = g.revealed[i] || temp.map(|(a, b)| a == i || b == i).unwrap_or(false);
            if show {
                s.push_str(g.cards[i]);
            } else {
                s.push_str(&format!("`{:02}`", i + 1));
            }
            s.push(if (i + 1) % 4 == 0 { '\n' } else { ' ' });
        }
        s
    }

    pub fn flip(
        &self,
        channel_id: u64,
        a_1: usize,
        b_1: usize,
    ) -> Result<(String, bool, bool, u32), String> {
        if !(1..=16).contains(&a_1) || !(1..=16).contains(&b_1) || a_1 == b_1 {
            return Err("Chọn 2 số khác nhau từ 1 đến 16 (vd: `1 5`)!".to_string());
        }
        let (a, b) = (a_1 - 1, b_1 - 1);
        let mut g = self
            .games
            .get_mut(&channel_id)
            .ok_or("Chưa có ván Memory nào!")?;
        if g.revealed[a] || g.revealed[b] {
            return Err("Một trong hai thẻ đã được lật mở trước đó!".to_string());
        }
        g.moves += 1;
        let matched = g.cards[a] == g.cards[b];
        if matched {
            g.revealed[a] = true;
            g.revealed[b] = true;
        }
        let view = Self::render(&g, Some((a, b)));
        let won = g.revealed.iter().all(|&x| x);
        let moves = g.moves;
        if won {
            drop(g);
            self.games.remove(&channel_id);
        }
        Ok((view, matched, won, moves))
    }
}

#[derive(Debug, Clone)]
pub struct UnoGame {
    pub players: Vec<u64>,
    pub top_card: String,
    pub turn_idx: usize,
}

#[derive(Clone, Default)]
pub struct UnoManager {
    pub games: Arc<DashMap<u64, UnoGame>>,
}

impl UnoManager {
    pub fn start(&self, channel_id: u64, host: u64) -> String {
        let colors = ["🔴", "🟡", "🟢", "🔵"];
        let mut rng = rand::thread_rng();
        let top = format!(
            "{} {}",
            colors.choose(&mut rng).unwrap(),
            rand::Rng::gen_range(&mut rng, 1..=9)
        );
        self.games.insert(
            channel_id,
            UnoGame {
                players: vec![host],
                top_card: top.clone(),
                turn_idx: 0,
            },
        );
        top
    }
}
