mod extra_cmds;
mod music;
mod noitu;
mod workers;

use noitu::NoituManager;
use poise::serenity_prelude as serenity;
use songbird::SerenityInit;
use tracing::{error, info};
use zenne_core::{i18n, wiki, AppContext};
use zenne_games::{baucua, taixiu, GameHub};
use zenne_music::multiplay::MultiPlayManager;
use zenne_music::MusicManager;

pub struct Data {
    pub app: AppContext,
    pub games: GameHub,
    pub music: MusicManager,
    pub multiplay: MultiPlayManager,
    pub noitu: NoituManager,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// Hiển thị bảng trợ giúp tất cả các lệnh Zenne Bot
#[poise::command(
    slash_command,
    prefix_command,
    aliases("noitu_help", "help_noitu", "helpnoitu", "huongdan")
)]
async fn help(ctx: Context<'_>) -> Result<(), Error> {
    let embed = serenity::CreateEmbed::new()
        .title("🤖 Zenne Bot Rust Edition — Bảng Lệnh")
        .color(0x5865F2)
        .field(
            "🔗 Nối Từ (300+ Ngôn Ngữ)",
            "`!start` / `/start`, `!batdautro`, `!stop`, `!setlang`, `!setuilang`, `!listlang`, `!tratu`, `!top`, `!diem`, `!gopthemtu`",
            false,
        )
        .field(
            "🎵 Nhạc SoundCloud (Độc quyền)",
            "`!play` / `/play`, `!pause`, `!resume`, `!skip`, `!mstop`, `!queue`, `!volume`, `!rloop`",
            false,
        )
        .field(
            "🎮 Minigames & AI",
            "`!taixiu`, `!baucua`, `!vtv`, `!quiz`, `!quizmath`, `!wordle`, `!anagram`, `!cadao`, `!chinhta`, `!emoji`, `!flag`, `!history`, `!whoami`, `!mastermind`, `!battleship`, `!memory`, `!typing`, `!afk`, `!ask`",
            false,
        );
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Strict authorization for `/resetpassword_admin`:
/// ONLY explicitly configured bot owners / designated recovery administrators (`super_admin_ids`).
/// Ordinary Discord Guild Administrator permission MUST NOT grant access.
pub(crate) fn can_invoke_admin_recovery(uid: u64, super_admins: &[u64]) -> bool {
    super_admins.contains(&uid)
}

pub(crate) async fn is_bot_admin(ctx: Context<'_>) -> bool {
    let uid = ctx.author().id.get();
    let super_admins = &ctx.data().app.config.super_admin_ids;
    let has_guild_admin_perm = if let Some(guild_id) = ctx.guild_id() {
        if let Some(member) = ctx.author_member().await {
            if let Some(guild) = ctx.cache().guild(guild_id) {
                #[allow(deprecated)]
                let perms = guild.member_permissions(&member);
                perms.administrator()
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };
    can_invoke_admin_recovery(uid, super_admins) || has_guild_admin_perm
}

pub(crate) async fn is_guild_admin(ctx: Context<'_>) -> bool {
    let uid = ctx.author().id.get();
    if ctx.data().app.config.super_admin_ids.contains(&uid) {
        return true;
    }
    let Some(guild_id) = ctx.guild_id() else {
        return true;
    };
    if let Some(member) = ctx.author_member().await {
        if let Some(guild) = ctx.cache().guild(guild_id) {
            #[allow(deprecated)]
            let perms = guild.member_permissions(&member);
            return perms.administrator() || perms.manage_guild() || perms.manage_channels();
        }
    }
    false
}

/// Bắt đầu ván Nối Từ trong kênh hiện tại
#[poise::command(
    slash_command,
    prefix_command,
    aliases("batdautro", "bdt", "startgame", "noitu_start")
)]
async fn start(
    ctx: Context<'_>,
    #[description = "Nhập 'bot' nếu muốn chơi với Bot"] mode: Option<String>,
) -> Result<(), Error> {
    let ch_id = ctx.channel_id().get();
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    if let Some(fixed_ch) = ctx.data().app.db.get_game_channel(guild_id).await {
        if fixed_ch != ch_id as i64 {
            ctx.say(format!(
                "⚠️ Server đã cố định kênh Nối Từ tại <#{}>!",
                fixed_ch
            ))
            .await?;
            return Ok(());
        }
    }
    let lang = ctx.data().app.db.get_lang(guild_id).await;
    let first_word = wiki::random_word(&ctx.data().app.http, &ctx.data().app.db, &lang).await;
    let bot_mode = mode
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("bot"))
        .unwrap_or(false);

    let room_arc = ctx.data().noitu.get_room(ch_id);
    {
        let mut r = room_arc.lock().await;
        r.active = true;
        r.players = vec![ctx.author().id.get()];
        r.turn_idx = 0;
        r.last_word = first_word.clone();
        r.used.clear();
        r.used.insert(first_word.clone());
        r.scores.clear();
        r.round = 1;
        r.bot_mode = bot_mode;
    }

    let next_syl = wiki::last_syl(&first_word, &lang);
    ctx.say(format!(
        "🎯 **Ván Nối Từ đã bắt đầu!** (Ngôn ngữ: `{}` | Chế độ: {})\nTừ mở đầu: **{}**\n🔗 Hãy nối tiếp bằng từ bắt đầu với **`{}`**!",
        lang,
        if bot_mode { "Đấu với Bot 🤖" } else { "Nhiều người chơi 👥" },
        first_word,
        next_syl
    ))
    .await?;
    Ok(())
}

