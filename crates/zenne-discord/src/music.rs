use crate::Data;
use poise::serenity_prelude as serenity;
use songbird::events::{Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use std::sync::Arc;
use tracing::{error, info};
use zenne_music::multiplay::{AllocationResult, MultiPlayManager, SessionState};
use zenne_music::{MusicManager, SOUNDCLOUD_COLOR, UNSUPPORTED_LINK_MSG};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

struct TrackEndNotifier {
    guild_id: u64,
    channel_id: serenity::ChannelId,
    http: Arc<serenity::Http>,
    songbird: Arc<songbird::Songbird>,
    music: MusicManager,
}

#[serenity::async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        MusicManager::cleanup_guild_audio(self.guild_id).await;
        let _ = play_next_in_guild(
            self.guild_id,
            self.channel_id,
            self.http.clone(),
            self.songbird.clone(),
            self.music.clone(),
            true,
        )
        .await;
        None
    }
}

struct WorkerTrackEndNotifier {
    worker_id: usize,
    guild_id: u64,
    voice_channel_id: u64,
    text_channel_id: serenity::ChannelId,
    main_http: Arc<serenity::Http>,
    multiplay: MultiPlayManager,
}

#[serenity::async_trait]
impl VoiceEventHandler for WorkerTrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        MusicManager::cleanup_worker_guild_audio(self.worker_id, self.guild_id).await;
        let _ = play_next_for_worker(
            self.worker_id,
            self.guild_id,
            self.voice_channel_id,
            self.text_channel_id,
            self.main_http.clone(),
            self.multiplay.clone(),
            true,
        )
        .await;
        None
    }
}

pub async fn play_next_in_guild(
    guild_id: u64,
    channel_id: serenity::ChannelId,
    http: Arc<serenity::Http>,
    songbird: Arc<songbird::Songbird>,
    music: MusicManager,
    notify: bool,
) -> Result<(), String> {
    let state_arc = music.get_state(guild_id);
    let (next_track, volume, loop_single) = {
        let mut st = state_arc.lock().await;
        let next = if st.loop_mode == 1 && st.current.is_some() {
            st.current.clone()
        } else if !st.queue.is_empty() {
            if st.loop_mode == 2 {
                if let Some(cur) = st.current.clone() {
                    st.queue.push(cur);
                }
            }
            Some(st.queue.remove(0))
        } else {
            st.current = None;
            st.current_handle = None;
            None
        };
        st.current = next.clone();
        (next, st.volume, st.loop_mode == 1)
    };

    let Some(track) = next_track else {
        if notify {
            let _ = channel_id
                .say(&http, "🎵 Đã phát hết danh sách nhạc trong hàng đợi!")
                .await;
        }
        return Ok(());
    };

    let path = music.download_audio(&track.stream_url, guild_id).await?;
    let handle = music
        .play_local_file(&songbird, guild_id, path, volume, loop_single)
        .await?;

    let _ = handle.add_event(
        Event::Track(TrackEvent::End),
        TrackEndNotifier {
            guild_id,
            channel_id,
            http: http.clone(),
            songbird: songbird.clone(),
            music: music.clone(),
        },
    );

    {
        let mut st = state_arc.lock().await;
        st.current_handle = Some(handle);
    }

    if notify {
        let embed = serenity::CreateEmbed::new()
            .title("🎵 Đang Phát Nhạc (SoundCloud)")
            .description(format!("**[{}]({})**", track.title, track.permalink_url))
            .color(SOUNDCLOUD_COLOR)
            .field("👤 Tác giả", &track.author, true)
            .field(
                "⏱️ Thời lượng",
                MusicManager::format_ms(track.duration_ms),
                true,
            )
            .field("🔊 Âm lượng", format!("{}%", (volume * 100.0) as u32), true)
            .footer(serenity::CreateEmbedFooter::new(format!(
                "Yêu cầu bởi: {} • Hỗ trợ độc quyền SoundCloud",
                track.requester_name
            )));
        let _ = channel_id
            .send_message(&http, serenity::CreateMessage::new().embed(embed))
            .await;
    }

    Ok(())
}

