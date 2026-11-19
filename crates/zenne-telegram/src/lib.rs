use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tracing::info;
use zenne_core::AppContext;
use zenne_games::{baucua, taixiu, GameHub};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "Các lệnh Zenne Bot Telegram:"
)]
enum Command {
    #[command(description = "Hiển thị hướng dẫn")]
    Start,
    #[command(description = "Hiển thị trợ giúp")]
    Help,
    #[command(description = "Tung xúc xắc Tài Xỉu")]
    Taixiu,
    #[command(description = "Chơi Bầu Cua")]
    Baucua,
    #[command(description = "Chơi Vua Tiếng Việt")]
    Vtv,
    #[command(description = "Trả lời câu hỏi Quiz")]
    Quiz,
    #[command(description = "Hỏi AI Kira (/ask câu hỏi)")]
    Ask(String),
    #[command(description = "Tra từ điển Wikipedia (/tratu từ)")]
    Tratu(String),
}

pub async fn run_telegram_bot(ctx: AppContext, games: GameHub) {
    let token = ctx.config.telegram_bot_token.trim().to_string();
    if token.is_empty() || token == "YOUR_TELEGRAM_BOT_TOKEN_HERE" {
        info!("[Telegram] Chua cau hinh TELEGRAM_BOT_TOKEN (Bo qua).");
        return;
    }

    info!("[Telegram] Dang khoi dong Telegram Bot (Rust Teloxide)...");
    let bot = Bot::new(token);

    let handler =
        Update::filter_message().branch(dptree::entry().filter_command::<Command>().endpoint(
            move |bot: Bot, msg: Message, cmd: Command| {
                let ctx = ctx.clone();
                let games = games.clone();
                async move {
                    let chat_id = msg.chat.id;
                    let ch_u64 = chat_id.0.unsigned_abs();
                    let reply = match cmd {
                        Command::Start | Command::Help => {
                            "🤖 *Zenne Nối Từ & Minigames (Rust)*\n\n\
                            /taixiu - Tung Tài Xỉu\n\
                            /baucua - Lắc Bầu Cua\n\
                            /vtv - Vua Tiếng Việt\n\
                            /quiz - Đố vui\n\
                            /tratu <từ> - Tra từ điển Wiki\n\
                            /ask <câu hỏi> - Hỏi trợ lý AI Kira"
                                .to_string()
                        }
                        Command::Taixiu => {
                            let r = taixiu::roll_taixiu();
                            let kq = if r.is_triple {
                                "BỘ BA"
                            } else if r.is_tai {
                                "TÀI"
                            } else {
                                "XỈU"
                            };
                            format!(
                                "🎲 Xúc xắc: {} - {} - {} = {} ({})",
                                r.dice[0], r.dice[1], r.dice[2], r.total, kq
                            )
                        }
                        Command::Baucua => {
                            let r = baucua::roll_baucua();
                            format!(
                                "🦀 Kết quả: {} {} | {} {} | {} {}",
                                r[0].display().0,
                                r[0].display().1,
                                r[1].display().0,
                                r[1].display().1,
                                r[2].display().0,
                                r[2].display().1
                            )
                        }
                        Command::Vtv => {
                            let s = games.vuatiengviet.start(ch_u64);
                            format!("👑 Vua Tiếng Việt: Hãy ghép chữ -> {}", s.scrambled)
                        }
                        Command::Quiz => {
                            let q = games.quiz.start_general(ch_u64);
                            format!(
                                "❓ [{}] {}\nA. {}\nB. {}\nC. {}\nD. {}",
                                q.category,
                                q.question,
                                q.options[0],
                                q.options[1],
                                q.options[2],
                                q.options[3]
                            )
                        }
                        Command::Tratu(word) => {
                            if word.trim().is_empty() {
                                "Vui lòng nhập từ cần tra! Ví dụ: /tratu hòa bình".to_string()
                            } else {
                                let (ok, extract) = zenne_core::wiki::check_word(
                                    &ctx.http, &ctx.db, &ctx.cache, 0, "vi", &word,
                                )
                                .await;
                                if ok {
                                    format!(
                                        "✅ Từ '{}' hợp lệ!\n{}",
                                        word.trim(),
                                        extract.unwrap_or_default()
                                    )
                                } else {
                                    format!(
                                        "❌ Không tìm thấy '{}' trên Wikipedia/Wiktionary.",
                                        word.trim()
                                    )
                                }
                            }
                        }
                        Command::Ask(prompt) => {
                            if prompt.trim().is_empty() {
                                "Vui lòng nhập câu hỏi sau /ask!".to_string()
                            } else {
                                zenne_core::kira::ask_ai_kira(
                                    &ctx.http,
                                    &ctx.config.kira_api_endpoint,
                                    &ctx.config.kira_api_key,
                                    &ctx.config.kira_default_model,
                                    &prompt,
                                    None,
                                )
                                .await
                                .unwrap_or_else(|e| format!("Lỗi AI: {}", e))
                            }
                        }
                    };
                    let _ = bot.send_message(chat_id, reply).await;
                    respond(())
                }
            },
        ));

    Dispatcher::builder(bot, handler).build().dispatch().await;
}
