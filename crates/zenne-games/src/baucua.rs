use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BauCuaFace {
    Bau,
    Cua,
    Tom,
    Ca,
    Ga,
    Nai,
}

impl BauCuaFace {
    pub fn all() -> [Self; 6] {
        [
            Self::Bau,
            Self::Cua,
            Self::Tom,
            Self::Ca,
            Self::Ga,
            Self::Nai,
        ]
    }

    pub fn from_str_vi(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "bau" | "bầu" => Some(Self::Bau),
            "cua" => Some(Self::Cua),
            "tom" | "tôm" => Some(Self::Tom),
            "ca" | "cá" => Some(Self::Ca),
            "ga" | "gà" => Some(Self::Ga),
            "nai" | "hươu" | "huou" => Some(Self::Nai),
            _ => None,
        }
    }

    pub fn display(&self) -> (&'static str, &'static str) {
        match self {
            Self::Bau => ("🍐", "Bầu"),
            Self::Cua => ("🦀", "Cua"),
            Self::Tom => ("🦐", "Tôm"),
            Self::Ca => ("🐟", "Cá"),
            Self::Ga => ("🐓", "Gà"),
            Self::Nai => ("🦌", "Nai"),
        }
    }
}

pub fn roll_baucua() -> [BauCuaFace; 3] {
    let all = BauCuaFace::all();
    let mut rng = rand::thread_rng();
    [
        all[rng.gen_range(0..6)],
        all[rng.gen_range(0..6)],
        all[rng.gen_range(0..6)],
    ]
}