/// Dừng ván Nối Từ đang diễn ra
#[poise::command(slash_command, prefix_command, aliases("noitu_stop", "end"))]
async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let is_admin = is_guild_admin(ctx).await;
    let uid = ctx.author().id.get();
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let room_arc = ctx.data().noitu.get_room(ctx.channel_id().get());
    let (players_i64, winner) = {
        let mut r = room_arc.lock().await;
        if !r.active {
            ctx.say("❌ Hiện không có ván Nối Từ nào đang diễn ra.")
                .await?;
            return Ok(());
        }
        if !is_admin && r.players.first() != Some(&uid) {
            ctx.say("⛔ Chỉ chủ phòng hoặc Admin mới được dừng ván chơi!")
                .await?;
            return Ok(());
        }
        r.cancel_timeout();
        r.active = false;
        let p: Vec<i64> = r.players.iter().map(|&id| id as i64).collect();
        let w = r
            .scores
            .iter()
            .max_by_key(|(_, s)| *s)
            .map(|(id, _)| *id as i64);
        (p, w)
    };
    if !players_i64.is_empty() {
        ctx.data()
            .app
            .db
            .lb_add_game(guild_id, &players_i64, winner)
            .await;
    }
    ctx.say("🛑 Đã kết thúc ván Nối Từ trong kênh này!").await?;
    Ok(())
}

/// Đổi ngôn ngữ từ điển Nối Từ (vi, en, ja, ko, zh, ...)
#[poise::command(
    slash_command,
    prefix_command,
    aliases("setngonngu", "noitu_setlang", "lang", "ngonngu", "language")
)]
async fn setlang(ctx: Context<'_>, code: String) -> Result<(), Error> {
    if !is_guild_admin(ctx).await {
        ctx.say("⛔ Bạn cần quyền Quản lý Server (Manage Guild) để đổi ngôn ngữ!")
            .await?;
        return Ok(());
    }
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let c = code.trim().to_lowercase();
    ctx.data().app.db.set_lang(guild_id, &c).await;
    ctx.say(format!(
        "✅ Đã đổi ngôn ngữ Nối Từ sang **{}** (`{}`)!",
        i18n::lang_display_name(&c),
        c
    ))
    .await?;
    Ok(())
}

