mod auth_routes;
mod pub_games;
mod ws_game;

use axum::{
    extract::{ws::WebSocketUpgrade, Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use zenne_core::AppContext;

#[derive(Clone)]
pub struct ApiState {
    pub ctx: AppContext,
    pub ws_rooms: ws_game::WsRoomsMap,
    pub ttt_sessions: pub_games::TttSessionsMap,
    pub ms_sessions: pub_games::MsSessionsMap,
    pub used_nonces: pub_games::UsedNoncesMap,
}

fn check_admin_auth(headers: &HeaderMap, api_key: &str) -> bool {
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(tok) = auth.strip_prefix("Bearer ") {
            if tok == api_key {
                return true;
            }
        }
    }
    if let Some(k) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        if k == api_key {
            return true;
        }
    }
    false
}

pub fn build_router(ctx: AppContext) -> Router {
    let state = Arc::new(ApiState {
        ctx,
        ws_rooms: Arc::new(dashmap::DashMap::new()),
        ttt_sessions: Arc::new(dashmap::DashMap::new()),
        ms_sessions: Arc::new(dashmap::DashMap::new()),
        used_nonces: Arc::new(dashmap::DashMap::new()),
    });
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(ah_health))
        .route("/api/health", get(ah_health))
        .route("/api/stats", get(ah_stats))
        .route("/api/pending", get(ah_pending))
        .route("/api/review", post(ah_review))
        .route("/api/review/bulk", post(ah_review_bulk))
        .route(
            "/api/wordbank",
            get(ah_wordbank_get).delete(ah_wordbank_del),
        )
        .route("/api/leaderboard", get(ah_leaderboard))
        .route("/api/guilds", get(ah_guilds))
        // Đường dẫn chuẩn cho Web Frontend (khớp 100% bot.py)
        .route("/pub/status", get(ah_pub_status))
        .route("/pub/wordbank", get(ah_pub_wordbank))
        .route("/pub/rejected", get(ah_pub_rejected))
        .route("/pub/servers", get(ah_pub_servers))
        .route("/pub/milestones", get(ah_pub_milestones))
        .route("/pub/leaderboard", get(ah_pub_leaderboard))
        .route("/pub/games", get(ah_pub_games))
        .route("/pub/game/score", post(ah_pub_game_score))
        .route("/pub/game/leaderboard", get(ah_pub_game_leaderboard))
        .route("/pub/game/session/start", post(ah_pub_game_session_start))
        .route("/pub/game/ttt", post(ah_pub_game_ttt))
        .route("/pub/game/ms", post(ah_pub_game_ms))
        .route("/pub/game/create", post(ah_pub_create_room))
        .route("/pub/multiplayer/rooms", get(ah_pub_mp_rooms))
        .route("/pub/multiplayer/create", post(ah_pub_mp_create))
        .route("/pub/multiplayer/token/:token", get(ah_pub_mp_token_info))
        .route("/pub/ai/ask", post(ah_pub_ai_ask))
        .route("/ws/game/:room_id", get(ah_ws_game_room))
        // Auth & Account & Recovery
        .route("/pub/auth/register", post(ah_auth_register))
        .route(
            "/pub/auth/register/email/verify",
            post(ah_register_email_verify),
        )
        .route(
            "/pub/auth/register/email/resend",
            post(ah_register_email_resend),
        )
        .route("/pub/auth/login", post(ah_auth_login))
        .route("/pub/auth/verify-2fa", post(ah_auth_verify_2fa))
        .route("/pub/auth/oauth", post(ah_auth_oauth))
        .route("/pub/auth/game-token", post(ah_pub_game_token))
        .route("/pub/account", get(ah_pub_account_info))
        .route(
            "/pub/account/change-password",
            post(ah_pub_account_change_password),
        )
        .route("/pub/account/setup-2fa", post(ah_pub_account_setup_2fa))
        .route("/pub/account/enable-2fa", post(ah_pub_account_enable_2fa))
        .route("/pub/account/disable-2fa", post(ah_pub_account_disable_2fa))
        .route("/pub/auth/recovery/request", post(ah_recovery_request))
        .route(
            "/pub/auth/recovery/email/verify",
            post(ah_recovery_email_verify),
        )
        .route(
            "/pub/auth/recovery/token/verify",
            post(ah_recovery_token_verify),
        )
        .route("/pub/auth/recovery/status", post(ah_recovery_status))
        .route(
            "/pub/auth/recovery/reset-password",
            post(ah_recovery_reset_password),
        )
        // Alias /api/pub/*
        .route("/api/pub/status", get(ah_pub_status))
        .route("/api/pub/leaderboard", get(ah_pub_leaderboard))
        .route("/api/pub/games", get(ah_pub_games))
        .route("/api/pub/game/score", post(ah_pub_game_score))
        .route("/api/pub/game/leaderboard", get(ah_pub_game_leaderboard))
        .route("/api/pub/mp/rooms", get(ah_pub_mp_rooms))
        .route("/api/pub/auth/login", post(ah_auth_login))
        .route("/api/pub/ai/ask", post(ah_pub_ai_ask))
        .route("/ws/game", get(ah_ws_game))
        .layer(cors)
        .with_state(state)
}