pub async fn play_next_for_worker(
    worker_id: usize,
    guild_id: u64,
    voice_channel_id: u64,
    text_channel_id: serenity::ChannelId,
    main_http: Arc<serenity::Http>,
    multiplay: MultiPlayManager,
    notify: bool,
) -> Result<(), String> {
    let slot = multiplay
        .get_worker(worker_id)
        .ok_or_else(|| format!("Zenne-Music {} không khả dụng.", worker_id))?;
    let songbird = slot
        .songbird
        .lock()
        .await
        .clone()
        .ok_or_else(|| format!("Zenne-Music {} chưa khởi tạo Songbird.", worker_id))?;

    let state_arc = slot.get_guild_state(guild_id);
    let (next_track, volume, loop_single) = {
        let mut st = state_arc.lock().await;
        let next = if st.loop_mode == 1 && st.current.is_some() {
            st.current.clone()
        } else if !st.queue.is_empty() {
            if st.loop_mode == 2 {
                if let Some(cur) = st.current.clone() {
                    st.queue.push(cur);
                }
            }
            Some(st.queue.remove(0))
        } else {
            st.current = None;
            st.current_handle = None;
            None
        };
        st.current = next.clone();
        (next, st.volume, st.loop_mode == 1)
    };

    let Some(track) = next_track else {
        if notify {
            let _ = text_channel_id
                .say(
                    &main_http,
                    format!(
                        "🎵 **Zenne-Music {}** đã phát hết danh sách nhạc trong hàng đợi!",
                        worker_id
                    ),
                )
                .await;
        }
        // Lên lịch tự động giải phóng worker sau idle_timeout_seconds nếu vẫn rảnh
        let timeout_secs = multiplay.idle_timeout_seconds;
        let mp_bg = multiplay.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
            if let Some(s) = mp_bg.get_worker(worker_id) {
                let is_idle = {
                    let gs = s.get_guild_state(guild_id);
                    let st = gs.lock().await;
                    st.current.is_none() && st.queue.is_empty()
                };
                if is_idle {
                    if let Some(sb) = s.songbird.lock().await.clone() {
                        let _ = sb.remove(serenity::model::id::GuildId::new(guild_id)).await;
                    }
                    mp_bg.release_session(guild_id, voice_channel_id).await;
                }
            }
        });
        return Ok(());
    };

    let path = multiplay
        .audio
        .download_audio_for_worker(&track.stream_url, worker_id, guild_id)
        .await?;
    let handle = multiplay
        .audio
        .play_local_file(&songbird, guild_id, path, volume, loop_single)
        .await?;

    let _ = handle.add_event(
        Event::Track(TrackEvent::End),
        WorkerTrackEndNotifier {
            worker_id,
            guild_id,
            voice_channel_id,
            text_channel_id,
            main_http: main_http.clone(),
            multiplay: multiplay.clone(),
        },
    );

    {
        let mut st = state_arc.lock().await;
        st.current_handle = Some(handle);
    }
    multiplay.set_session_state(guild_id, voice_channel_id, SessionState::Playing);

    if notify {
        let embed = serenity::CreateEmbed::new()
            .title(format!("🎵 Đang Phát Nhạc (Zenne-Music {})", worker_id))
            .description(format!("**[{}]({})**", track.title, track.permalink_url))
            .color(SOUNDCLOUD_COLOR)
            .field("👤 Tác giả", &track.author, true)
            .field(
                "⏱️ Thời lượng",
                MusicManager::format_ms(track.duration_ms),
                true,
            )
            .field("🔊 Âm lượng", format!("{}%", (volume * 100.0) as u32), true)
            .footer(serenity::CreateEmbedFooter::new(format!(
                "Yêu cầu bởi: {} • Hỗ trợ độc quyền SoundCloud",
                track.requester_name
            )));
        let _ = text_channel_id
            .send_message(&main_http, serenity::CreateMessage::new().embed(embed))
            .await;
    }

    Ok(())
}

fn user_voice_channel(ctx: &Context<'_>) -> Option<serenity::ChannelId> {
    let guild = ctx.guild()?;
    guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|vs| vs.channel_id)
}