/// Đổi ngôn ngữ giao diện thông báo của bot
#[poise::command(slash_command, prefix_command, aliases("uilang", "ngugiaodien"))]
async fn setuilang(ctx: Context<'_>, code: String) -> Result<(), Error> {
    if !is_guild_admin(ctx).await {
        ctx.say("⛔ Bạn cần quyền Quản lý Server (Manage Guild) để đổi ngôn ngữ giao diện!")
            .await?;
        return Ok(());
    }
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let c = code.trim().to_lowercase();
    if !i18n::is_known_ui_lang(&c) {
        ctx.say(i18n::t("vi", "setuilang_invalid", &[])).await?;
        return Ok(());
    }
    ctx.data().app.db.set_ui_lang(guild_id, &c).await;
    ctx.say(i18n::t(&c, "setuilang_success", &[("lang", &c)]))
        .await?;
    Ok(())
}

/// Tra từ trên Wikipedia / Wiktionary
#[poise::command(slash_command, prefix_command, aliases("check", "kt", "tra"))]
async fn tratu(ctx: Context<'_>, #[rest] word: String) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let lang = ctx.data().app.db.get_lang(guild_id).await;
    let (ok, extract) = wiki::check_word(
        &ctx.data().app.http,
        &ctx.data().app.db,
        &ctx.data().app.cache,
        guild_id,
        &lang,
        &word,
    )
    .await;
    if ok {
        ctx.say(format!(
            "✅ Từ **{}** hợp lệ!\n{}",
            word.trim(),
            extract.unwrap_or_default()
        ))
        .await?;
    } else {
        ctx.say(format!(
            "❌ Không tìm thấy **{}** trên Wikipedia/Wiktionary `{}`.",
            word.trim(),
            lang
        ))
        .await?;
    }
    Ok(())
}