pub async fn start_web_api(ctx: AppContext) {
    let port = ctx.config.api_port;
    let app = build_router(ctx);
    let addr_v6 = SocketAddr::from(([0u16; 8], port));
    let addr_v4 = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = match tokio::net::TcpListener::bind(addr_v6).await {
        Ok(l) => l,
        Err(_) => match tokio::net::TcpListener::bind(addr_v4).await {
            Ok(l) => l,
            Err(e) => {
                error!("[Web API] Khong the bind port {}: {}", port, e);
                return;
            }
        },
    };
    info!("[Web API] Axum HTTP/WS server dang chay tren port {}", port);
    if let Err(e) = axum::serve(listener, app).await {
        error!("[Web API] Server loi: {}", e);
    }
}

async fn ah_health() -> impl IntoResponse {
    Json(json!({ "status": "ok", "engine": "rust-axum" }))
}

async fn ah_stats(headers: HeaderMap, State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    let summary = st.ctx.db.stats_summary().await;
    Json(summary).into_response()
}

async fn ah_pending(
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    let gid = q.get("guild_id").and_then(|v| v.parse::<i64>().ok());
    let status = q.get("status").map(|s| s.as_str()).unwrap_or("pending");
    let rows = st.ctx.db.get_pending(gid, status).await;
    Json(json!({ "items": rows })).into_response()
}

#[derive(Deserialize)]
struct ReviewBody {
    id: i64,
    approve: bool,
    reviewer_id: Option<i64>,
}

async fn ah_review(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
    Json(body): Json<ReviewBody>,
) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    let res = st
        .ctx
        .db
        .review_word(body.id, body.approve, body.reviewer_id.unwrap_or(0))
        .await;
    Json(json!({ "ok": res.is_some() })).into_response()
}

async fn ah_leaderboard(
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let gid = q
        .get("guild_id")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let rows = st.ctx.db.lb_get_top(gid, 20).await;
    Json(json!({ "leaderboard": rows }))
}

async fn ah_pub_status(State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    let summary = st.ctx.db.stats_summary().await;
    let live = st.ctx.live.read().await.clone();
    Json(json!({
        "online": live.online,
        "status": if live.online { "online" } else { "offline" },
        "latency_ms": live.latency_ms,
        "guild_count": live.guild_count,
        "user_count": live.user_count,
        "runtime": "Rust (Tokio + Axum)",
        "stats": summary
    }))
}

async fn ah_pub_wordbank(
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let q_str = q.get("q").map(|s| s.as_str()).unwrap_or("");
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200);
    let offset = q
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let (data, total) = st.ctx.db.pub_wordbank(q_str, limit, offset).await;
    Json(json!({ "data": data, "total": total }))
}

async fn ah_pub_rejected(
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let q_str = q.get("q").map(|s| s.as_str()).unwrap_or("");
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200);
    let offset = q
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let (data, total) = st.ctx.db.pub_rejected(q_str, limit, offset).await;
    Json(json!({ "data": data, "total": total }))
}

async fn ah_pub_servers(State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    let live = st.ctx.live.read().await;
    Json(json!({ "data": live.guilds, "total": live.guild_count }))
}

