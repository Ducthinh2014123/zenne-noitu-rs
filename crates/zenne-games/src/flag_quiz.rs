use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::sync::Arc;

const FLAG_BANK: &[(&str, &[&str])] = &[
    ("🇻🇳", &["việt nam", "vietnam", "viet nam"]),
    ("🇯🇵", &["nhật bản", "japan", "nhat ban"]),
    ("🇰🇷", &["hàn quốc", "south korea", "korea", "han quoc"]),
    ("🇺🇸", &["mỹ", "hoa kỳ", "usa", "united states", "america"]),
    ("🇬🇧", &["anh", "uk", "united kingdom", "england"]),
    ("🇫🇷", &["pháp", "france", "phap"]),
    ("🇩🇪", &["đức", "germany", "duc"]),
    ("🇨🇳", &["trung quốc", "china", "trung quoc"]),
    ("🇹🇭", &["thái lan", "thailand", "thai lan"]),
    ("🇸🇬", &["singapore"]),
    ("🇧🇷", &["brazil", "braxin"]),
    ("🇨🇦", &["canada"]),
    ("🇦🇺", &["úc", "australia", "uc"]),
    ("🇷🇺", &["nga", "russia"]),
    ("🇮🇹", &["ý", "italy", "italia"]),
    ("🇪🇸", &["tây ban nha", "spain", "tay ban nha"]),
];

#[derive(Clone, Default)]
pub struct FlagQuizManager {
    sessions: Arc<DashMap<u64, usize>>,
}

impl FlagQuizManager {
    pub fn start(&self, channel_id: u64) -> (&'static str, &'static str) {
        let mut rng = rand::thread_rng();
        let indices: Vec<usize> = (0..FLAG_BANK.len()).collect();
        let idx = *indices.choose(&mut rng).unwrap_or(&0);
        self.sessions.insert(channel_id, idx);
        (FLAG_BANK[idx].0, FLAG_BANK[idx].1[0])
    }

    pub fn check_answer(&self, channel_id: u64, guess: &str) -> Option<(bool, &'static str)> {
        let idx = *self.sessions.get(&channel_id)?;
        let g = guess.trim().to_lowercase();
        let (display, accepted) = (FLAG_BANK[idx].1[0], FLAG_BANK[idx].1);
        if accepted.iter().any(|a| *a == g) {
            self.sessions.remove(&channel_id);
            Some((true, display))
        } else {
            Some((false, display))
        }
    }
}
