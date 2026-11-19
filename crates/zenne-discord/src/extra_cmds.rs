use crate::{Context, Error};
use zenne_core::db::PublicRoomRow;
use zenne_core::{i18n, wiki};

/// Cố định kênh hiện tại làm kênh Nối Từ của server
#[poise::command(slash_command, prefix_command)]
pub async fn setnoitu(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let ch_id = ctx.channel_id().get() as i64;
    ctx.data().app.db.set_game_channel(guild_id, ch_id).await;
    ctx.say(format!("✅ Đã đặt kênh <#{}> làm kênh Nối Từ chính thức!", ch_id))
        .await?;
    Ok(())
}

/// Tham gia ván Nối Từ đang mở
#[poise::command(slash_command, prefix_command, aliases("join"))]
pub async fn thamgia(ctx: Context<'_>) -> Result<(), Error> {
    let room_arc = ctx.data().noitu.get_room(ctx.channel_id().get());
    let mut r = room_arc.lock().await;
    if !r.active {
        ctx.say("❌ Chưa có ván Nối Từ nào đang chạy! Dùng `!start` để mở ván mới.")
            .await?;
        return Ok(());
    }
    let uid = ctx.author().id.get();
    if !r.players.contains(&uid) {
        r.players.push(uid);
    }
    ctx.say(format!(
        "✅ **{}** đã tham gia ván Nối Từ! (Tổng: {} người chơi)",
        ctx.author().name,
        r.players.len()
    ))
    .await?;
    Ok(())
}

/// Đổi từ mở đầu mới cho ván Nối Từ hiện tại
#[poise::command(slash_command, prefix_command, aliases("newword"))]
pub async fn batdaulai(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let lang = ctx.data().app.db.get_lang(guild_id).await;
    let new_word = wiki::random_word(&ctx.data().app.http, &ctx.data().app.db, &lang).await;
    let room_arc = ctx.data().noitu.get_room(ctx.channel_id().get());
    let mut r = room_arc.lock().await;
    if !r.active {
        ctx.say("❌ Chưa có ván Nối Từ nào đang chạy!").await?;
        return Ok(());
    }
    r.last_word = new_word.clone();
    r.used.insert(new_word.clone());
    let next_syl = wiki::last_syl(&new_word, &lang);
    ctx.say(format!(
        "🔄 Đã đổi từ mới: **{}** → Hãy nối tiếp bằng **`{}`**!",
        new_word, next_syl
    ))
    .await?;
    Ok(())
}

/// Xem điểm số ván Nối Từ hiện tại trong kênh
#[poise::command(slash_command, prefix_command, aliases("score"))]
pub async fn diem(ctx: Context<'_>) -> Result<(), Error> {
    let room_arc = ctx.data().noitu.get_room(ctx.channel_id().get());
    let r = room_arc.lock().await;
    if r.scores.is_empty() {
        ctx.say("📊 Chưa có điểm nào trong ván này.").await?;
        return Ok(());
    }
    let mut items: Vec<(u64, u32)> = r.scores.iter().map(|(k, v)| (*k, *v)).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));
    let lines = items
        .iter()
        .enumerate()
        .map(|(i, (uid, pts))| format!("{}. <@{}>: **{}** điểm", i + 1, uid, pts))
        .collect::<Vec<_>>()
        .join("\n");
    ctx.say(format!("📊 **Bảng điểm ván hiện tại:**\n{}", lines))
        .await?;
    Ok(())
}