async fn ah_pub_milestones(State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    let summary = st.ctx.db.stats_summary().await;
    let live = st.ctx.live.read().await.clone();
    let gc = live.guild_count as i64;
    let uc = if live.user_count > 0 {
        live.user_count as i64
    } else {
        summary["total_players"].as_i64().unwrap_or(0)
    };
    let wb = summary["word_bank"].as_i64().unwrap_or(0);
    let gm = summary["games_played"].as_i64().unwrap_or(0);
    let cur = json!({ "servers": gc, "wordbank": wb, "games": gm, "users": uc });
    let defs = [
        ("s10", "10 server", "🏠", "servers", 10, gc),
        ("s50", "50 server", "🏘️", "servers", 50, gc),
        ("s100", "100 server", "🌆", "servers", 100, gc),
        ("w100", "100 từ duyệt", "📝", "wordbank", 100, wb),
        ("w1k", "1.000 từ", "📚", "wordbank", 1000, wb),
        ("w10k", "10.000 từ", "🏆", "wordbank", 10000, wb),
        ("g1k", "1.000 ván chơi", "🎮", "games", 1000, gm),
        ("u100", "100 người dùng", "👥", "users", 100, uc),
        ("u1k", "1.000 người dùng", "🌟", "users", 1000, uc),
    ];
    let data: Vec<serde_json::Value> = defs
        .into_iter()
        .map(|(k, l, e, tp, target, val)| {
            let pct = ((val as f64 / target as f64) * 100.0).min(100.0);
            json!({
                "key": k, "label": l, "emoji": e, "type": tp, "target": target,
                "current": val, "reached": val >= target, "pct": (pct * 10.0).round() / 10.0
            })
        })
        .collect();
    Json(json!({ "data": data, "current": cur }))
}

async fn enrich_discord_users(
    st: &ApiState,
    mut rows: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let token = &st.ctx.config.discord_token;
    if token.is_empty() {
        return rows;
    }
    for item in &mut rows {
        let Some(uid_str) = item["user_id"].as_str().map(|s| s.to_string()) else {
            continue;
        };
        let Ok(uid) = uid_str.parse::<i64>() else {
            continue;
        };
        if uid <= 0 {
            continue;
        }
        let cache_key = format!("dc_user:{}", uid);
        if let Some(cached) = st.ctx.cache.get(&cache_key).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cached) {
                if let Some(n) = v["name"].as_str() {
                    item["name"] = json!(n);
                }
                item["avatar"] = v["avatar"].clone();
                continue;
            }
        }
        let url = format!("https://discord.com/api/v10/users/{}", uid);
        if let Ok(res) = st
            .ctx
            .http
            .get(&url)
            .header("Authorization", format!("Bot {}", token))
            .send()
            .await
        {
            if res.status().is_success() {
                if let Ok(u) = res.json::<serde_json::Value>().await {
                    let name = u["global_name"]
                        .as_str()
                        .or_else(|| u["username"].as_str())
                        .unwrap_or("Discord User")
                        .to_string();
                    let avatar = u["avatar"].as_str().map(|av| {
                        let ext = if av.starts_with("a_") { "gif" } else { "png" };
                        format!("https://cdn.discordapp.com/avatars/{}/{}.{}", uid, av, ext)
                    });
                    item["name"] = json!(name);
                    item["avatar"] = json!(avatar);
                    let _ = st
                        .ctx
                        .cache
                        .set(
                            &cache_key,
                            &json!({ "name": name, "avatar": avatar }).to_string(),
                            3600,
                        )
                        .await;
                }
            }
        }
    }
    rows
}

async fn ah_pub_leaderboard(
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .min(100);
    let mode = q.get("mode").map(|s| s.as_str()).unwrap_or("words");
    let raw = st.ctx.db.pub_leaderboard(mode, limit).await;
    let data = enrich_discord_users(&st, raw).await;
    Json(json!({ "data": data }))
}