/// Xem bảng xếp hạng Nối Từ của server
#[poise::command(
    slash_command,
    prefix_command,
    aliases("lb", "top_noitu", "bxh", "xephang")
)]
async fn top(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let rows = ctx.data().app.db.lb_get_top(guild_id, 10).await;
    if rows.is_empty() {
        ctx.say("📭 Chưa có dữ liệu bảng xếp hạng trong server này!")
            .await?;
        return Ok(());
    }
    let mut desc = String::new();
    for (i, r) in rows.iter().enumerate() {
        desc.push_str(&format!(
            "**{}.** <@{}> — **{}** từ (Max streak: {})\n",
            i + 1,
            r.user_id,
            r.total_words,
            r.max_streak
        ));
    }
    let embed = serenity::CreateEmbed::new()
        .title("🏆 Bảng Xếp Hạng Nối Từ")
        .description(desc)
        .color(0xF1C40F);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Đề nghị thêm từ vào ngân hàng từ của server
#[poise::command(slash_command, prefix_command, aliases("addword", "gtt"))]
async fn gopthemtu(ctx: Context<'_>, #[rest] word: String) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let w = wiki::normalize_word(&word);
    match ctx
        .data()
        .app
        .db
        .submit_pending(guild_id, &w, ctx.author().id.get() as i64)
        .await
    {
        Ok(id) => {
            ctx.say(format!(
                "📩 Đã gửi từ **{}** vào danh sách chờ duyệt (ID: `{}`)!",
                w, id
            ))
            .await?;
        }
        Err(e) => {
            ctx.say(format!("⚠️ {}", e)).await?;
        }
    }
    Ok(())
}

/// Hỏi trợ lý AI Kira
#[poise::command(slash_command, prefix_command, aliases("hoi", "ai", "hoiai"))]
async fn ask(ctx: Context<'_>, #[rest] prompt: String) -> Result<(), Error> {
    ctx.defer().await?;
    let cfg = &ctx.data().app.config;
    match zenne_core::kira::ask_ai_kira(
        &ctx.data().app.http,
        &cfg.kira_api_endpoint,
        &cfg.kira_api_key,
        &cfg.kira_default_model,
        &prompt,
        None,
    )
    .await
    {
        Ok(ans) => {
            let truncated: String = ans.chars().take(1900).collect();
            ctx.say(truncated).await?;
        }
        Err(e) => {
            ctx.say(format!("⚠️ {}", e)).await?;
        }
    }
    Ok(())
}

/// Đặt trạng thái AFK
#[poise::command(slash_command, prefix_command)]
async fn afk(ctx: Context<'_>, #[rest] reason: Option<String>) -> Result<(), Error> {
    let gid = ctx.guild_id().map(|g| g.get()).unwrap_or(0);
    let entry =
        ctx.data()
            .games
            .afk
            .set_afk(gid, ctx.author().id.get(), reason.as_deref().unwrap_or(""));
    ctx.say(format!(
        "💤 **{}** đã bật chế độ AFK: *{}*",
        ctx.author().name,
        entry.reason
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("tx", "xucxac"))]
async fn taixiu(ctx: Context<'_>) -> Result<(), Error> {
    let r = taixiu::roll_taixiu();
    let kq = if r.is_triple {
        "BỘ BA"
    } else if r.is_tai {
        "TÀI"
    } else {
        "XỈU"
    };
    ctx.say(format!(
        "🎲 Xúc xắc: **{} - {} - {}** = **{}** (**{}**)",
        r.dice[0], r.dice[1], r.dice[2], r.total, kq
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("bc", "tomca"))]
async fn baucua(ctx: Context<'_>) -> Result<(), Error> {
    let r = baucua::roll_baucua();
    ctx.say(format!(
        "🦀 Kết quả: {} **{}** | {} **{}** | {} **{}**",
        r[0].display().0,
        r[0].display().1,
        r[1].display().0,
        r[1].display().1,
        r[2].display().0,
        r[2].display().1
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("vuatiengviet", "xapchu"))]
async fn vtv(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.vuatiengviet.check_answer(ch, &g) {
            if ok {
                ctx.say(format!("🎉 Chính xác! Đáp án là **{}**!", ans))
                    .await?;
            } else {
                ctx.say("❌ Sai rồi, thử lại nhé!").await?;
            }
            return Ok(());
        }
    }
    let s = ctx.data().games.vuatiengviet.start(ch);
    ctx.say(format!(
        "👑 **Vua Tiếng Việt**: Hãy ghép các chữ cái sau -> `{}`\n*(Trả lời bằng `!vtv <đáp án>`)*",
        s.scrambled
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("quizz", "giaido", "dautri"))]
async fn quiz(ctx: Context<'_>, answer: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(ans) = answer {
        if let Some((ok, correct)) = ctx.data().games.quiz.answer(ch, &ans) {
            if ok {
                ctx.say(format!("🎉 Chính xác! Đáp án: **{}**", correct))
                    .await?;
            } else {
                ctx.say("❌ Chưa đúng rồi!").await?;
            }
            return Ok(());
        }
    }
    let q = ctx.data().games.quiz.start_general(ch);
    ctx.say(format!(
        "❓ **[{}]** {}\nA. {}\nB. {}\nC. {}\nD. {}\n*(Trả lời bằng `!quiz <A/B/C/D>`)*",
        q.category, q.question, q.options[0], q.options[1], q.options[2], q.options[3]
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("toan"))]
async fn quizmath(ctx: Context<'_>) -> Result<(), Error> {
    let q = ctx.data().games.quiz.start_math(ctx.channel_id().get());
    ctx.say(format!(
        "🧮 **Toán Nhanh:** {}\nA. {}\nB. {}\nC. {}\nD. {}\n*(Trả lời bằng `!quiz <A/B/C/D>`)*",
        q.question, q.options[0], q.options[1], q.options[2], q.options[3]
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn wordle(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        match ctx.data().games.wordle.guess(ch, &g) {
            Ok((hist, won, over, tries, target)) => {
                if won {
                    ctx.say(format!(
                        "🎉 Tuyệt vời! Bạn đoán đúng `{}` sau **{}** lượt!\n{}",
                        target, tries, hist
                    ))
                    .await?;
                } else if over {
                    ctx.say(format!("💀 Hết lượt! Từ bí mật là `{}`.\n{}", target, hist))
                        .await?;
                } else {
                    ctx.say(format!("🟩 **Wordle** ({}/6):\n{}", tries, hist))
                        .await?;
                }
            }
            Err(e) => {
                ctx.say(format!("⚠️ {}", e)).await?;
            }
        }
        return Ok(());
    }
    ctx.data().games.wordle.start(ch);
    ctx.say("🟩 Đã tạo ván **Wordle** mới (5 chữ cái tiếng Anh)! Dùng `!wordle <từ>` để đoán.")
        .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("doichu", "ghepchu"))]
async fn anagram(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.anagram.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chuẩn xác! Đáp án là **{}**!", ans)
            } else {
                "❌ Sai rồi!".to_string()
            })
            .await?;
            return Ok(());
        }
    }
    let s = ctx.data().games.anagram.start(ch);
    ctx.say(format!(
        "🔀 **Đảo Chữ**: `{}`\n💡 Gợi ý: *{}*\n*(Đoán bằng `!anagram <từ>`)*",
        s.scrambled, s.hint
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("thuyachien", "naval"))]
async fn battleship(ctx: Context<'_>, coord: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(c) = coord {
        match ctx.data().games.battleship.shoot(ch, &c) {
            Ok((board, hit, over, left)) => {
                let status = if over {
                    "🏁 Ván đấu kết thúc!"
                } else if hit {
                    "💥 Trúng tàu!"
                } else {
                    "💦 Bắn hụt!"
                };
                ctx.say(format!("{} (Còn {} lượt)\n{}", status, left, board))
                    .await?;
            }
            Err(e) => {
                ctx.say(format!("⚠️ {}", e)).await?;
            }
        }
        return Ok(());
    }
    let board = ctx.data().games.battleship.start(ch);
    ctx.say(format!(
        "🚢 **Bắn Tàu (5x5)** — Bắn bằng `!battleship A1`:\n{}",
        board
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, aliases("latthe", "trinho"))]
async fn memory(ctx: Context<'_>, a: Option<usize>, b: Option<usize>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let (Some(x), Some(y)) = (a, b) {
        match ctx.data().games.memory.flip(ch, x, y) {
            Ok((view, matched, won, moves)) => {
                let msg = if won {
                    format!("🏆 Hoàn thành sau {} nước!", moves)
                } else if matched {
                    "✅ Trùng khớp 1 cặp!".to_string()
                } else {
                    "❌ Không khớp!".to_string()
                };
                ctx.say(format!("{}\n{}", msg, view)).await?;
            }
            Err(e) => {
                ctx.say(format!("⚠️ {}", e)).await?;
            }
        }
        return Ok(());
    }
    let view = ctx.data().games.memory.start(ch);
    ctx.say(format!(
        "🧠 **Lật Thẻ Trí Nhớ** — Lật bằng `!memory 1 5`:\n{}",
        view
    ))
    .await?;
    Ok(())
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            info!(
                "[Discord] Đã đăng nhập thành công với bot: {} (ID: {})",
                data_about_bot.user.name, data_about_bot.user.id
            );
            let mut live = data.app.live.write().await;
            live.online = true;
            live.guild_count = data_about_bot.guilds.len();
            live.latency_ms = Some(25.0);
        }
        serenity::FullEvent::CacheReady { guilds } => {
            let mut list = Vec::new();
            let mut total_users = 0u64;
            for gid in guilds {
                if let Some(g) = ctx.cache.guild(*gid) {
                    total_users += g.member_count;
                    list.push(zenne_core::GuildSummary {
                        id: gid.get().to_string(),
                        name: g.name.clone(),
                        member_count: g.member_count,
                        icon: g.icon_url(),
                    });
                }
            }
            list.sort_by_key(|a| std::cmp::Reverse(a.member_count));
            let mut live = data.app.live.write().await;
            live.online = true;
            live.guild_count = guilds.len();
            live.user_count = total_users;
            live.guilds = list.into_iter().take(50).collect();
        }
        serenity::FullEvent::Message { new_message } => {
            if new_message.author.bot {
                return Ok(());
            }
            let gid = new_message.guild_id.map(|g| g.get()).unwrap_or(0);
            if let Some(removed) = data.games.afk.remove_afk(gid, new_message.author.id.get()) {
                let dur = zenne_games::afk::AfkManager::format_duration(removed.since_ts);
                let _ = new_message
                    .reply(
                        &ctx.http,
                        format!(
                            "👋 Chào mừng **{}** đã quay trở lại! (AFK trong {})",
                            new_message.author.name, dur
                        ),
                    )
                    .await;
            }
            for mention in &new_message.mentions {
                if let Some(afk) = data.games.afk.get_afk(gid, mention.id.get()) {
                    let dur = zenne_games::afk::AfkManager::format_duration(afk.since_ts);
                    let _ = new_message
                        .reply(
                            &ctx.http,
                            format!(
                                "💤 **{}** hiện đang AFK ({}) — Lý do: *{}*",
                                mention.name, dur, afk.reason
                            ),
                        )
                        .await;
                }
            }
            let _ = noitu::handle_word_message(ctx, new_message, &data.app, &data.noitu).await;
        }
        _ => {}
    }
    Ok(())
}