/// Kiểm tra âm tiết có từ nối tiếp không
#[poise::command(slash_command, prefix_command, aliases("ctk"))]
pub async fn contukhong(ctx: Context<'_>, syllable: String) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let lang = ctx.data().app.db.get_lang(guild_id).await;
    let sugs = wiki::find_continuations(
        &ctx.data().app.db,
        guild_id,
        &lang,
        syllable.trim(),
        &[],
        8,
    )
    .await;
    if sugs.is_empty() {
        ctx.say(format!(
            "⚠️ Không tìm thấy từ nối tiếp cho âm tiết `{}` ({}).",
            syllable.trim(),
            lang
        ))
        .await?;
    } else {
        ctx.say(format!(
            "✅ Âm tiết `{}` còn từ nối! Gợi ý: {}",
            syllable.trim(),
            sugs.iter()
                .map(|w| format!("`{}`", w))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .await?;
    }
    Ok(())
}

/// Xem ngôn ngữ từ điển hiện tại của server
#[poise::command(slash_command, prefix_command)]
pub async fn ngonngu_hientai(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let lang = ctx.data().app.db.get_lang(guild_id).await;
    ctx.say(format!(
        "🌐 Ngôn ngữ Nối Từ hiện tại: **{}** (`{}`)",
        i18n::lang_display_name(&lang),
        lang
    ))
    .await?;
    Ok(())
}

/// Liệt kê các ngôn ngữ phổ biến được hỗ trợ
#[poise::command(slash_command, prefix_command)]
pub async fn listlang(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(
        "🌐 **Hỗ trợ 300+ ngôn ngữ Wikipedia!** Một số mã phổ biến:\n`vi` (Tiếng Việt), `en` (English), `ja` (Nhật), `ko` (Hàn), `zh` (Trung), `fr` (Pháp), `de` (Đức), `es` (Tây Ban Nha), `ru` (Nga), `th` (Thái)\nDùng `!setlang <mã>` để đổi.",
    )
    .await?;
    Ok(())
}

/// Xem thống kê kho từ & cache của server
#[poise::command(slash_command, prefix_command)]
pub async fn cachestats(ctx: Context<'_>) -> Result<(), Error> {
    let stats = ctx.data().app.db.stats_summary().await;
    ctx.say(format!("📊 **Thống kê hệ thống:**\n```json\n{}\n```", stats))
        .await?;
    Ok(())
}

/// Xem danh sách từ trong ngân hàng từ của server
#[poise::command(slash_command, prefix_command, aliases("tutruong", "wordbank"))]
pub async fn danhsachtu(ctx: Context<'_>) -> Result<(), Error> {
    let (words, total) = ctx.data().app.db.pub_wordbank("", 1, 20).await;
    let list: Vec<String> = words
        .iter()
        .filter_map(|w| w.get("word").and_then(|v| v.as_str()).map(|s| format!("`{}`", s)))
        .collect();
    ctx.say(format!(
        "📚 **Ngân hàng từ (Tổng: {} từ, 20 từ gần nhất):**\n{}",
        total,
        if list.is_empty() { "Trống".to_string() } else { list.join(", ") }
    ))
    .await?;
    Ok(())
}

/// Tạo phòng chơi Online Web/Discord đồng bộ
#[poise::command(slash_command, prefix_command)]
pub async fn taoroom(ctx: Context<'_>, name: Option<String>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let lang = ctx.data().app.db.get_lang(guild_id).await;
    let room_id = format!("{:06X}", (ctx.id() ^ ctx.author().id.get()) & 0xFFFFFF);
    let room_name = name.unwrap_or_else(|| format!("Phòng của {}", ctx.author().name));
    let web_base = ctx.data().app.config.web_base_url.trim_end_matches('/');
    let web_url = format!("{}/pub/play/{}", web_base, room_id);

    ctx.data()
        .app
        .db
        .create_public_room(&PublicRoomRow {
            room_id: room_id.clone(),
            name: room_name.clone(),
            platform: "discord".to_string(),
            host_id: ctx.author().id.get().to_string(),
            host_name: ctx.author().name.clone(),
            lang,
            max_players: 0,
            current_players: 1,
            join_token: None,
            web_url: Some(web_url.clone()),
            tele_url: None,
            status: "waiting".to_string(),
        })
        .await;

    ctx.say(format!(
        "🎉 **Đã tạo phòng Multiplayer công khai!**\n🏷️ Tên: **{}** (`[{}]`)\n🔗 Link vào phòng: {}",
        room_name, room_id, web_url
    ))
    .await?;
    Ok(())
}

/// Xem danh sách phòng Multiplayer công khai đang mở
#[poise::command(slash_command, prefix_command, aliases("mp"))]
pub async fn multiplayer(ctx: Context<'_>) -> Result<(), Error> {
    let rooms = ctx.data().app.db.get_public_rooms().await;
    if rooms.is_empty() {
        ctx.say("🌐 Hiện chưa có phòng công khai nào đang mở. Dùng `!taoroom` để tạo mới!")
            .await?;
        return Ok(());
    }
    let lines = rooms
        .iter()
        .take(10)
        .map(|r| {
            format!(
                "• **[{}]** {} — Host: `{}` ({}) | `{}`",
                r.room_id,
                r.name,
                r.host_name,
                r.platform,
                r.web_url.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    ctx.say(format!("🌐 **Sảnh Multiplayer Công Khai:**\n{}", lines))
        .await?;
    Ok(())
}

/// Tổ chức giải đấu Nối Từ mới trong server
#[poise::command(slash_command, prefix_command)]
pub async fn tochucgiaidau(
    ctx: Context<'_>,
    name: String,
    prize: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    let ch_id = ctx.channel_id().get() as i64;
    let host_id = ctx.author().id.get() as i64;
    if let Ok(conn) = ctx.data().app.db.conn() {
        let _ = conn
            .execute(
                "INSERT INTO tournaments (guild_id, channel_id, host_id, name, prize, status, started_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', datetime('now','localtime'))",
                (guild_id, ch_id, host_id, name.as_str(), prize.as_deref().unwrap_or("Vinh danh")),
            )
            .await;
    }
    ctx.say(format!(
        "🏆 **Giải đấu Nối Từ \"{}\" đã mở!**\n🎁 Phần thưởng: {}\nDùng `!xemgiaidau` để xem trạng thái.",
        name,
        prize.unwrap_or_else(|| "Vinh danh".to_string())
    ))
    .await?;
    Ok(())
}

/// Xem giải đấu đang diễn ra trong server
#[poise::command(slash_command, prefix_command)]
pub async fn xemgiaidau(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    if let Ok(conn) = ctx.data().app.db.conn() {
        if let Ok(mut rows) = conn
            .query(
                "SELECT id, name, prize, status FROM tournaments WHERE guild_id = ?1 AND status = 'active' ORDER BY id DESC LIMIT 1",
                [guild_id],
            )
            .await
        {
            if let Ok(Some(r)) = rows.next().await {
                let id: i64 = r.get(0).unwrap_or(0);
                let name: String = r.get(1).unwrap_or_default();
                let prize: String = r.get(2).unwrap_or_default();
                ctx.say(format!(
                    "🏆 **Giải đấu #{} — {}**\n🎁 Phần thưởng: {}\n📌 Trạng thái: Đang diễn ra",
                    id, name, prize
                ))
                .await?;
                return Ok(());
            }
        }
    }
    ctx.say("⚠️ Server hiện không có giải đấu nào đang mở.").await?;
    Ok(())
}

/// Kết thúc giải đấu hiện tại của server
#[poise::command(slash_command, prefix_command)]
pub async fn ketthucgiaidau(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get() as i64).unwrap_or(0);
    if let Ok(conn) = ctx.data().app.db.conn() {
        let _ = conn
            .execute(
                "UPDATE tournaments SET status = 'ended', ended_at = datetime('now','localtime') WHERE guild_id = ?1 AND status = 'active'",
                [guild_id],
            )
            .await;
    }
    ctx.say("🏁 Đã kết thúc giải đấu hiện tại của server!").await?;
    Ok(())
}

/// Admin: Gỡ 2FA cho tài khoản web theo username/email
#[poise::command(slash_command, prefix_command)]
pub async fn revoke_2fa(ctx: Context<'_>, identifier: String) -> Result<(), Error> {
    let uid = ctx.author().id.get();
    if !ctx.data().app.config.super_admin_ids.contains(&uid) {
        ctx.say("⛔ Chỉ Super Admin mới có quyền dùng lệnh này!").await?;
        return Ok(());
    }
    if let Ok(conn) = ctx.data().app.db.conn() {
        let _ = conn
            .execute(
                "UPDATE web_users SET totp_enabled = 0, totp_secret = NULL WHERE username = ?1 OR email = ?1",
                [identifier.trim()],
            )
            .await;
    }
    ctx.say(format!("✅ Đã gỡ 2FA cho tài khoản `{}`.", identifier.trim()))
        .await?;
    Ok(())
}

/// Xem bài nhạc SoundCloud đang phát hiện tại
#[poise::command(slash_command, prefix_command, aliases("np"))]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().map(|g| g.get()).unwrap_or(0);
    let st_arc = ctx.data().music.get_state(guild_id);
    let guard = st_arc.lock().await;
    if let Some(ref track) = guard.current {
        ctx.say(format!(
            "🎵 Đang phát: **{}** ({})",
            track.title, track.permalink_url
        ))
        .await?;
    } else if let Some(track) = guard.queue.first() {
        ctx.say(format!(
            "🎵 Hàng chờ tiếp theo: **{}** ({})",
            track.title, track.permalink_url
        ))
        .await?;
    } else {
        ctx.say("🔇 Hiện không có bài nào đang phát.").await?;
    }
    Ok(())
}

// ── Extra Minigame Commands ──

/// Chơi điền khuyết Ca Dao Tục Ngữ
#[poise::command(slash_command, prefix_command)]
pub async fn cadao(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.cadao.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chính xác! Đáp án: **{}**", ans)
            } else {
                format!("❌ Sai rồi! Thử lại nhé.")
            })
            .await?;
            return Ok(());
        }
    }
    let s = ctx.data().games.cadao.start(ch);
    ctx.say(format!("🌾 **Ca Dao:** *\"{}\"* — Gõ `!cadao <đáp án>` để trả lời!", s.prompt))
        .await?;
    Ok(())
}