/// Phát nhạc hoặc tìm kiếm bài hát từ SoundCloud (chỉ hỗ trợ SoundCloud)
#[poise::command(slash_command, prefix_command, aliases("p", "sc"))]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Tên bài hát hoặc liên kết SoundCloud"]
    #[rest]
    query: String,
) -> Result<(), Error> {
    let q = query.trim();
    if q.is_empty() {
        ctx.say("❌ Vui lòng nhập tên bài hát hoặc liên kết SoundCloud!")
            .await?;
        return Ok(());
    }
    if MusicManager::is_url(q) && !MusicManager::is_soundcloud_url(q) {
        ctx.say(UNSUPPORTED_LINK_MSG).await?;
        return Ok(());
    }

    let guild_id = ctx
        .guild_id()
        .ok_or("Lệnh này chỉ dùng được trong server!")?;
    let Some(vc_id) = user_voice_channel(&ctx) else {
        ctx.say("❌ Bạn cần tham gia một kênh thoại (Voice Channel) trước!")
            .await?;
        return Ok(());
    };

    ctx.defer().await?;

    // Nhánh MultiPlay (khi bật và có ít nhất 1 worker sẵn sàng)
    let mp = &ctx.data().multiplay;
    if mp.enabled && mp.ready_worker_count() > 0 {
        let alloc = match mp
            .allocate_or_reuse(guild_id.get(), vc_id.get(), ctx.channel_id().get())
            .await
        {
            Ok(a) => a,
            Err(e) => {
                ctx.say(format!("⚠️ {}", e)).await?;
                return Ok(());
            }
        };

        let (worker_id, is_new_join) = match alloc {
            AllocationResult::Allocated { worker_id } => (worker_id, true),
            AllocationResult::Reuse { worker_id } => (worker_id, false),
        };

        let Some(slot) = mp.get_worker(worker_id) else {
            mp.release_session(guild_id.get(), vc_id.get()).await;
            ctx.say("⚠️ Worker không còn sẵn sàng, vui lòng thử lại!")
                .await?;
            return Ok(());
        };

        if is_new_join {
            mp.set_session_state(guild_id.get(), vc_id.get(), SessionState::Connecting);
            let sb_opt = slot.songbird.lock().await.clone();
            let Some(sb) = sb_opt else {
                mp.release_session(guild_id.get(), vc_id.get()).await;
                ctx.say("⚠️ Worker chưa sẵn sàng kết nối thoại!").await?;
                return Ok(());
            };
            if let Err(e) = sb.join(guild_id, vc_id).await {
                mp.release_session(guild_id.get(), vc_id.get()).await;
                ctx.say(format!(
                    "❌ Zenne-Music {} không thể vào kênh thoại: {}",
                    worker_id, e
                ))
                .await?;
                return Ok(());
            }
            info!(
                "[MultiPlay] Zenne-Music {} đã vào kênh thoại {} tại guild {}",
                worker_id, vc_id, guild_id
            );
        }

        let track = match mp
            .audio
            .resolve_soundcloud_track(q, MusicManager::is_url(q), &ctx.author().name)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                if is_new_join {
                    if let Some(sb) = slot.songbird.lock().await.clone() {
                        let _ = sb.remove(guild_id).await;
                    }
                    mp.release_session(guild_id.get(), vc_id.get()).await;
                }
                ctx.say(format!("⚠️ {}", e)).await?;
                return Ok(());
            }
        };

        let state_arc = slot.get_guild_state(guild_id.get());
        let (should_start, queue_pos, volume) = {
            let mut st = state_arc.lock().await;
            let idle = st.current.is_none();
            st.queue.push(track.clone());
            (idle, st.queue.len(), st.volume)
        };

        if should_start {
            if let Err(e) = play_next_for_worker(
                worker_id,
                guild_id.get(),
                vc_id.get(),
                ctx.channel_id(),
                ctx.serenity_context().http.clone(),
                mp.clone(),
                false,
            )
            .await
            {
                if is_new_join {
                    if let Some(sb) = slot.songbird.lock().await.clone() {
                        let _ = sb.remove(guild_id).await;
                    }
                    mp.release_session(guild_id.get(), vc_id.get()).await;
                }
                ctx.say(format!("⚠️ Lỗi phát nhạc: {}", e)).await?;
                return Ok(());
            }

            let mut embed = serenity::CreateEmbed::new()
                .title(format!("🎵 Đang Phát Nhạc (Zenne-Music {})", worker_id))
                .description(format!("**[{}]({})**", track.title, track.permalink_url))
                .color(SOUNDCLOUD_COLOR)
                .field("👤 Nghệ sĩ", &track.author, true)
                .field(
                    "⏱️ Thời lượng",
                    MusicManager::format_ms(track.duration_ms),
                    true,
                )
                .field("🔊 Âm lượng", format!("{}%", (volume * 100.0) as u32), true)
                .footer(serenity::CreateEmbedFooter::new(format!(
                    "Yêu cầu bởi: {} • Hỗ trợ độc quyền SoundCloud",
                    ctx.author().name
                )));
            if let Some(ref art) = track.artwork {
                embed = embed.thumbnail(art);
            }
            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        } else {
            let embed = serenity::CreateEmbed::new()
                .title(format!(
                    "📥 Đã Thêm Vào Hàng Đợi (Zenne-Music {})",
                    worker_id
                ))
                .description(format!("**[{}]({})**", track.title, track.permalink_url))
                .color(SOUNDCLOUD_COLOR)
                .field("👤 Nghệ sĩ", &track.author, true)
                .field(
                    "⏱️ Thời lượng",
                    MusicManager::format_ms(track.duration_ms),
                    true,
                )
                .field("🔢 Vị trí", format!("#{}", queue_pos), true);
            ctx.send(poise::CreateReply::default().embed(embed)).await?;
        }
        return Ok(());
    }

    // Fallback Single-Bot (giữ nguyên hành vi cũ khi tắt MultiPlay hoặc chưa có worker)
    let manager = songbird::get(ctx.serenity_context())
        .await
        .ok_or("Songbird Voice client chưa được khởi tạo!")?
        .clone();

    if manager.get(guild_id).is_none() {
        if let Err(e) = manager.join(guild_id, vc_id).await {
            ctx.say(format!("❌ Không thể kết nối vào kênh thoại: {}", e))
                .await?;
            return Ok(());
        }
        info!(
            "[Music] Đã tham gia Voice Channel {} tại guild {}",
            vc_id, guild_id
        );
    }

    let track = match ctx
        .data()
        .music
        .resolve_soundcloud_track(q, MusicManager::is_url(q), &ctx.author().name)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            ctx.say(format!("⚠️ {}", e)).await?;
            return Ok(());
        }
    };

    let state_arc = ctx.data().music.get_state(guild_id.get());
    let (should_start, queue_pos, volume) = {
        let mut st = state_arc.lock().await;
        let idle = st.current.is_none();
        st.queue.push(track.clone());
        (idle, st.queue.len(), st.volume)
    };

    if should_start {
        if let Err(e) = play_next_in_guild(
            guild_id.get(),
            ctx.channel_id(),
            ctx.serenity_context().http.clone(),
            manager,
            ctx.data().music.clone(),
            false,
        )
        .await
        {
            error!("[Music] Lỗi phát nhạc: {}", e);
            ctx.say(format!("⚠️ Lỗi phát nhạc: {}", e)).await?;
            return Ok(());
        }

        let mut embed = serenity::CreateEmbed::new()
            .title("🎵 Đang Phát Nhạc (SoundCloud)")
            .description(format!("**[{}]({})**", track.title, track.permalink_url))
            .color(SOUNDCLOUD_COLOR)
            .field("👤 Nghệ sĩ", &track.author, true)
            .field(
                "⏱️ Thời lượng",
                MusicManager::format_ms(track.duration_ms),
                true,
            )
            .field("🔊 Âm lượng", format!("{}%", (volume * 100.0) as u32), true)
            .footer(serenity::CreateEmbedFooter::new(format!(
                "Yêu cầu bởi: {} • Hỗ trợ độc quyền SoundCloud",
                ctx.author().name
            )));
        if let Some(ref art) = track.artwork {
            embed = embed.thumbnail(art);
        }
        ctx.send(poise::CreateReply::default().embed(embed)).await?;
    } else {
        let embed = serenity::CreateEmbed::new()
            .title("📥 Đã Thêm Vào Hàng Đợi (SoundCloud)")
            .description(format!("**[{}]({})**", track.title, track.permalink_url))
            .color(SOUNDCLOUD_COLOR)
            .field("👤 Nghệ sĩ", &track.author, true)
            .field(
                "⏱️ Thời lượng",
                MusicManager::format_ms(track.duration_ms),
                true,
            )
            .field("🔢 Vị trí", format!("#{}", queue_pos), true);
        ctx.send(poise::CreateReply::default().embed(embed)).await?;
    }

    Ok(())
}

