use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub discord_token: String,
    pub discord_proxy: Option<String>,
    pub telegram_bot_token: String,
    pub turn_timeout: u64,
    pub database_url: String,
    pub database_auth_token: String,
    pub sqlite_path: String,
    pub redis_url: String,
    pub kira_api_key: String,
    pub kira_api_endpoint: String,
    pub kira_default_model: String,
    pub api_key: String,
    pub api_port: u16,
    pub web_base_url: String,
    pub web_token_secret: String,
    pub game_session_secret: String,
    pub twitch_bot_nick: String,
    pub twitch_oauth_token: String,
    pub twitch_channels: Vec<String>,
    pub twitch_command_prefix: String,
    pub revolt_bot_token: String,
    pub revolt_command_prefix: String,
    pub soundcloud_client_id: String,
    pub lavalink_uri: String,
    pub lavalink_password: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub smtp_from: String,
    pub super_admin_ids: Vec<u64>,
    pub multiplay_enabled: bool,
    pub multiplay_max_workers: usize,
    pub multiplay_auto_select: bool,
    pub multiplay_idle_timeout_seconds: u64,
    pub music_worker_tokens: Vec<(usize, String)>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let env_str = |k: &str, default: &str| -> String {
            env::var(k)
                .ok()
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| default.to_string())
        };

        let discord_proxy = env::var("DISCORD_PROXY")
            .ok()
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty());

        let twitch_channels = env_str("TWITCH_CHANNELS", "")
            .split(',')
            .map(|s| s.trim().trim_start_matches('#').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut super_admin_ids: Vec<u64> = env_str("SUPER_ADMIN_IDS", "1395221255589986374")
            .split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect();
        for extra_key in ["SUPER_ADMIN", "ADMIN_ID"] {
            if let Ok(id) = env_str(extra_key, "").parse::<u64>() {
                if !super_admin_ids.contains(&id) {
                    super_admin_ids.push(id);
                }
            }
        }

        let api_key = env_str("API_KEY", "Duc thinh 1234@");
        let web_token_secret = env_str("WEB_TOKEN_SECRET", &api_key);
        let game_session_secret = env_str("GAME_SESSION_SECRET", &web_token_secret);

        let multiplay_enabled = matches!(
            env_str("MULTIPLAY_ENABLED", "true").to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        );
        let multiplay_max_workers = env_str("MULTIPLAY_MAX_WORKERS", "10")
            .parse::<usize>()
            .unwrap_or(10)
            .clamp(1, 10);
        let multiplay_auto_select = matches!(
            env_str("MULTIPLAY_AUTO_SELECT", "true")
                .to_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        );
        let multiplay_idle_timeout_seconds = env_str("MULTIPLAY_IDLE_TIMEOUT_SECONDS", "300")
            .parse::<u64>()
            .unwrap_or(300);

        let mut music_worker_tokens = Vec::new();
        for idx in 1..=multiplay_max_workers {
            let key = format!("ZENNE_MUSIC_{}_TOKEN", idx);
            let tok = env_str(&key, "");
            if !tok.is_empty() {
                music_worker_tokens.push((idx, tok));
            }
        }

        Self {
            discord_token: env_str("DISCORD_TOKEN", ""),
            discord_proxy,
            telegram_bot_token: env_str("TELEGRAM_BOT_TOKEN", ""),
            turn_timeout: env_str("TURN_TIMEOUT", "60").parse().unwrap_or(60),
            database_url: env_str("DATABASE_URL", ""),
            database_auth_token: env_str("DATABASE_AUTH_TOKEN", ""),
            sqlite_path: env_str("SQLITE_PATH", "noitu_bot.db"),
            redis_url: env_str("REDIS_URL", ""),
            kira_api_key: env_str("KIRA_API_KEY", ""),
            kira_api_endpoint: env_str("KIRA_API_ENDPOINT", "https://kiraai.vn/api/v1"),
            kira_default_model: env_str("KIRA_DEFAULT_MODEL", "stealth-missingno"),
            api_key,
            api_port: env_str("API_PORT", "26035").parse().unwrap_or(26035),
            web_base_url: env_str("WEB_BASE_URL", "https://zenne-noitu.me"),
            web_token_secret,
            game_session_secret,
            twitch_bot_nick: env_str("TWITCH_BOT_NICK", "zenne_noitu_bot"),
            twitch_oauth_token: env_str("TWITCH_OAUTH_TOKEN", ""),
            twitch_channels,
            twitch_command_prefix: env_str("TWITCH_COMMAND_PREFIX", "!"),
            revolt_bot_token: env_str("REVOLT_BOT_TOKEN", ""),
            revolt_command_prefix: env_str("REVOLT_COMMAND_PREFIX", "!"),
            soundcloud_client_id: env_str(
                "SOUNDCLOUD_CLIENT_ID",
                "QhVEWrrJKvW4uSU5u5EoI0FTXYUGzy5k",
            ),
            lavalink_uri: env_str("LAVALINK_URI", "http://june.hidencloud.com:24605"),
            lavalink_password: env_str("LAVALINK_PASSWORD", "youshallnotpass"),
            smtp_host: env_str("SMTP_HOST", "smtp-relay.brevo.com"),
            smtp_port: env_str("SMTP_PORT", "587").parse().unwrap_or(587),
            smtp_user: env_str("SMTP_USER", ""),
            smtp_pass: env_str("SMTP_PASS", ""),
            smtp_from: env_str("SMTP_FROM", ""),
            super_admin_ids,
            multiplay_enabled,
            multiplay_max_workers,
            multiplay_auto_select,
            multiplay_idle_timeout_seconds,
            music_worker_tokens,
        }
    }
}