/// Thử thách chính tả Tiếng Việt
#[poise::command(slash_command, prefix_command)]
pub async fn chinhta(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.chinhta.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chuẩn xác! Từ đúng là **{}**", ans)
            } else {
                format!("❌ Chưa đúng! Đáp án chuẩn là **{}**", ans)
            })
            .await?;
            return Ok(());
        }
    }
    let s = ctx.data().games.chinhta.start(ch);
    ctx.say(format!(
        "✍️ **Chính Tả:** Từ nào đúng? **{}** hay **{}**? (Gõ `!chinhta <từ>`)",
        s.opt_a, s.opt_b
    ))
    .await?;
    Ok(())
}

/// Đuổi hình bắt chữ Emoji
#[poise::command(slash_command, prefix_command)]
pub async fn emoji(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.emoji.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chính xác: **{}**!", ans)
            } else {
                "❌ Chưa đúng, thử lại nhé!".to_string()
            })
            .await?;
            return Ok(());
        }
    }
    let (icons, _) = ctx.data().games.emoji.start(ch);
    ctx.say(format!("😎 **Đoán chữ qua Emoji:** {} — Gõ `!emoji <đáp án>`!", icons))
        .await?;
    Ok(())
}

/// Đoán Quốc Kỳ thế giới
#[poise::command(slash_command, prefix_command)]
pub async fn flag(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.flag_quiz.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chính xác! Quốc gia: **{}**", ans)
            } else {
                format!("❌ Chưa đúng! Gợi ý lại hoặc thử nước khác.")
            })
            .await?;
            return Ok(());
        }
    }
    let (f, _) = ctx.data().games.flag_quiz.start(ch);
    ctx.say(format!("🌍 **Quốc kỳ:** {} — Gõ `!flag <tên nước>` để đoán!", f))
        .await?;
    Ok(())
}