async fn resolve_target_state(
    ctx: &Context<'_>,
    guild_id: u64,
) -> Option<(
    Option<usize>,
    Arc<tokio::sync::Mutex<zenne_music::GuildMusicState>>,
)> {
    let mp = &ctx.data().multiplay;
    if mp.enabled && mp.ready_worker_count() > 0 {
        if let Some(vc_id) = user_voice_channel(ctx) {
            if let Some(wid) = mp.get_session_worker(guild_id, vc_id.get()) {
                if let Some(slot) = mp.get_worker(wid) {
                    return Some((Some(wid), slot.get_guild_state(guild_id)));
                }
            }
        }
        return None;
    }
    Some((None, ctx.data().music.get_state(guild_id)))
}

/// Tạm dừng bài hát đang phát
#[poise::command(slash_command, prefix_command)]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let Some((_, state_arc)) = resolve_target_state(&ctx, guild_id.get()).await else {
        ctx.say("❌ Kênh thoại của bạn hiện không có phiên phát nhạc nào!")
            .await?;
        return Ok(());
    };
    let st = state_arc.lock().await.current_handle.clone();
    if let Some(h) = st {
        let _ = h.pause();
        ctx.say("⏸️ Đã tạm dừng phát nhạc!").await?;
    } else {
        ctx.say("❌ Không có bài hát nào đang phát!").await?;
    }
    Ok(())
}

