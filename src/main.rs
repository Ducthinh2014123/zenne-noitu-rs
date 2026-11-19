use tracing::info;
use tracing_subscriber::EnvFilter;
use zenne_core::AppContext;
use zenne_games::GameHub;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "all".to_string())
        .to_lowercase();

    info!("======================================================================");
    info!("🚀 ZENNE BOT RUST ENGINE (Siêu nhẹ, Zero Memory Leak)");
    info!("======================================================================");

    let ctx = AppContext::init().await?;
    let games = GameHub::default();

    match mode.as_str() {
        "discord" | "dc" => {
            let api_ctx = ctx.clone();
            tokio::spawn(async move {
                zenne_api::start_web_api(api_ctx).await;
            });
            zenne_discord::run_discord_bot(ctx, games).await;
        }
        "telegram" | "tele" | "tg" => {
            zenne_telegram::run_telegram_bot(ctx, games).await;
        }
        "twitch" | "tw" => {
            zenne_twitch::run_twitch_bot(ctx, games).await;
        }
        "revolt" | "stoat" | "rv" => {
            zenne_revolt::run_revolt_bot(ctx, games).await;
        }
        _ => {
            let api_ctx = ctx.clone();
            let h_api = tokio::spawn(async move {
                zenne_api::start_web_api(api_ctx).await;
            });

            let dc_ctx = ctx.clone();
            let dc_games = games.clone();
            let h_dc = tokio::spawn(async move {
                zenne_discord::run_discord_bot(dc_ctx, dc_games).await;
            });

            let tg_ctx = ctx.clone();
            let tg_games = games.clone();
            let h_tg = tokio::spawn(async move {
                zenne_telegram::run_telegram_bot(tg_ctx, tg_games).await;
            });

            let tw_ctx = ctx.clone();
            let tw_games = games.clone();
            let h_tw = tokio::spawn(async move {
                zenne_twitch::run_twitch_bot(tw_ctx, tw_games).await;
            });

            let rv_ctx = ctx.clone();
            let rv_games = games.clone();
            let h_rv = tokio::spawn(async move {
                zenne_revolt::run_revolt_bot(rv_ctx, rv_games).await;
            });

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("🛑 Đã nhận tín hiệu Ctrl+C, đang dừng toàn bộ hệ thống bot...");
                }
                _ = h_api => {}
                _ = h_dc => {}
                _ = h_tg => {}
                _ = h_tw => {}
                _ = h_rv => {}
            }
        }
    }

    Ok(())
}