pub async fn run_discord_bot(app: AppContext, games: GameHub) {
    let token = app.config.discord_token.trim().to_string();
    if token.is_empty() {
        info!("[Discord] Chua cau hinh DISCORD_TOKEN (Bo qua).");
        return;
    }

    let music = MusicManager::new(app.http.clone(), app.config.soundcloud_client_id.clone());
    let multiplay = MultiPlayManager::new(
        app.http.clone(),
        app.config.soundcloud_client_id.clone(),
        app.config.multiplay_enabled,
        app.config.multiplay_max_workers,
        app.config.multiplay_auto_select,
        app.config.multiplay_idle_timeout_seconds,
    );
    workers::spawn_music_workers(app.config.music_worker_tokens.clone(), multiplay.clone());
    let mp_for_setup = multiplay.clone();
    let mp_for_shutdown = multiplay.clone();
    let noitu = NoituManager::default();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: all_commands(),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    app,
                    games,
                    music,
                    multiplay: mp_for_setup,
                    noitu,
                })
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_VOICE_STATES;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        .await;

    match client {
        Ok(mut c) => {
            if let Err(e) = c.start().await {
                error!("[Discord] Client loi: {:?}", e);
            }
            mp_for_shutdown.shutdown_all().await;
        }
        Err(e) => error!("[Discord] Khong the tao client: {:?}", e),
    }
}

