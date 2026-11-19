use dashmap::DashMap;
use rand::Rng;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct HigherLowerSession {
    pub current_num: i32,
    pub streak: u32,
}

#[derive(Clone, Default)]
pub struct HigherLowerManager {
    sessions: Arc<DashMap<u64, HigherLowerSession>>,
}

impl HigherLowerManager {
    pub fn start(&self, channel_id: u64) -> i32 {
        let num = rand::thread_rng().gen_range(1..=100);
        self.sessions.insert(
            channel_id,
            HigherLowerSession {
                current_num: num,
                streak: 0,
            },
        );
        num
    }

    /// guess_higher = true nếu đoán số tiếp theo lớn hơn
    pub fn guess(&self, channel_id: u64, guess_higher: bool) -> Option<(bool, i32, i32, u32)> {
        let mut entry = self.sessions.get_mut(&channel_id)?;
        let prev = entry.current_num;
        let next = rand::thread_rng().gen_range(1..=100);
        let won = if next == prev {
            true
        } else if guess_higher {
            next > prev
        } else {
            next < prev
        };

        if won {
            entry.current_num = next;
            entry.streak += 1;
            Some((true, prev, next, entry.streak))
        } else {
            let final_streak = entry.streak;
            drop(entry);
            self.sessions.remove(&channel_id);
            Some((false, prev, next, final_streak))
        }
    }
}