async fn ah_pub_games() -> impl IntoResponse {
    let list = json!([
        {"id": "2048", "name": "2048 Master", "icon": "🔢", "type": "puzzle", "unit": "điểm"},
        {"id": "tictactoe", "name": "Cờ Caro XO", "icon": "❌", "type": "strategy", "unit": "chuỗi thắng"},
        {"id": "minesweeper", "name": "Dò Mìn (Minesweeper)", "icon": "💣", "type": "logic", "unit": "điểm"},
        {"id": "afk", "name": "Treo Máy (AFK Time)", "icon": "⏳", "type": "idle", "unit": "giây"},
        {"id": "quiz", "name": "Đấu Trí Tri Thức", "icon": "🧠", "type": "trivia", "unit": "điểm"},
        {"id": "vuatiengviet", "name": "Vua Tiếng Việt", "icon": "👑", "type": "word", "unit": "điểm"},
        {"id": "wordle", "name": "Wordle 5 Chữ", "icon": "🟩", "type": "word", "unit": "điểm"},
        {"id": "duoihinh", "name": "Đuổi Hình Bắt Chữ", "icon": "🎨", "type": "puzzle", "unit": "điểm"},
        {"id": "typing", "name": "Đua Tốc Độ Gõ Phím", "icon": "⌨️", "type": "speed", "unit": "WPM"},
        {"id": "trieuphu", "name": "Ai Là Triệu Phú", "icon": "💎", "type": "trivia", "unit": "VNĐ"},
        {"id": "cadao", "name": "Điền Ca Dao - Tục Ngữ", "icon": "📜", "type": "word", "unit": "điểm"},
        {"id": "chinhta", "name": "Soi Lỗi Chính Tả", "icon": "✍️", "type": "word", "unit": "điểm"},
        {"id": "flagquiz", "name": "Đoán Quốc Kỳ & Quốc Gia", "icon": "🚩", "type": "trivia", "unit": "điểm"},
        {"id": "history", "name": "Đoán Năm Lịch Sử", "icon": "⏳", "type": "trivia", "unit": "điểm"},
        {"id": "whoami", "name": "Đoán Danh Nhân (Who Am I)", "icon": "🕵️", "type": "trivia", "unit": "điểm"},
        {"id": "mastermind", "name": "Giải Mật Mã (Mastermind)", "icon": "🔐", "type": "logic", "unit": "điểm"},
        {"id": "baucua", "name": "Bầu Cua Tôm Cá", "icon": "🎲", "type": "arcade", "unit": "điểm"},
        {"id": "higherlower", "name": "So Kèo Lượt Xem Wiki", "icon": "📈", "type": "trivia", "unit": "điểm"},
        {"id": "wikiquiz", "name": "Đoán Định Nghĩa Wiki", "icon": "📖", "type": "word", "unit": "điểm"},
        {"id": "anagram", "name": "Đổi Chữ Lấy Điểm (Anagram)", "icon": "🔠", "type": "word", "unit": "điểm"},
        {"id": "connect4", "name": "Nối 4 Điểm (Connect 4)", "icon": "🔴", "type": "strategy", "unit": "điểm"},
        {"id": "memory", "name": "Lật Thẻ Trí Nhớ (Memory)", "icon": "🃏", "type": "puzzle", "unit": "điểm"},
        {"id": "tamsao", "name": "Tam Sao Thất Bản", "icon": "🌀", "type": "word", "unit": "điểm"},
        {"id": "taixiu", "name": "Tài Xỉu", "icon": "🎰", "type": "arcade", "unit": "điểm"},
        {"id": "catword", "name": "Nối Từ Theo Chủ Đề", "icon": "🔗", "type": "word", "unit": "điểm"},
        {"id": "battleship", "name": "Thủy Chiến (Battleship)", "icon": "⚓", "type": "strategy", "unit": "điểm"},
        {"id": "uno", "name": "UNO Online", "icon": "🎴", "type": "arcade", "unit": "điểm"},
        {"id": "sudoku", "name": "Sudoku Speedrun 4x4", "icon": "🔢", "type": "logic", "unit": "điểm"}
    ]);
    Json(json!({ "data": list, "total": 28 }))
}

#[derive(Deserialize)]
struct ScoreBody {
    game: String,
    user_id: i64,
    score: i64,
    session_token: Option<String>,
}

async fn ah_pub_game_score(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<ScoreBody>,
) -> impl IntoResponse {
    if !pub_games::sanity_check_score(&body.game, body.score) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Điểm không hợp lệ"})),
        )
            .into_response();
    }
    if let Some(ref tok) = body.session_token {
        if !tok.is_empty() {
            if !zenne_core::auth::verify_game_session(
                &st.ctx.config.game_session_secret,
                tok,
                body.user_id,
                &body.game,
            ) {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Session token không hợp lệ hoặc đã hết hạn"})),
                )
                    .into_response();
            }
            if st.used_nonces.contains_key(tok) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Session token đã được sử dụng"})),
                )
                    .into_response();
            }
            st.used_nonces
                .insert(tok.clone(), chrono::Utc::now().timestamp());
        }
    }
    st.ctx
        .db
        .game_submit_score(&body.game, body.user_id, body.score)
        .await;
    Json(json!({ "ok": true })).into_response()
}