/// Đố sự kiện Lịch Sử
#[poise::command(slash_command, prefix_command)]
pub async fn history(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.history_guess.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chính xác: **{}**!", ans)
            } else {
                "❌ Chưa đúng!".to_string()
            })
            .await?;
            return Ok(());
        }
    }
    let (q, _) = ctx.data().games.history_guess.start(ch);
    ctx.say(format!("📜 **Lịch Sử:** {} — Gõ `!history <đáp án>`!", q))
        .await?;
    Ok(())
}

/// Tôi là ai (Who Am I)
#[poise::command(slash_command, prefix_command)]
pub async fn whoami(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.whoami.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Đúng rồi! Là **{}**!", ans)
            } else {
                "❌ Chưa đúng!".to_string()
            })
            .await?;
            return Ok(());
        }
    }
    let (clue, _) = ctx.data().games.whoami.start(ch);
    ctx.say(format!("🕵️ **Tôi là ai?** {} — Gõ `!whoami <đáp án>`!", clue))
        .await?;
    Ok(())
}

/// Game đoán mã số Mastermind
#[poise::command(slash_command, prefix_command)]
pub async fn mastermind(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        match ctx.data().games.mastermind.guess(ch, &g) {
            Ok((bulls, cows, won, tries)) => {
                ctx.say(if won {
                    format!("🎉 Phá mã thành công sau {} lượt!", tries)
                } else {
                    format!("🔢 Lượt {}: **{}** đúng vị trí, **{}** sai vị trí.", tries, bulls, cows)
                })
                .await?;
            }
            Err(e) => {
                ctx.say(format!("⚠️ {}", e)).await?;
            }
        }
        return Ok(());
    }
    ctx.data().games.mastermind.start(ch);
    ctx.say("🔐 **Mastermind** đã bắt đầu! Hãy đoán dãy 4 chữ số bằng `!mastermind <4 số>`!")
        .await?;
    Ok(())
}

