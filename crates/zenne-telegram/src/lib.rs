mod minigames;
mod noitu;
mod state;

use state::{user_display_name, TelegramState};
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;
use teloxide::types::{BotCommand, ParseMode};
use tracing::{info, warn};
use zenne_core::AppContext;
use zenne_games::GameHub;

fn build_bot_commands() -> Vec<BotCommand> {
    vec![
        BotCommand::new(
            "multiplayer",
            "Sảnh chơi nhiều người công khai (Web, Discord, Telegram)",
        ),
        BotCommand::new("noitu", "Mở phòng chờ nối từ & lấy link mời"),
        BotCommand::new(
            "taophong",
            "Tạo nhanh phòng công khai: /taophong <Tên>|<Lang>|<Max>",
        ),
        BotCommand::new("solo", "Chơi nối từ 1-1 ngay với Bot"),
        BotCommand::new("batdau", "Bắt đầu ván đấu nối từ"),
        BotCommand::new("stop", "Dừng ván đấu hiện tại"),
        BotCommand::new("diem", "Xem điểm ván đấu nối từ"),
        BotCommand::new("top", "Xem bảng xếp hạng nối từ"),
        BotCommand::new("tratu", "Tra cứu từ trên từ điển"),
        BotCommand::new("contukhong", "Kiểm tra âm tiết có từ nối không"),
        BotCommand::new("gopthemtu", "Góp từ mới vào ngân hàng từ"),
        BotCommand::new("setlang", "Cài đặt ngôn ngữ từ điển"),
        BotCommand::new("quiz", "Đấu trí trắc nghiệm Toán & Kiến thức"),
        BotCommand::new("vtv", "Mini-game Vua Tiếng Việt"),
        BotCommand::new("taixiu", "Lắc Xúc Xắc Tài Xỉu"),
        BotCommand::new("baucua", "Bầu Cua Tôm Cá dân gian"),
        BotCommand::new("cadao", "Đoán Ca Dao Tục Ngữ Việt Nam"),
        BotCommand::new("chinhta", "Thử thách bắt lỗi chính tả"),
        BotCommand::new("flag", "Đoán Quốc kỳ thế giới"),
        BotCommand::new("trieuphu", "Gameshow Ai Là Triệu Phú"),
        BotCommand::new("wordle", "Đoán từ 5 chữ cái Wordle"),
        BotCommand::new("ask", "Hỏi đáp với Trợ lý AI Kira"),
        BotCommand::new("mygame", "Xem kỷ lục cá nhân"),
        BotCommand::new("help", "Xem hướng dẫn toàn diện"),
    ]
}

fn parse_slash_command(raw: &str) -> Option<(String, &str)> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let mut split = trimmed.splitn(2, char::is_whitespace);
    let head = split.next()?.trim_start_matches('/');
    let cmd = head.split('@').next()?.to_lowercase();
    if cmd.is_empty() {
        return None;
    }
    let args = split.next().unwrap_or("").trim();
    Some((cmd, args))
}

async fn handle_message(bot: Bot, msg: Message, st: TelegramState) -> ResponseResult<()> {
    let Some(raw_text) = msg.text() else {
        return Ok(());
    };

    if let Some((cmd, args)) = parse_slash_command(raw_text) {
        let chat_id = msg.chat.id;
        let (uid, uname) = user_display_name(msg.from.as_ref());
        match cmd.as_str() {
            "start" => noitu::cmd_start(&bot, &msg, &st, args).await,
            "help" => noitu::cmd_help(&bot, chat_id).await,
            "multiplayer" | "mp" => noitu::cmd_multiplayer(&bot, chat_id).await,
            "taophong" => noitu::cmd_taophong(&bot, &msg, &st, args).await,
            "noitu" | "start_noitu" => noitu::cmd_noitu(&bot, chat_id, uid, uname, &st).await,
            "solo" | "botplay" => noitu::cmd_solo(&bot, chat_id, uid, uname, &st).await,
            "batdau" | "play" | "bdt" => noitu::cmd_play(&bot, &msg, &st).await,
            "stop" | "end" | "dung" => noitu::cmd_stop(&bot, &msg, &st).await,
            "diem" | "score" | "scores" => noitu::cmd_diem(&bot, &msg, &st).await,
            "top" | "bxh" | "leaderboard" => noitu::cmd_top(&bot, chat_id, &st).await,
            "tratu" | "check" | "tra" => noitu::cmd_tratu(&bot, &msg, &st, args).await,
            "contukhong" | "ctk" => noitu::cmd_contukhong(&bot, &msg, &st, args).await,
            "gopthemtu" | "addword" | "gtt" => noitu::cmd_gopthemtu(&bot, &msg, &st, args).await,
            "setlang" | "lang" => noitu::cmd_setlang(&bot, &msg, &st, args).await,
            "quiz" | "quizz" | "toan" | "khoahoc" => {
                let effective_args =
                    if matches!(cmd.as_str(), "toan" | "khoahoc") && args.is_empty() {
                        cmd.as_str()
                    } else {
                        args
                    };
                minigames::cmd_quiz(&bot, &msg, &st, effective_args).await;
            }
            "vtv" | "vuatiengviet" => minigames::cmd_vtv(&bot, &msg, &st).await,
            "taixiu" | "tx" => minigames::cmd_taixiu(&bot, &msg, &st).await,
            "baucua" | "bc" => minigames::cmd_baucua(&bot, &msg, &st).await,
            "cadao" | "tucngu" => minigames::cmd_cadao(&bot, &msg, &st).await,
            "chinhta" | "soiloichinhta" => minigames::cmd_chinhta(&bot, &msg, &st).await,
            "flag" | "flagquiz" | "quocky" => minigames::cmd_flag(&bot, &msg, &st).await,
            "trieuphu" | "altp" => minigames::cmd_trieuphu(&bot, &msg, &st).await,
            "wordle" | "doantu" => minigames::cmd_wordle(&bot, &msg, &st).await,
            "ask" | "ai" | "kira" => noitu::cmd_ask(&bot, &msg, &st, args).await,
            "mygame" | "profile" => minigames::cmd_mygame(&bot, &msg, &st).await,
            _ => {
                let _ = bot
                    .send_message(
                        chat_id,
                        "❓ Lệnh không hợp lệ. Gõ <code>/help</code> để xem toàn bộ danh sách lệnh!",
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
            }
        }
        return Ok(());
    }

    noitu::handle_text_message(&bot, &msg, &st, raw_text).await;
    Ok(())
}

pub async fn run_telegram_bot(ctx: AppContext, games: GameHub) {
    let token = ctx.config.telegram_bot_token.trim().to_string();
    if token.is_empty() || token == "YOUR_TELEGRAM_BOT_TOKEN_HERE" {
        info!("[Telegram] Chua cau hinh TELEGRAM_BOT_TOKEN (Bo qua).");
        return;
    }

    info!("[Telegram] Dang khoi dong Telegram Bot (Rust Teloxide)...");
    let bot = Bot::new(token);

    if let Err(e) = bot.set_my_commands(build_bot_commands()).await {
        warn!("[Telegram] Khong the set_my_commands: {}", e);
    } else {
        info!("[Telegram] Da dang ky 24 BotCommand vao Telegram Menu!");
    }

    let state = TelegramState::new(ctx, games);

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(noitu::handle_callback_query));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .build()
        .dispatch()
        .await;
}