async fn ah_pub_game_leaderboard(
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let game = q.get("game").map(|s| s.as_str()).unwrap_or("noitu");
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .min(100);
    let raw = st.ctx.db.pub_game_leaderboard(game, limit).await;
    let data = enrich_discord_users(&st, raw).await;
    Json(json!({ "game": game, "data": data }))
}

async fn ah_pub_mp_rooms(State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    let rooms = st.ctx.db.get_public_rooms().await;
    Json(json!({ "ok": true, "rooms": rooms, "data": rooms }))
}

async fn ah_auth_login(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::LoginBody>,
) -> impl IntoResponse {
    auth_routes::handle_login(&st, body).await
}

#[derive(Deserialize)]
struct AiAskBody {
    prompt: String,
}

async fn ah_pub_ai_ask(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<AiAskBody>,
) -> impl IntoResponse {
    match zenne_core::kira::ask_ai_kira(
        &st.ctx.http,
        &st.ctx.config.kira_api_endpoint,
        &st.ctx.config.kira_api_key,
        &st.ctx.config.kira_default_model,
        &body.prompt,
        None,
    )
    .await
    {
        Ok(reply) => Json(json!({ "reply": reply })).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e }))).into_response(),
    }
}

#[derive(Deserialize)]
struct SessionStartBody {
    game: String,
    user_id: Option<i64>,
}

async fn ah_pub_game_session_start(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<SessionStartBody>,
) -> impl IntoResponse {
    let uid = body.user_id.unwrap_or(0);
    let token =
        zenne_core::auth::sign_game_session(&st.ctx.config.game_session_secret, uid, &body.game);
    Json(json!({ "token": token }))
}

async fn ah_ws_game_room(
    AxumPath(room_id): AxumPath<String>,
    ws: WebSocketUpgrade,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let ctx = st.ctx.clone();
    let rooms = st.ws_rooms.clone();
    ws.on_upgrade(move |socket| ws_game::handle_ws_connection(socket, room_id, ctx, rooms))
}

async fn ah_ws_game(ws: WebSocketUpgrade, State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    let ctx = st.ctx.clone();
    let rooms = st.ws_rooms.clone();
    ws.on_upgrade(move |socket| {
        ws_game::handle_ws_connection(socket, "LOBBY".to_string(), ctx, rooms)
    })
}

#[derive(Deserialize)]
struct BulkReviewBody {
    ids: Vec<i64>,
    approve: bool,
    reviewer_id: Option<i64>,
}

async fn ah_review_bulk(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
    Json(body): Json<BulkReviewBody>,
) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    let mut done = 0;
    for id in body.ids {
        if st
            .ctx
            .db
            .review_word(id, body.approve, body.reviewer_id.unwrap_or(0))
            .await
            .is_some()
        {
            done += 1;
        }
    }
    Json(json!({ "ok": true, "processed": done })).into_response()
}

async fn ah_wordbank_get(
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    let q_str = q.get("q").map(|s| s.as_str()).unwrap_or("");
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200);
    let offset = q
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let (data, total) = st.ctx.db.pub_wordbank(q_str, limit, offset).await;
    Json(json!({ "data": data, "total": total })).into_response()
}

#[derive(Deserialize)]
struct WordDelQuery {
    word: Option<String>,
}

async fn ah_wordbank_del(
    headers: HeaderMap,
    Query(q): Query<WordDelQuery>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    if let Some(w) = q.word {
        if let Ok(conn) = st.ctx.db.conn() {
            let _ = conn
                .execute("DELETE FROM word_bank WHERE word = ?1", [w.as_str()])
                .await;
        }
    }
    Json(json!({ "ok": true })).into_response()
}

async fn ah_guilds(headers: HeaderMap, State(st): State<Arc<ApiState>>) -> impl IntoResponse {
    if !check_admin_auth(&headers, &st.ctx.config.api_key) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    }
    let live = st.ctx.live.read().await;
    Json(json!({ "guilds": live.guilds })).into_response()
}

async fn ah_pub_game_ttt(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    match pub_games::handle_ttt(&st.ctx, &st.ttt_sessions, &body).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err((code, msg)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST),
            Json(json!({ "error": msg })),
        )
            .into_response(),
    }
}