/// Tiếp tục phát nhạc đã tạm dừng
#[poise::command(slash_command, prefix_command)]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let Some((_, state_arc)) = resolve_target_state(&ctx, guild_id.get()).await else {
        ctx.say("❌ Kênh thoại của bạn hiện không có phiên phát nhạc nào!")
            .await?;
        return Ok(());
    };
    let st = state_arc.lock().await.current_handle.clone();
    if let Some(h) = st {
        let _ = h.play();
        ctx.say("▶️ Đã tiếp tục phát nhạc!").await?;
    } else {
        ctx.say("❌ Không có bài hát nào đang tạm dừng!").await?;
    }
    Ok(())
}

/// Bỏ qua bài hát hiện tại
#[poise::command(slash_command, prefix_command, aliases("s", "next"))]
pub async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let Some((_, state_arc)) = resolve_target_state(&ctx, guild_id.get()).await else {
        ctx.say("❌ Kênh thoại của bạn hiện không có phiên phát nhạc nào!")
            .await?;
        return Ok(());
    };
    let st = state_arc.lock().await.current_handle.clone();
    if let Some(h) = st {
        let _ = h.stop();
        ctx.say("⏭️ Đã bỏ qua bài hát hiện tại!").await?;
    } else {
        ctx.say("❌ Không có bài hát nào đang phát!").await?;
    }
    Ok(())
}

/// Dừng phát nhạc, xóa hàng đợi và rời kênh thoại
#[poise::command(slash_command, prefix_command, aliases("musicstop", "dc", "leave"))]
pub async fn mstop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let mp = &ctx.data().multiplay;
    if mp.enabled && mp.ready_worker_count() > 0 {
        let Some(vc_id) = user_voice_channel(&ctx) else {
            ctx.say("❌ Bạn cần ở trong kênh thoại để dừng nhạc của kênh đó!")
                .await?;
            return Ok(());
        };
        if let Some(wid) = mp.get_session_worker(guild_id.get(), vc_id.get()) {
            if let Some(slot) = mp.get_worker(wid) {
                if let Some(sb) = slot.songbird.lock().await.clone() {
                    let _ = sb.remove(guild_id).await;
                }
            }
            mp.release_session(guild_id.get(), vc_id.get()).await;
            ctx.say(format!(
                "⏹️ Đã dừng **Zenne-Music {}**, xóa hàng đợi và rời kênh thoại!",
                wid
            ))
            .await?;
            return Ok(());
        }
        ctx.say("❌ Kênh thoại của bạn hiện không có bot nhạc nào đang phát!")
            .await?;
        return Ok(());
    }

    {
        let state_arc = ctx.data().music.get_state(guild_id.get());
        let mut st = state_arc.lock().await;
        st.queue.clear();
        st.current = None;
        if let Some(h) = st.current_handle.take() {
            let _ = h.stop();
        }
    }
    if let Some(manager) = songbird::get(ctx.serenity_context()).await {
        let _ = manager.remove(guild_id).await;
    }
    MusicManager::cleanup_guild_audio(guild_id.get()).await;
    ctx.say("⏹️ Đã dừng phát nhạc, xóa hàng đợi và rời kênh thoại!")
        .await?;
    Ok(())
}

