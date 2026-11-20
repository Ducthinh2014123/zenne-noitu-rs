use dashmap::DashMap;
use rand::seq::SliceRandom;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use zenne_core::AppContext;

const MS_COLS: usize = 9;
const MS_ROWS: usize = 9;
const MS_MINES: usize = 10;

#[derive(Clone, Default)]
pub struct TttSession {
    pub board: [Option<char>; 9],
    pub result: Option<String>,
    pub streak: i64,
}

#[derive(Clone, Default)]
pub struct MsSession {
    pub mines: Option<HashSet<usize>>,
    pub revealed: HashSet<usize>,
    pub over: bool,
    pub start_time: Option<Instant>,
}

pub type TttSessionsMap = Arc<DashMap<i64, TttSession>>;
pub type MsSessionsMap = Arc<DashMap<i64, MsSession>>;
pub type UsedNoncesMap = Arc<DashMap<String, i64>>;

fn ttt_winner(b: &[Option<char>; 9]) -> Option<&'static str> {
    const LINES: [[usize; 3]; 8] = [
        [0, 1, 2],
        [3, 4, 5],
        [6, 7, 8],
        [0, 3, 6],
        [1, 4, 7],
        [2, 5, 8],
        [0, 4, 8],
        [2, 4, 6],
    ];
    for [a, c, d] in LINES {
        if let (Some(x), Some(y), Some(z)) = (b[a], b[c], b[d]) {
            if x == y && y == z {
                return Some(if x == 'X' { "X" } else { "O" });
            }
        }
    }
    if b.iter().all(|c| c.is_some()) {
        return Some("draw");
    }
    None
}

fn minimax(b: &mut [Option<char>; 9], is_ai: bool, depth: i32) -> i32 {
    if let Some(w) = ttt_winner(b) {
        return match w {
            "O" => 10 - depth,
            "X" => depth - 10,
            _ => 0,
        };
    }
    if is_ai {
        let mut best = i32::MIN;
        for i in 0..9 {
            if b[i].is_none() {
                b[i] = Some('O');
                best = best.max(minimax(b, false, depth + 1));
                b[i] = None;
            }
        }
        best
    } else {
        let mut best = i32::MAX;
        for i in 0..9 {
            if b[i].is_none() {
                b[i] = Some('X');
                best = best.min(minimax(b, true, depth + 1));
                b[i] = None;
            }
        }
        best
    }
}

fn ttt_ai_move(b: &[Option<char>; 9]) -> Option<usize> {
    let mut best_score = i32::MIN;
    let mut best_idx = None;
    let mut copy = *b;
    for i in 0..9 {
        if copy[i].is_none() {
            copy[i] = Some('O');
            let score = minimax(&mut copy, false, 0);
            copy[i] = None;
            if score > best_score {
                best_score = score;
                best_idx = Some(i);
            }
        }
    }
    best_idx
}

fn board_to_json(b: &[Option<char>; 9]) -> Vec<Value> {
    b.iter()
        .map(|c| match c {
            Some(ch) => json!(ch.to_string()),
            None => Value::Null,
        })
        .collect()
}

pub async fn handle_ttt(
    ctx: &AppContext,
    sessions: &TttSessionsMap,
    body: &Value,
) -> Result<Value, (u16, String)> {
    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or((400, "user_id khong hop le".to_string()))?;
    let action = body
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let mut sess = sessions.entry(user_id).or_default();
    if action == "new" {
        sess.board = [None; 9];
        sess.result = None;
        return Ok(json!({
            "board": board_to_json(&sess.board),
            "result": Value::Null,
            "streak": sess.streak,
        }));
    }
    if action != "move" {
        return Err((400, "Action khong hop le".to_string()));
    }
    let idx = body
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .ok_or((400, "index khong hop le".to_string()))?;
    if sess.result.is_some() {
        return Err((400, "Van da ket thuc, hay bat dau van moi".to_string()));
    }
    if idx > 8 || sess.board[idx].is_some() {
        return Err((400, "O da duoc danh hoac khong hop le".to_string()));
    }

    sess.board[idx] = Some('X');
    let mut win = ttt_winner(&sess.board);
    if win.is_none() {
        if let Some(ai_idx) = ttt_ai_move(&sess.board) {
            sess.board[ai_idx] = Some('O');
        }
        win = ttt_winner(&sess.board);
    }

    let mut won_streak = None;
    match win {
        Some("X") => {
            sess.result = Some("X".to_string());
            sess.streak += 1;
            won_streak = Some(sess.streak);
        }
        Some("O") => {
            sess.result = Some("O".to_string());
            sess.streak = 0;
        }
        Some("draw") => {
            sess.result = Some("draw".to_string());
        }
        _ => {}
    }

    let resp = json!({
        "board": board_to_json(&sess.board),
        "result": sess.result,
        "streak": sess.streak,
    });
    drop(sess);

    if let Some(streak) = won_streak {
        ctx.db.game_submit_score("tictactoe", user_id, streak).await;
    }
    Ok(resp)
}