async fn ah_pub_game_ms(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    match pub_games::handle_ms(&st.ctx, &st.ms_sessions, &body).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err((code, msg)) => (
            StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST),
            Json(json!({ "error": msg })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateRoomBody {
    name: Option<String>,
    platform: Option<String>,
    host_id: Option<String>,
    host_name: Option<String>,
    lang: Option<String>,
    max_players: Option<i64>,
}

async fn ah_pub_create_room(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<CreateRoomBody>,
) -> impl IntoResponse {
    let rid = uuid::Uuid::new_v4().to_string()[..8].to_uppercase();
    let tok = uuid::Uuid::new_v4().to_string();
    let room = zenne_core::db::PublicRoomRow {
        room_id: rid.clone(),
        name: body.name.unwrap_or_else(|| format!("Phòng {}", rid)),
        platform: body.platform.unwrap_or_else(|| "web".to_string()),
        host_id: body.host_id.unwrap_or_else(|| "0".to_string()),
        host_name: body.host_name.unwrap_or_else(|| "Web Host".to_string()),
        lang: body.lang.unwrap_or_else(|| "vi".to_string()),
        max_players: body.max_players.unwrap_or(0),
        current_players: 1,
        join_token: Some(tok.clone()),
        web_url: Some(format!("{}/room/{}", st.ctx.config.web_base_url, rid)),
        tele_url: None,
        status: "waiting".to_string(),
    };
    st.ctx.db.create_public_room(&room).await;
    Json(json!({ "ok": true, "room": room }))
}

async fn ah_pub_mp_create(
    st: State<Arc<ApiState>>,
    body: Json<CreateRoomBody>,
) -> impl IntoResponse {
    ah_pub_create_room(st, body).await
}

async fn ah_pub_mp_token_info(
    AxumPath(token): AxumPath<String>,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let rooms = st.ctx.db.get_public_rooms().await;
    if let Some(r) = rooms
        .into_iter()
        .find(|r| r.join_token.as_deref() == Some(token.as_str()) || r.room_id == token)
    {
        Json(json!({ "ok": true, "room": r })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Không tìm thấy phòng!" })),
        )
            .into_response()
    }
}

fn extract_bearer_uid(headers: &HeaderMap, secret: &str) -> Option<(i64, String)> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let tok = auth.strip_prefix("Bearer ")?;
    let val = zenne_core::auth::verify_web_token(secret, tok)?;
    let uid = val["uid"].as_i64()?;
    let uname = val["username"].as_str().unwrap_or("user").to_string();
    Some((uid, uname))
}

async fn ah_auth_register(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::RegisterBody>,
) -> impl IntoResponse {
    auth_routes::handle_register(&st, body).await
}

async fn ah_register_email_verify(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::EmailVerifyBody>,
) -> impl IntoResponse {
    auth_routes::handle_register_email_verify(&st, body).await
}

async fn ah_register_email_resend(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::EmailResendBody>,
) -> impl IntoResponse {
    auth_routes::handle_register_email_resend(&st, body).await
}

#[derive(Deserialize)]
struct Verify2FaBody {
    username: String,
    code: String,
}

async fn ah_auth_verify_2fa(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<Verify2FaBody>,
) -> impl IntoResponse {
    let Some(u) = st
        .ctx
        .db
        .find_user_by_username_or_email(&body.username)
        .await
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Không tìm thấy user"})),
        )
            .into_response();
    };
    let ok = u
        .totp_secret
        .as_deref()
        .map(|s| zenne_core::auth::verify_totp_code(s, &body.code))
        .unwrap_or(false);
    if !ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Mã 2FA không đúng"})),
        )
            .into_response();
    }
    let token = zenne_core::auth::sign_web_token(
        &st.ctx.config.web_token_secret,
        u.id,
        &u.username,
        86400 * 7,
    );
    Json(json!({ "ok": true, "token": token })).into_response()
}

#[derive(Deserialize)]
struct OAuthBody {
    username: Option<String>,
    email: Option<String>,
    provider: Option<String>,
    provider_id: Option<String>,
}

async fn ah_auth_oauth(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<OAuthBody>,
) -> impl IntoResponse {
    let uname = body.username.unwrap_or_else(|| "oauth_user".to_string());
    let email = body
        .email
        .unwrap_or_else(|| format!("{}@oauth.local", uname));
    if let Ok(conn) = st.ctx.db.conn() {
        let _ = conn
            .execute(
                "INSERT OR IGNORE INTO web_users (username, email, provider, provider_id, email_verified) VALUES (?1, ?2, ?3, ?4, 1)",
                (
                    uname.as_str(),
                    email.as_str(),
                    body.provider.as_deref().unwrap_or("discord"),
                    body.provider_id.as_deref().unwrap_or(""),
                ),
            )
            .await;
    }
    if let Some(u) = st.ctx.db.find_user_by_username_or_email(&email).await {
        let token = zenne_core::auth::sign_web_token(
            &st.ctx.config.web_token_secret,
            u.id,
            &u.username,
            86400 * 7,
        );
        return Json(json!({ "ok": true, "token": token, "user": { "id": u.id, "username": u.username, "email": u.email } })).into_response();
    }
    Json(json!({ "ok": true })).into_response()
}