/// Xem danh sách hàng đợi hiện tại
#[poise::command(slash_command, prefix_command, aliases("q"))]
pub async fn queue(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let Some((wid_opt, state_arc)) = resolve_target_state(&ctx, guild_id.get()).await else {
        ctx.say("📭 Hàng đợi phát nhạc của kênh thoại này hiện đang trống!")
            .await?;
        return Ok(());
    };
    let st = state_arc.lock().await;
    if st.current.is_none() && st.queue.is_empty() {
        ctx.say("📭 Hàng đợi phát nhạc hiện đang trống!").await?;
        return Ok(());
    }
    let mut desc = String::new();
    if let Some(ref cur) = st.current {
        desc.push_str(&format!(
            "▶️ **Đang phát:** [{}]({}) `[{}]`\n\n",
            cur.title,
            cur.permalink_url,
            MusicManager::format_ms(cur.duration_ms)
        ));
    }
    for (i, tr) in st.queue.iter().take(10).enumerate() {
        desc.push_str(&format!(
            "`{}.` [{}]({}) `[{}]`\n",
            i + 1,
            tr.title,
            tr.permalink_url,
            MusicManager::format_ms(tr.duration_ms)
        ));
    }
    let title = match wid_opt {
        Some(wid) => format!("📜 Danh Sách Hàng Đợi (Zenne-Music {})", wid),
        None => "📜 Danh Sách Hàng Đợi (SoundCloud)".to_string(),
    };
    let embed = serenity::CreateEmbed::new()
        .title(title)
        .description(desc)
        .color(SOUNDCLOUD_COLOR);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Điều chỉnh âm lượng (0 - 150%)
#[poise::command(slash_command, prefix_command, aliases("vol"))]
pub async fn volume(ctx: Context<'_>, level: u32) -> Result<(), Error> {
    if level > 150 {
        ctx.say("⚠️ Vui lòng nhập âm lượng từ 0 đến 150%!").await?;
        return Ok(());
    }
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let Some((_, state_arc)) = resolve_target_state(&ctx, guild_id.get()).await else {
        ctx.say("❌ Kênh thoại của bạn hiện không có phiên phát nhạc nào!")
            .await?;
        return Ok(());
    };
    let vol = level as f32 / 100.0;
    let mut st = state_arc.lock().await;
    st.volume = vol;
    if let Some(ref h) = st.current_handle {
        let _ = h.set_volume(vol);
    }
    ctx.say(format!("🔊 Đã chỉnh âm lượng thành **{}%**!", level))
        .await?;
    Ok(())
}

/// Chuyển đổi chế độ lặp lại (Tắt -> Bài hiện tại -> Hàng đợi)
#[poise::command(slash_command, prefix_command)]
pub async fn rloop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Chỉ dùng trong server!")?;
    let Some((_, state_arc)) = resolve_target_state(&ctx, guild_id.get()).await else {
        ctx.say("❌ Kênh thoại của bạn hiện không có phiên phát nhạc nào!")
            .await?;
        return Ok(());
    };
    let mut st = state_arc.lock().await;
    st.loop_mode = (st.loop_mode + 1) % 3;
    let mode = st.loop_mode;
    if let Some(ref h) = st.current_handle {
        MusicManager::toggle_loop_handle(h, mode == 1);
    }
    let msg = match mode {
        1 => "🔂 Đã bật: **Lặp lại bài hiện tại**",
        2 => "🔁 Đã bật: **Lặp lại toàn bộ hàng đợi**",
        _ => "➡️ Đã **tắt** chế độ lặp lại",
    };
    ctx.say(msg).await?;
    Ok(())
}