/// Thử thách tốc độ gõ phím (Typing Test)
#[poise::command(slash_command, prefix_command)]
pub async fn typing(ctx: Context<'_>, typed: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(t) = typed {
        if let Some((ok, wpm, secs)) = ctx.data().games.typing.submit(ch, &t) {
            ctx.say(if ok {
                format!("⌨️ Tuyệt vời! Tốc độ: **{:.1} WPM** ({:.2}s)", wpm, secs)
            } else {
                "❌ Bạn gõ chưa khớp hoàn toàn với câu mẫu!".to_string()
            })
            .await?;
            return Ok(());
        }
    }
    let sentence = ctx.data().games.typing.start(ch);
    ctx.say(format!("⌨️ **Typing Test:** Hãy gõ chính xác câu sau bằng `!typing <câu>`:\n`{}`", sentence))
        .await?;
    Ok(())
}

/// Game Cao/Thấp (Higher or Lower)
#[poise::command(slash_command, prefix_command)]
pub async fn higherlower(ctx: Context<'_>, choice: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(c) = choice {
        let high = matches!(c.to_lowercase().as_str(), "cao" | "high" | "h" | "len");
        if let Some((ok, prev, next, streak)) = ctx.data().games.higher_lower.guess(ch, high) {
            ctx.say(if ok {
                format!("✅ Số mới là **{}** (trước: **{}**)! Chuỗi thắng: 🔥{}", next, prev, streak)
            } else {
                format!("❌ Số mới là **{}** (trước: **{}**). Kết thúc chuỗi: {}", next, prev, streak)
            })
            .await?;
            return Ok(());
        }
    }
    let start_num = ctx.data().games.higher_lower.start(ch);
    ctx.say(format!("🔼🔽 **Higher/Lower:** Số hiện tại là **{}** (1-100). Đoán `!higherlower cao` hoặc `thap`!", start_num))
        .await?;
    Ok(())
}