async fn ah_pub_game_token(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let (uid, uname) = extract_bearer_uid(&headers, &st.ctx.config.web_token_secret)
        .unwrap_or((0, "guest".to_string()));
    let tok = zenne_core::auth::sign_web_token(&st.ctx.config.web_token_secret, uid, &uname, 3600);
    Json(json!({ "token": tok, "user_id": uid }))
}

async fn ah_pub_account_info(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let Some((uid, _)) = extract_bearer_uid(&headers, &st.ctx.config.web_token_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    };
    if let Ok(conn) = st.ctx.db.conn() {
        if let Ok(mut rows) = conn
            .query(
                "SELECT id, username, email, is_admin, totp_enabled FROM web_users WHERE id = ?1",
                [uid],
            )
            .await
        {
            if let Ok(Some(r)) = rows.next().await {
                return Json(json!({
                    "id": r.get::<i64>(0).unwrap_or(0),
                    "username": r.get::<String>(1).unwrap_or_default(),
                    "email": r.get::<String>(2).unwrap_or_default(),
                    "is_admin": r.get::<i64>(3).unwrap_or(0),
                    "totp_enabled": r.get::<i64>(4).unwrap_or(0),
                }))
                .into_response();
            }
        }
    }
    (StatusCode::NOT_FOUND, Json(json!({"error": "Not found"}))).into_response()
}

async fn ah_pub_account_change_password(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::ChangePwBody>,
) -> impl IntoResponse {
    let Some((uid, _)) = extract_bearer_uid(&headers, &st.ctx.config.web_token_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    };
    auth_routes::handle_change_password(&headers, &st, body, uid).await
}

async fn ah_pub_account_setup_2fa(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let Some((uid, uname)) = extract_bearer_uid(&headers, &st.ctx.config.web_token_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    };
    let secret = zenne_core::auth::gen_totp_secret();
    if let Ok(conn) = st.ctx.db.conn() {
        let _ = conn
            .execute(
                "UPDATE web_users SET totp_secret = ?1 WHERE id = ?2",
                (secret.as_str(), uid),
            )
            .await;
    }
    let uri = zenne_core::auth::totp_uri(&secret, &uname);
    Json(json!({ "secret": secret, "uri": uri })).into_response()
}

async fn ah_pub_account_enable_2fa(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
    body: Option<Json<auth_routes::TotpCodeBody>>,
) -> impl IntoResponse {
    let Some((uid, _)) = extract_bearer_uid(&headers, &st.ctx.config.web_token_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    };
    auth_routes::handle_toggle_2fa(&st, uid, true, body.map(|b| b.0)).await
}

async fn ah_pub_account_disable_2fa(
    headers: HeaderMap,
    State(st): State<Arc<ApiState>>,
    body: Option<Json<auth_routes::TotpCodeBody>>,
) -> impl IntoResponse {
    let Some((uid, _)) = extract_bearer_uid(&headers, &st.ctx.config.web_token_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unauthorized"})),
        )
            .into_response();
    };
    auth_routes::handle_toggle_2fa(&st, uid, false, body.map(|b| b.0)).await
}

async fn ah_recovery_request(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::RecoveryRequestBody>,
) -> impl IntoResponse {
    auth_routes::handle_recovery_request(&st, body).await
}
async fn ah_recovery_email_verify(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::RecoveryVerifyBody>,
) -> impl IntoResponse {
    auth_routes::handle_recovery_verify(&st, body).await
}
async fn ah_recovery_token_verify(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::RecoveryVerifyBody>,
) -> impl IntoResponse {
    auth_routes::handle_recovery_verify(&st, body).await
}
async fn ah_recovery_status() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}
async fn ah_recovery_reset_password(
    State(st): State<Arc<ApiState>>,
    Json(body): Json<auth_routes::RecoveryResetBody>,
) -> impl IntoResponse {
    auth_routes::handle_recovery_reset_password(&st, body).await
}
