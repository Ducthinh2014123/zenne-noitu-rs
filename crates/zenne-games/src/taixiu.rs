use rand::Rng;

pub struct TaiXiuResult {
    pub dice: [u8; 3],
    pub total: u8,
    pub is_tai: bool,
    pub is_triple: bool,
}

pub fn roll_taixiu() -> TaiXiuResult {
    let mut rng = rand::thread_rng();
    let d1 = rng.gen_range(1..=6);
    let d2 = rng.gen_range(1..=6);
    let d3 = rng.gen_range(1..=6);
    let total = d1 + d2 + d3;
    let is_triple = d1 == d2 && d2 == d3;
    let is_tai = (11..=17).contains(&total) && !is_triple;
    TaiXiuResult {
        dice: [d1, d2, d3],
        total,
        is_tai,
        is_triple,
    }
}