fn all_commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        help(),
        start(),
        stop(),
        setlang(),
        setuilang(),
        tratu(),
        top(),
        gopthemtu(),
        ask(),
        afk(),
        taixiu(),
        baucua(),
        vtv(),
        quiz(),
        quizmath(),
        wordle(),
        anagram(),
        battleship(),
        memory(),
        music::play(),
        music::pause(),
        music::resume(),
        music::skip(),
        music::mstop(),
        music::queue(),
        music::volume(),
        music::rloop(),
        extra_cmds::setnoitu(),
        extra_cmds::thamgia(),
        extra_cmds::batdaulai(),
        extra_cmds::diem(),
        extra_cmds::contukhong(),
        extra_cmds::ngonngu_hientai(),
        extra_cmds::listlang(),
        extra_cmds::cachestats(),
        extra_cmds::danhsachtu(),
        extra_cmds::taoroom(),
        extra_cmds::multiplayer(),
        extra_cmds::tochucgiaidau(),
        extra_cmds::xemgiaidau(),
        extra_cmds::ketthucgiaidau(),
        extra_cmds::resetpassword_admin(),
        extra_cmds::revoke_2fa(),
        extra_cmds::nowplaying(),
        extra_cmds::cadao(),
        extra_cmds::chinhta(),
        extra_cmds::emoji(),
        extra_cmds::flag(),
        extra_cmds::history(),
        extra_cmds::whoami(),
        extra_cmds::mastermind(),
        extra_cmds::typing(),
        extra_cmds::higherlower(),
        extra_cmds::trieuphu(),
        extra_cmds::catword(),
        extra_cmds::tamsao(),
        extra_cmds::sudoku(),
        extra_cmds::thamgiagiaidau(),
        extra_cmds::duyettu(),
        extra_cmds::connect4(),
        extra_cmds::uno(),
        extra_cmds::wikiquiz(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_existing_command_registry_preserved() {
        let cmds = all_commands();
        let names: Vec<String> = cmds.iter().map(|c| c.name.clone()).collect();
        for expected in [
            "help",
            "start",
            "stop",
            "setlang",
            "setuilang",
            "tratu",
            "top",
            "gopthemtu",
            "ask",
            "afk",
            "taixiu",
            "baucua",
            "vtv",
            "quiz",
            "quizmath",
            "wordle",
            "anagram",
            "battleship",
            "memory",
            "play",
            "pause",
            "resume",
            "skip",
            "mstop",
            "queue",
            "volume",
            "rloop",
            "setnoitu",
            "thamgia",
            "batdaulai",
            "diem",
            "contukhong",
            "taoroom",
            "multiplayer",
            "tochucgiaidau",
            "nowplaying",
            "cadao",
            "chinhta",
            "flag",
            "trieuphu",
            "resetpassword_admin",
            "revoke_2fa",
            "wikiquiz",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "Missing command: {}",
                expected
            );
        }
        assert_eq!(cmds.len(), 62);

        // Verify no duplicate command names or aliases
        let mut seen = std::collections::HashSet::new();
        for c in &cmds {
            assert!(seen.insert(c.name.clone()), "Duplicate cmd: {}", c.name);
            for a in &c.aliases {
                assert!(seen.insert(a.clone()), "Duplicate alias: {}", a);
            }
            assert!(
                c.slash_action.is_some(),
                "Missing slash_action on {}",
                c.name
            );
            if c.name == "resetpassword_admin" {
                assert!(
                    c.prefix_action.is_none(),
                    "resetpassword_admin must be slash-only"
                );
                assert!(
                    c.aliases.is_empty(),
                    "resetpassword_admin must not have aliases"
                );
            } else {
                assert!(
                    c.prefix_action.is_some(),
                    "Missing prefix_action on {}",
                    c.name
                );
            }
        }

        // Verify ONLY /resetpassword_admin exists for admin recovery (no admin_recovery alias/cmd)
        assert_eq!(
            cmds.iter()
                .filter(|c| c.name == "resetpassword_admin")
                .count(),
            1
        );
        assert!(!seen.contains("admin_recovery"));
        assert!(seen.contains("wikiquiz"));
        assert!(seen.contains("dinhnghia"));
        assert!(seen.contains("noitu_start"));
        assert!(seen.contains("noitu_stop"));
        assert!(seen.contains("noitu_setlang"));
        assert!(seen.contains("vuatiengviet"));
        assert!(seen.contains("quizz"));
    }

    #[test]
    fn test_admin_recovery_authorization_rejects_unauthorized() {
        let super_admins = vec![111_222_333u64];
        // 1. Authorized recovery administrator can issue a token
        assert!(can_invoke_admin_recovery(111_222_333, &super_admins));
        // 2. Unauthorized user cannot issue a token
        assert!(!can_invoke_admin_recovery(999_888_777, &super_admins));
        // 3. Ordinary Guild Administrator (not in super_admins) cannot issue
        assert!(!can_invoke_admin_recovery(444_555_666, &super_admins));
    }
}