fn ms_neighbors(i: usize) -> Vec<usize> {
    let x = (i % MS_COLS) as isize;
    let y = (i / MS_COLS) as isize;
    let mut out = Vec::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            if nx >= 0 && nx < MS_COLS as isize && ny >= 0 && ny < MS_ROWS as isize {
                out.push((ny as usize) * MS_COLS + (nx as usize));
            }
        }
    }
    out
}

fn ms_adjacent_count(mines: &HashSet<usize>, i: usize) -> usize {
    ms_neighbors(i)
        .into_iter()
        .filter(|n| mines.contains(n))
        .count()
}

fn ms_flood(mines: &HashSet<usize>, revealed: &mut HashSet<usize>, start_idx: usize) -> Vec<Value> {
    let mut stack = vec![start_idx];
    let mut newly = Vec::new();
    while let Some(i) = stack.pop() {
        if revealed.contains(&i) || mines.contains(&i) {
            continue;
        }
        revealed.insert(i);
        let adj = ms_adjacent_count(mines, i);
        newly.push(json!({ "index": i, "adjacent": adj }));
        if adj == 0 {
            for n in ms_neighbors(i) {
                if !revealed.contains(&n) && !mines.contains(&n) {
                    stack.push(n);
                }
            }
        }
    }
    newly
}

pub async fn handle_ms(
    ctx: &AppContext,
    sessions: &MsSessionsMap,
    body: &Value,
) -> Result<Value, (u16, String)> {
    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or((400, "user_id khong hop le".to_string()))?;
    let action = body
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let mut sess = sessions.entry(user_id).or_default();
    if action == "new" {
        sess.mines = None;
        sess.revealed.clear();
        sess.over = false;
        sess.start_time = None;
        return Ok(json!({
            "cols": MS_COLS,
            "rows": MS_ROWS,
            "mines": MS_MINES,
        }));
    }
    if action != "reveal" {
        return Err((400, "Action khong hop le".to_string()));
    }
    if sess.over {
        return Err((400, "Van da ket thuc, hay bat dau van moi".to_string()));
    }
    let idx = body
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .ok_or((400, "index khong hop le".to_string()))?;
    let total = MS_COLS * MS_ROWS;
    if idx >= total {
        return Err((400, "O khong hop le".to_string()));
    }
    if sess.revealed.contains(&idx) {
        return Ok(json!({ "result": "playing", "revealed": [] }));
    }

    if sess.mines.is_none() {
        let mut all_idx: Vec<usize> = (0..total).filter(|&i| i != idx).collect();
        all_idx.shuffle(&mut rand::thread_rng());
        let mines: HashSet<usize> = all_idx.into_iter().take(MS_MINES).collect();
        sess.mines = Some(mines);
        sess.start_time = Some(Instant::now());
    }

    let mines = sess.mines.as_ref().unwrap().clone();
    if mines.contains(&idx) {
        sess.over = true;
        let mut sorted_mines: Vec<usize> = mines.into_iter().collect();
        sorted_mines.sort_unstable();
        return Ok(json!({ "result": "lost", "mines": sorted_mines }));
    }

    let newly = ms_flood(&mines, &mut sess.revealed, idx);
    let won = sess.revealed.len() + mines.len() >= total;
    if won {
        sess.over = true;
        let elapsed = sess
            .start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        let score = (1000 - (elapsed * 3.0) as i64).max(0);
        drop(sess);
        ctx.db
            .game_submit_score("minesweeper", user_id, score)
            .await;
        return Ok(json!({
            "result": "won",
            "revealed": newly,
            "elapsed": (elapsed * 10.0).round() / 10.0,
            "score": score,
        }));
    }

    Ok(json!({ "result": "playing", "revealed": newly }))
}

pub const ALLOWED_GAMES: &[&str] = &[
    "2048",
    "snake",
    "minesweeper",
    "tictactoe",
    "afk",
    "wordle",
    "trivia",
    "math",
    "scramble",
    "noitu",
    "taixiu",
    "baucua",
    "trieuphu",
    "typing",
    "memory",
    "flappy",
    "tetris",
];

pub fn sanity_check_score(game: &str, score: i64) -> bool {
    if score < 0 {
        return false;
    }
    let g = game.trim().to_lowercase();
    if !ALLOWED_GAMES.contains(&g.as_str()) {
        return false;
    }
    match g.as_str() {
        "2048" => score <= 500_000,
        "snake" => score <= 10_000,
        "minesweeper" => score <= 1_000,
        "tictactoe" => score <= 1_000,
        "afk" => score <= 86_400,
        _ => score <= 1_000_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttt_minimax_and_sanity_checks() {
        let mut board = [None; 9];
        board[0] = Some('X');
        board[1] = Some('X');
        // AI ('O') must block index 2
        assert_eq!(ttt_ai_move(&board), Some(2));
        assert!(sanity_check_score("2048", 2048));
        assert!(!sanity_check_score("2048", 999_999));
        assert!(!sanity_check_score("snake", -1));
        assert!(sanity_check_score("  2048 ", 2048));
        assert!(!sanity_check_score("unknown_game", 10));
    }
}