/// Gameshow Ai Là Triệu Phú
#[poise::command(slash_command, prefix_command)]
pub async fn trieuphu(ctx: Context<'_>, ans: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(a) = ans {
        let idx = match a.trim().to_uppercase().as_str() {
            "A" | "1" => 0,
            "B" | "2" => 1,
            "C" | "3" => 2,
            _ => 3,
        };
        if let Some((ok, lvl, prize, next)) = ctx.data().games.trieuphu.answer(ch, idx) {
            if ok {
                if let Some(nq) = next {
                    ctx.say(format!(
                        "🎉 Chính xác câu {} ({} VNĐ)!\n**Câu {}:** {}\nA. {} | B. {} | C. {} | D. {}",
                        lvl, prize, nq.level, nq.current.question,
                        nq.current.options[0], nq.current.options[1], nq.current.options[2], nq.current.options[3]
                    ))
                    .await?;
                } else {
                    ctx.say("👑 XUẤT SẮC! Bạn đã vượt qua câu 15 và giành **150.000.000 VNĐ**!").await?;
                }
            } else {
                ctx.say(format!("❌ Sai ở câu {}! Ra về với **{} VNĐ**.", lvl, prize)).await?;
            }
            return Ok(());
        }
    }
    let s = ctx.data().games.trieuphu.start(ch, ctx.author().id.get());
    ctx.say(format!(
        "👑 **Ai Là Triệu Phú — Câu 1:** {}\nA. {} | B. {} | C. {} | D. {}\nDùng `!trieuphu <A/B/C/D>` để trả lời!",
        s.current.question, s.current.options[0], s.current.options[1], s.current.options[2], s.current.options[3]
    ))
    .await?;
    Ok(())
}

/// Game Cắt Chữ đoán từ
#[poise::command(slash_command, prefix_command)]
pub async fn catword(ctx: Context<'_>, guess: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(g) = guess {
        if let Some((ok, ans)) = ctx.data().games.catword.check_answer(ch, &g) {
            ctx.say(if ok {
                format!("🎉 Chính xác: **{}**!", ans)
            } else {
                "❌ Chưa đúng!".to_string()
            })
            .await?;
            return Ok(());
        }
    }
    let s = ctx.data().games.catword.start(ch);
    ctx.say(format!("✂️ **Cắt Chữ:** `{}`\n💡 Gợi ý: *{}*\nGõ `!catword <đáp án>`!", s.masked, s.hint))
        .await?;
    Ok(())
}

/// Game Tam Sao Thất Bản
#[poise::command(slash_command, prefix_command)]
pub async fn tamsao(ctx: Context<'_>, typed: Option<String>) -> Result<(), Error> {
    let ch = ctx.channel_id().get();
    if let Some(t) = typed {
        if let Some((ok, orig)) = ctx.data().games.tamsao.check(ch, &t) {
            ctx.say(if ok {
                "🎉 Chuẩn không cần chỉnh!".to_string()
            } else {
                format!("❌ Sai rồi! Câu gốc là: *{}*", orig)
            })
            .await?;
            return Ok(());
        }
    }
    let phrase = ctx.data().games.tamsao.start(ch);
    ctx.say(format!("🗣️ **Tam Sao Thất Bản:** Hãy gõ lại chính xác: `{}` bằng `!tamsao <câu>`!", phrase))
        .await?;
    Ok(())
}

/// Bắt đầu ván Sudoku
#[poise::command(slash_command, prefix_command)]
pub async fn sudoku(ctx: Context<'_>) -> Result<(), Error> {
    let (board, _) = ctx.data().games.sudoku.start(ctx.channel_id().get());
    ctx.say(format!("🔢 **Sudoku:**\n```\n{}\n```", board))
        .await?;
    Ok(())
}
