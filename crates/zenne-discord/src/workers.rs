use poise::serenity_prelude as serenity;
use songbird::SerenityInit;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info, warn};
use zenne_music::multiplay::MultiPlayManager;

struct WorkerHandler {
    worker_id: usize,
    multiplay: MultiPlayManager,
    bot_user_id: AtomicU64,
}

#[serenity::async_trait]
impl serenity::EventHandler for WorkerHandler {
    async fn ready(&self, ctx: serenity::Context, ready: serenity::Ready) {
        let uid = ready.user.id.get();
        self.bot_user_id.store(uid, Ordering::Release);
        let sb = songbird::get(&ctx).await;
        self.multiplay
            .register_worker(self.worker_id, uid, sb, Some(ctx.http.clone()))
            .await;
        info!(
            "[MultiPlay] Zenne-Music {} dang nhap thanh cong: {} (ID: {})",
            self.worker_id, ready.user.name, uid
        );
    }

    async fn voice_state_update(
        &self,
        _ctx: serenity::Context,
        _old: Option<serenity::VoiceState>,
        new: serenity::VoiceState,
    ) {
        let self_id = self.bot_user_id.load(Ordering::Acquire);
        if self_id == 0 || new.user_id.get() != self_id {
            return;
        }
        if new.channel_id.is_none() {
            if let Some(gid) = new.guild_id {
                self.multiplay
                    .release_worker_guild(self.worker_id, gid.get())
                    .await;
            }
        }
    }
}

/// Khởi chạy toàn bộ các worker Zenne-Music 1..=10 (chỉ giữ kết nối Voice, tuyệt đối không nhận lệnh người dùng)
pub fn spawn_music_workers(tokens: Vec<(usize, String)>, multiplay: MultiPlayManager) {
    if !multiplay.enabled {
        info!("[MultiPlay] MULTIPLAY_ENABLED=false, giu che do single-bot.");
        return;
    }
    if tokens.is_empty() {
        info!(
            "[MultiPlay] Chua cau hinh ZENNE_MUSIC_*_TOKEN nao, fallback sang che do single-bot."
        );
        return;
    }

    for (worker_id, token) in tokens {
        let mp = multiplay.clone();
        tokio::spawn(async move {
            let intents =
                serenity::GatewayIntents::GUILDS | serenity::GatewayIntents::GUILD_VOICE_STATES;
            let handler = WorkerHandler {
                worker_id,
                multiplay: mp.clone(),
                bot_user_id: AtomicU64::new(0),
            };
            let client = serenity::ClientBuilder::new(token, intents)
                .event_handler(handler)
                .register_songbird()
                .await;

            match client {
                Ok(mut c) => {
                    if let Err(e) = c.start().await {
                        error!(
                            "[MultiPlay] Zenne-Music {} dung voi loi: {:?}",
                            worker_id, e
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "[MultiPlay] Khong the khoi tao Zenne-Music {} (token loi hoac mat mang): {:?}",
                        worker_id, e
                    );
                }
            }
            mp.unregister_worker(worker_id);
        });
    }
}
