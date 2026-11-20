use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use rand::{rngs::OsRng, Rng};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use zenne_core::{auth, AppContext};

use crate::ApiState;

fn gen_6_digit_otp() -> String {
    let code: u32 = OsRng.gen_range(100_000..=999_999);
    code.to_string()
}

fn gen_backup_codes() -> Vec<String> {
    (0..8)
        .map(|_| {
            let a: u16 = OsRng.gen_range(1000..=9999);
            let b: u16 = OsRng.gen_range(1000..=9999);
            format!("{}-{}", a, b)
        })
        .collect()
}

fn is_expired(exp_str: &str) -> bool {
    let trimmed = exp_str.trim();
    if trimmed.is_empty() {
        return true;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return chrono::Utc::now() > dt;
    }
    if let Ok(ts) = trimmed.parse::<f64>() {
        return (chrono::Utc::now().timestamp() as f64) > ts;
    }
    true
}

pub async fn verify_totp_or_backup(
    db: &zenne_core::db::DbManager,
    uid: i64,
    secret_opt: Option<&str>,
    code: &str,
) -> bool {
    let c = code.trim();
    if c.is_empty() {
        return false;
    }
    if let Some(sec) = secret_opt {
        if auth::verify_totp_code(sec, c) {
            return true;
        }
    }
    let Ok(conn) = db.conn() else {
        return false;
    };
    if let Ok(mut rows) = conn
        .query("SELECT backup_codes FROM web_users WHERE id = ?1", [uid])
        .await
    {
        if let Ok(Some(r)) = rows.next().await {
            let raw: Option<String> = r.get(0).ok();
            if let Some(ref json_str) = raw {
                if let Ok(mut codes) = serde_json::from_str::<Vec<String>>(json_str) {
                    let norm = c.replace(['-', ' '], "").to_uppercase();
                    let h_raw = auth::hash_token(&c.to_uppercase());
                    let h_norm = auth::hash_token(&norm);
                    if let Some(pos) = codes.iter().position(|x| {
                        let x_is_hash = x.len() == 64 && x.bytes().all(|b| b.is_ascii_hexdigit());
                        if x_is_hash {
                            auth::constant_time_eq(x.as_bytes(), h_raw.as_bytes())
                                || auth::constant_time_eq(x.as_bytes(), h_norm.as_bytes())
                        } else {
                            let x_norm = x.replace(['-', ' '], "").to_uppercase();
                            auth::constant_time_eq(x_norm.as_bytes(), norm.as_bytes())
                        }
                    }) {
                        codes.remove(pos);
                        let updated = serde_json::to_string(&codes).unwrap_or_else(|_| "[]".into());
                        let _ = conn
                            .execute(
                                "UPDATE web_users SET backup_codes = ?1 WHERE id = ?2",
                                (updated.as_str(), uid),
                            )
                            .await;
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub async fn send_smtp_email(
    ctx: &AppContext,
    to_email: &str,
    subject: &str,
    body_text: &str,
) -> Result<bool, String> {
    let host = ctx.config.smtp_host.trim();
    let user = ctx.config.smtp_user.trim();
    let pass = ctx.config.smtp_pass.trim();
    if host.is_empty() || user.is_empty() || pass.is_empty() {
        return Ok(false);
    }

    let from_addr = if ctx.config.smtp_from.trim().is_empty() {
        user.to_string()
    } else {
        ctx.config.smtp_from.trim().to_string()
    };

    let email = lettre::Message::builder()
        .from(
            from_addr
                .parse()
                .map_err(|e| format!("Invalid from: {}", e))?,
        )
        .to(to_email.parse().map_err(|e| format!("Invalid to: {}", e))?)
        .subject(subject)
        .body(body_text.to_string())
        .map_err(|e| format!("Build email err: {}", e))?;

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        user.to_string(),
        pass.to_string(),
    );

    let mailer = if ctx.config.smtp_port == 465 {
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::relay(host)
            .map_err(|e| format!("SMTP relay err: {}", e))?
            .port(465)
            .credentials(creds)
            .build()
    } else {
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay(host)
            .map_err(|e| format!("SMTP starttls err: {}", e))?
            .port(ctx.config.smtp_port)
            .credentials(creds)
            .build()
    };

    use lettre::AsyncTransport;
    mailer
        .send(email)
        .await
        .map_err(|e| format!("Send email err: {}", e))?;
    Ok(true)
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub totp_code: Option<String>,
}

pub async fn handle_login(st: &Arc<ApiState>, body: LoginBody) -> axum::response::Response {
    let ident = body
        .username
        .or(body.email)
        .unwrap_or_default()
        .trim()
        .to_string();
    if ident.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Thiếu tài khoản hoặc email"})),
        )
            .into_response();
    }

    if st
        .ctx
        .cache
        .rate_limited(&format!("login:{}", ident.to_lowercase()), 20, 60)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Thao tác quá nhanh, vui lòng thử lại sau."})),
        )
            .into_response();
    }

    let Some(user) = st.ctx.db.find_user_by_username_or_email(&ident).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sai tài khoản hoặc mật khẩu"})),
        )
            .into_response();
    };

    let ok = user
        .password_hash
        .as_deref()
        .map(|h| auth::verify_password(&body.password, h))
        .unwrap_or(false);
    if !ok {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Sai tài khoản hoặc mật khẩu"})),
        )
            .into_response();
    }

    // Upgrade legacy Python PBKDF2-HMAC-SHA256 hash to bcrypt on successful login
    if let Some(ref stored) = user.password_hash {
        if auth::is_legacy_pbkdf2_hash(stored) {
            if let Ok(new_bcrypt) = auth::hash_password(&body.password) {
                if let Ok(conn) = st.ctx.db.conn() {
                    let _ = conn
                        .execute(
                            "UPDATE web_users SET password_hash = ?1 WHERE id = ?2",
                            (new_bcrypt.as_str(), user.id),
                        )
                        .await;
                }
            }
        }
    }

    if user.email_verified == 0 {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "EmailNotVerified", "email": user.email })),
        )
            .into_response();
    }

    if user.totp_enabled == 1 {
        if let Some(code) = body
            .totp_code
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if !verify_totp_or_backup(&st.ctx.db, user.id, user.totp_secret.as_deref(), code).await
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Mã 2FA không đúng"})),
                )
                    .into_response();
            }
        } else {
            let pre_auth_token =
                auth::sign_2fa_challenge(&st.ctx.config.web_token_secret, user.id, &user.username);
            return Json(json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "is_admin": user.is_admin,
                "totp_enabled": true,
                "requires_2fa": true,
                "pre_auth_token": pre_auth_token,
                "user": {
                    "id": user.id,
                    "username": user.username,
                    "email": user.email,
                    "is_admin": user.is_admin,
                    "totp_enabled": user.totp_enabled,
                }
            }))
            .into_response();
        }
    }

    let token = auth::sign_web_token(
        &st.ctx.config.web_token_secret,
        user.id,
        &user.username,
        86400 * 7,
    );
    Json(json!({
        "id": user.id,
        "username": user.username,
        "email": user.email,
        "is_admin": user.is_admin,
        "totp_enabled": user.totp_enabled != 0,
        "requires_2fa": false,
        "token": token,
        "user": {
            "id": user.id,
            "username": user.username,
            "email": user.email,
            "is_admin": user.is_admin,
            "totp_enabled": user.totp_enabled,
        }
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub username: String,
    pub email: String,
    pub password: String,
}

pub async fn handle_register(st: &Arc<ApiState>, body: RegisterBody) -> axum::response::Response {
    let uname = body.username.trim();
    let email = body.email.trim();
    if st
        .ctx
        .cache
        .rate_limited(&format!("reg:{}", email.to_lowercase()), 5, 300)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Bạn gửi yêu cầu quá nhanh, vui lòng thử lại sau."})),
        )
            .into_response();
    }

    let Ok(hash) = auth::hash_password(&body.password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Hash error"})),
        )
            .into_response();
    };
    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };

    let smtp_ready = !st.ctx.config.smtp_host.trim().is_empty()
        && !st.ctx.config.smtp_user.trim().is_empty()
        && !st.ctx.config.smtp_pass.trim().is_empty();
    let verified_flag: i64 = if smtp_ready { 0 } else { 1 };

    if let Err(e) = conn
        .execute(
            "INSERT INTO web_users (username, email, password_hash, email_verified) VALUES (?1, ?2, ?3, ?4)",
            (uname, email, hash.as_str(), verified_flag),
        )
        .await
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Tài khoản hoặc email đã tồn tại: {}", e)})),
        )
            .into_response();
    }

    let Some(u) = st.ctx.db.find_user_by_username_or_email(uname).await else {
        return Json(json!({ "ok": true })).into_response();
    };

    if smtp_ready {
        let otp = gen_6_digit_otp();
        let otp_hash = auth::hash_token(&otp);
        let exp = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
        let _ = conn
            .execute(
                "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE user_id = ?1 AND type IN ('registration_email_code', 'register_verify') AND used_at IS NULL",
                [u.id],
            )
            .await;
        let _ = conn
            .execute(
                "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'registration_email_code', ?3, ?4)",
                (u.id, otp_hash.as_str(), email, exp.as_str()),
            )
            .await;
        let _ = send_smtp_email(
            &st.ctx,
            email,
            "Mã xác thực đăng ký Zenne Nối Từ",
            &format!(
                "Xin chào {},\n\nMã OTP xác thực tài khoản của bạn là: {}\nMã có hiệu lực trong 10 phút.",
                uname, otp
            ),
        )
        .await;
        return Json(json!({
            "ok": true,
            "requires_verification": true,
            "expires_in": 600,
            "email": email,
        }))
        .into_response();
    }

    let token = auth::sign_web_token(
        &st.ctx.config.web_token_secret,
        u.id,
        &u.username,
        86400 * 7,
    );
    Json(json!({
        "ok": true,
        "token": token,
        "expires_in": 600,
        "user": { "id": u.id, "username": u.username, "email": u.email }
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct EmailVerifyBody {
    pub email: String,
    pub code: String,
}

pub async fn handle_register_email_verify(
    st: &Arc<ApiState>,
    body: EmailVerifyBody,
) -> axum::response::Response {
    let email = body.email.trim();
    if st
        .ctx
        .cache
        .rate_limited(&format!("reg_verify:{}", email.to_lowercase()), 10, 300)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Thử quá nhiều lần, vui lòng đợi ít phút."})),
        )
            .into_response();
    }

    let Some(u) = st.ctx.db.find_user_by_username_or_email(email).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Không tìm thấy tài khoản"})),
        )
            .into_response();
    };

    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };

    if u.email_verified == 1 {
        return Json(json!({ "ok": true, "already_verified": true })).into_response();
    }

    let mut rows = match conn
        .query(
            "SELECT id, token_hash, expires_at, attempts FROM password_recovery WHERE user_id = ?1 AND type IN ('registration_email_code', 'register_verify') AND used_at IS NULL ORDER BY id DESC LIMIT 1",
            [u.id],
        )
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Mã xác thực không hợp lệ"})),
            )
                .into_response()
        }
    };

    let Some(r) = rows.next().await.ok().flatten() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã OTP không đúng hoặc đã sử dụng"})),
        )
            .into_response();
    };

    let rec_id: i64 = r.get(0).unwrap_or(0);
    let stored_hash: String = r.get(1).unwrap_or_default();
    let exp_str: String = r.get(2).unwrap_or_default();
    let attempts: i64 = r.get(3).unwrap_or(0);

    if is_expired(&exp_str) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã OTP đã hết hạn"})),
        )
            .into_response();
    }
    if attempts >= 5 {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Bạn đã nhập sai quá 5 lần. Vui lòng gửi lại mã mới."})),
        )
            .into_response();
    }

    let code_hash = auth::hash_token(body.code.trim());
    if !auth::constant_time_eq(stored_hash.as_bytes(), code_hash.as_bytes()) {
        let _ = conn
            .execute(
                "UPDATE password_recovery SET attempts = attempts + 1 WHERE id = ?1 AND used_at IS NULL AND attempts < 5",
                [rec_id],
            )
            .await;
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã OTP không chính xác"})),
        )
            .into_response();
    }

    let consumed = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL AND attempts < 5",
            [rec_id],
        )
        .await
        .unwrap_or(0);
    if consumed != 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã OTP đã được sử dụng"})),
        )
            .into_response();
    }

    let _ = conn
        .execute(
            "UPDATE web_users SET email_verified = 1 WHERE id = ?1",
            [u.id],
        )
        .await;

    let token = auth::sign_web_token(
        &st.ctx.config.web_token_secret,
        u.id,
        &u.username,
        86400 * 7,
    );
    Json(json!({ "ok": true, "token": token })).into_response()
}

#[derive(Deserialize)]
pub struct EmailResendBody {
    pub email: String,
}

pub async fn handle_register_email_resend(
    st: &Arc<ApiState>,
    body: EmailResendBody,
) -> axum::response::Response {
    let email = body.email.trim();
    if st
        .ctx
        .cache
        .rate_limited(&format!("reg_resend:{}", email.to_lowercase()), 1, 60)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Vui lòng đợi 60 giây trước khi gửi lại mã."})),
        )
            .into_response();
    }

    let Some(u) = st.ctx.db.find_user_by_username_or_email(email).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Không tìm thấy tài khoản"})),
        )
            .into_response();
    };

    let otp = gen_6_digit_otp();
    let otp_hash = auth::hash_token(&otp);
    let exp = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    if let Ok(conn) = st.ctx.db.conn() {
        let _ = conn
            .execute(
                "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE user_id = ?1 AND type IN ('registration_email_code', 'register_verify') AND used_at IS NULL",
                [u.id],
            )
            .await;
        let _ = conn
            .execute(
                "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'registration_email_code', ?3, ?4)",
                (u.id, otp_hash.as_str(), u.email.as_str(), exp.as_str()),
            )
            .await;
    }
    let _ = send_smtp_email(
        &st.ctx,
        &u.email,
        "Gửi lại mã xác thực Zenne Nối Từ",
        &format!("Mã OTP mới của bạn là: {} (hiệu lực 10 phút)", otp),
    )
    .await;

    Json(json!({ "ok": true, "expires_in": 600 })).into_response()
}

#[derive(Deserialize)]
pub struct ChangePwBody {
    pub user_id: Option<i64>,
    #[serde(alias = "current_password")]
    pub old_password: Option<String>,
    pub new_password: String,
}

pub async fn handle_change_password(
    _headers: &HeaderMap,
    st: &Arc<ApiState>,
    body: ChangePwBody,
    uid: i64,
) -> axum::response::Response {
    if body.new_password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mật khẩu mới tối thiểu 6 ký tự"})),
        )
            .into_response();
    }
    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };

    if let Ok(mut rows) = conn
        .query(
            "SELECT password_hash FROM web_users WHERE id = ?1 LIMIT 1",
            [uid],
        )
        .await
    {
        if let Ok(Some(r)) = rows.next().await {
            let existing: Option<String> = r.get(0).ok();
            if let Some(ref existing_hash) = existing {
                if !existing_hash.is_empty() {
                    let old_pw = body.old_password.as_deref().unwrap_or("");
                    if !auth::verify_password(old_pw, existing_hash) {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": "Mật khẩu hiện tại không đúng"})),
                        )
                            .into_response();
                    }
                }
            }
        }
    }

    let Ok(hash) = auth::hash_password(&body.new_password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Hash error"})),
        )
            .into_response();
    };
    let changed_at = chrono::Utc::now().timestamp();
    let _ = conn
        .execute(
            "UPDATE web_users SET password_hash = ?1, password_changed_at = ?2 WHERE id = ?3",
            (hash.as_str(), changed_at, uid),
        )
        .await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct TotpCodeBody {
    pub user_id: Option<i64>,
    pub code: Option<String>,
    pub password: Option<String>,
}

pub async fn handle_toggle_2fa(
    st: &Arc<ApiState>,
    uid: i64,
    enable: bool,
    body: Option<TotpCodeBody>,
) -> axum::response::Response {
    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };

    let (code_opt, _pw_opt) = match body {
        Some(b) => (b.code, b.password),
        None => (None, None),
    };
    let code = code_opt.unwrap_or_default().trim().to_string();

    let mut totp_secret: Option<String> = None;
    if let Ok(mut rows) = conn
        .query(
            "SELECT totp_secret FROM web_users WHERE id = ?1 LIMIT 1",
            [uid],
        )
        .await
    {
        if let Ok(Some(r)) = rows.next().await {
            totp_secret = r.get(0).ok();
        }
    }

    if enable {
        let Some(ref sec) = totp_secret else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Chưa khởi tạo mã 2FA. Hãy gọi setup-2fa trước."})),
            )
                .into_response();
        };
        if code.is_empty() || !auth::verify_totp_code(sec, &code) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Mã OTP 2FA không chính xác"})),
            )
                .into_response();
        }
        let plain_backups = gen_backup_codes();
        let hashed_backups: Vec<String> =
            plain_backups.iter().map(|c| auth::hash_token(c)).collect();
        let backups_json = serde_json::to_string(&hashed_backups).unwrap_or_else(|_| "[]".into());
        let _ = conn
            .execute(
                "UPDATE web_users SET totp_enabled = 1, backup_codes = ?1 WHERE id = ?2",
                (backups_json.as_str(), uid),
            )
            .await;
        return Json(json!({ "ok": true, "backup_codes": plain_backups })).into_response();
    }

    // Disable 2FA: require valid TOTP/backup code (or password only when 2FA is not yet enabled)
    let code_ok = !code.is_empty()
        && verify_totp_or_backup(&st.ctx.db, uid, totp_secret.as_deref(), &code).await;
    if !code_ok {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Vui lòng nhập mã 2FA hoặc mã dự phòng hợp lệ để tắt 2FA."})),
        )
            .into_response();
    }

    let _ = conn
        .execute(
            "UPDATE web_users SET totp_enabled = 0, totp_secret = NULL, backup_codes = NULL WHERE id = ?1",
            [uid],
        )
        .await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct RecoveryRequestBody {
    pub identifier: Option<String>,
    pub email: Option<String>,
}

pub async fn handle_recovery_request(
    st: &Arc<ApiState>,
    body: RecoveryRequestBody,
) -> axum::response::Response {
    let ident = body
        .identifier
        .or(body.email)
        .unwrap_or_default()
        .trim()
        .to_string();
    if ident.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Thiếu email hoặc tên tài khoản"})),
        )
            .into_response();
    }

    if st
        .ctx
        .cache
        .rate_limited(&format!("rec_req:{}", ident.to_lowercase()), 5, 300)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Yêu cầu khôi phục quá nhanh, vui lòng đợi ít phút."})),
        )
            .into_response();
    }

    if let Some(u) = st.ctx.db.find_user_by_username_or_email(&ident).await {
        let otp = gen_6_digit_otp();
        let otp_hash = auth::hash_token(&otp);
        let exp = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
        if let Ok(conn) = st.ctx.db.conn() {
            let _ = conn
                .execute(
                    "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE user_id = ?1 AND type IN ('email_code', 'email_otp') AND used_at IS NULL",
                    [u.id],
                )
                .await;
            let _ = conn
                .execute(
                    "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'email_code', ?3, ?4)",
                    (u.id, otp_hash.as_str(), u.email.as_str(), exp.as_str()),
                )
                .await;
        }
        let _ = send_smtp_email(
            &st.ctx,
            &u.email,
            "Khôi phục mật khẩu Zenne Nối Từ",
            &format!(
                "Xin chào {},\n\nMã OTP khôi phục mật khẩu của bạn là: {}\nMã có hiệu lực trong 10 phút.",
                u.username, otp
            ),
        )
        .await;
    }
    Json(json!({ "ok": true, "success": true, "expires_in": 600 })).into_response()
}

#[derive(Deserialize)]
pub struct RecoveryEmailVerifyBody {
    pub email: Option<String>,
    pub identifier: Option<String>,
    pub code: String,
}

pub async fn handle_recovery_email_verify(
    st: &Arc<ApiState>,
    body: RecoveryEmailVerifyBody,
) -> axum::response::Response {
    let ident = body
        .email
        .or(body.identifier)
        .unwrap_or_default()
        .trim()
        .to_string();
    if st
        .ctx
        .cache
        .rate_limited(&format!("rec_ev:{}", ident.to_lowercase()), 10, 300)
        .await
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Thử quá nhiều lần, vui lòng đợi ít phút."})),
        )
            .into_response();
    }

    let Some(u) = st.ctx.db.find_user_by_username_or_email(&ident).await else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Tài khoản không tồn tại"})),
        )
            .into_response();
    };
    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };

    let mut rows = match conn
        .query(
            "SELECT id, token_hash, expires_at, attempts FROM password_recovery WHERE user_id = ?1 AND type IN ('email_code', 'email_otp') AND used_at IS NULL ORDER BY id DESC LIMIT 1",
            [u.id],
        )
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Mã không hợp lệ"})),
            )
                .into_response()
        }
    };
    let Some(r) = rows.next().await.ok().flatten() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Không tìm thấy mã xác thực hoặc mã đã sử dụng"})),
        )
            .into_response();
    };

    let rec_id: i64 = r.get(0).unwrap_or(0);
    let stored_hash: String = r.get(1).unwrap_or_default();
    let exp_str: String = r.get(2).unwrap_or_default();
    let attempts: i64 = r.get(3).unwrap_or(0);

    if is_expired(&exp_str) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã xác thực đã hết hạn"})),
        )
            .into_response();
    }
    if attempts >= 5 {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Nhập sai quá 5 lần, vui lòng yêu cầu mã mới"})),
        )
            .into_response();
    }

    let code_hash = auth::hash_token(body.code.trim());
    if !auth::constant_time_eq(stored_hash.as_bytes(), code_hash.as_bytes()) {
        let _ = conn
            .execute(
                "UPDATE password_recovery SET attempts = attempts + 1 WHERE id = ?1 AND used_at IS NULL AND attempts < 5",
                [rec_id],
            )
            .await;
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã OTP không đúng"})),
        )
            .into_response();
    }

    let consumed = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL AND attempts < 5",
            [rec_id],
        )
        .await
        .unwrap_or(0);
    if consumed != 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Mã OTP đã được sử dụng"})),
        )
            .into_response();
    }

    let rec_token = format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    );
    let rec_hash = auth::hash_token(&rec_token);
    let exp = (chrono::Utc::now() + chrono::Duration::minutes(8)).to_rfc3339();
    let _ = conn
        .execute(
            "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'recovery_token', ?3, ?4)",
            (u.id, rec_hash.as_str(), u.email.as_str(), exp.as_str()),
        )
        .await;

    Json(json!({
        "ok": true,
        "success": true,
        "recovery_token": rec_token,
        "reset_token": rec_token
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct RecoveryTokenVerifyBody {
    pub token: String,
}

pub async fn handle_recovery_token_verify(
    st: &Arc<ApiState>,
    body: RecoveryTokenVerifyBody,
) -> axum::response::Response {
    let tok = body.token.trim();
    if tok.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Thiếu token khôi phục"})),
        )
            .into_response();
    }
    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };
    let tok_hash = auth::hash_token(tok);
    let mut rows = match conn
        .query(
            "SELECT id, user_id, source, expires_at FROM password_recovery WHERE token_hash = ?1 AND type IN ('recovery_token', 'reset_token') AND used_at IS NULL ORDER BY id DESC LIMIT 1",
            [tok_hash.as_str()],
        )
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Token không hợp lệ"})),
            )
                .into_response()
        }
    };
    let Some(r) = rows.next().await.ok().flatten() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Token khôi phục không hợp lệ hoặc đã dùng"})),
        )
            .into_response();
    };

    let rec_id: i64 = r.get(0).unwrap_or(0);
    let uid: i64 = r.get(1).unwrap_or(0);
    let source: String = r.get(2).unwrap_or_default();
    let exp_str: String = r.get(3).unwrap_or_default();
    if is_expired(&exp_str) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Token khôi phục đã hết hạn"})),
        )
            .into_response();
    }

    let consumed = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL",
            [rec_id],
        )
        .await
        .unwrap_or(0);
    if consumed != 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Token khôi phục đã được sử dụng"})),
        )
            .into_response();
    }

    let reset_session = format!(
        "{}{}",
        uuid::Uuid::new_v4().as_simple(),
        uuid::Uuid::new_v4().as_simple()
    );
    let sess_hash = auth::hash_token(&reset_session);
    let exp = (chrono::Utc::now() + chrono::Duration::minutes(8)).to_rfc3339();
    let _ = conn
        .execute(
            "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'reset_session', ?3, ?4)",
            (uid, sess_hash.as_str(), source.as_str(), exp.as_str()),
        )
        .await;

    Json(json!({
        "ok": true,
        "success": true,
        "reset_session": reset_session
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct RecoveryStatusBody {
    pub session: Option<String>,
}

pub async fn handle_recovery_status(
    st: &Arc<ApiState>,
    body: Option<RecoveryStatusBody>,
) -> axum::response::Response {
    let sess = body
        .and_then(|b| b.session)
        .unwrap_or_default()
        .trim()
        .to_string();
    if sess.is_empty() {
        return Json(json!({ "canResetPassword": false, "totpEnabled": false })).into_response();
    }
    let Ok(conn) = st.ctx.db.conn() else {
        return Json(json!({ "canResetPassword": false, "totpEnabled": false })).into_response();
    };
    let sess_hash = auth::hash_token(&sess);
    if let Ok(mut rows) = conn
        .query(
            "SELECT pr.expires_at, wu.totp_enabled
             FROM password_recovery pr
             JOIN web_users wu ON wu.id = pr.user_id
             WHERE pr.token_hash = ?1 AND pr.type IN ('reset_session', 'reset_token') AND pr.used_at IS NULL
             ORDER BY pr.id DESC LIMIT 1",
            [sess_hash.as_str()],
        )
        .await
    {
        if let Ok(Some(r)) = rows.next().await {
            let exp_str: String = r.get(0).unwrap_or_default();
            let totp: i64 = r.get(1).unwrap_or(0);
            if !is_expired(&exp_str) {
                return Json(json!({
                    "canResetPassword": true,
                    "totpEnabled": totp != 0
                }))
                .into_response();
            }
        }
    }
    Json(json!({ "canResetPassword": false, "totpEnabled": false })).into_response()
}

#[derive(Deserialize)]
pub struct RecoveryResetBody {
    pub session: Option<String>,
    pub reset_token: Option<String>,
    pub token: Option<String>,
    #[serde(alias = "password")]
    pub new_password: String,
    pub totp_code: Option<String>,
}

pub async fn handle_recovery_reset_password(
    st: &Arc<ApiState>,
    body: RecoveryResetBody,
) -> axum::response::Response {
    let tok = body
        .session
        .or(body.reset_token)
        .or(body.token)
        .unwrap_or_default()
        .trim()
        .to_string();
    if tok.is_empty() || body.new_password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phiên khôi phục hoặc mật khẩu mới không hợp lệ"})),
        )
            .into_response();
    }
    let Ok(conn) = st.ctx.db.conn() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "DB error"})),
        )
            .into_response();
    };
    let tok_hash = auth::hash_token(&tok);
    let mut rows = match conn
        .query(
            "SELECT pr.id, pr.user_id, pr.expires_at, wu.totp_enabled, wu.totp_secret
             FROM password_recovery pr
             JOIN web_users wu ON wu.id = pr.user_id
             WHERE pr.token_hash = ?1 AND pr.type IN ('reset_session', 'reset_token') AND pr.used_at IS NULL
             ORDER BY pr.id DESC LIMIT 1",
            [tok_hash.as_str()],
        )
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Phiên khôi phục không hợp lệ"})),
            )
                .into_response()
        }
    };
    let Some(r) = rows.next().await.ok().flatten() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phiên khôi phục không hợp lệ hoặc đã dùng"})),
        )
            .into_response();
    };
    let rec_id: i64 = r.get(0).unwrap_or(0);
    let uid: i64 = r.get(1).unwrap_or(0);
    let exp_str: String = r.get(2).unwrap_or_default();
    let totp_enabled: i64 = r.get(3).unwrap_or(0);
    let totp_secret: Option<String> = r.get(4).ok();

    if is_expired(&exp_str) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phiên khôi phục đã hết hạn"})),
        )
            .into_response();
    }

    if totp_enabled == 1 {
        let rl_key = format!("rec_reset_2fa:{}", uid);
        if st.ctx.cache.rate_limited(&rl_key, 5, 300).await {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "needs2FA": true, "error": "Thử sai 2FA quá nhiều lần, vui lòng thử lại sau." })),
            )
                .into_response();
        }
        let code = body.totp_code.as_deref().unwrap_or("").trim();
        if code.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "needs2FA": true, "error": "Cần mã xác thực 2FA" })),
            )
                .into_response();
        }
        if !verify_totp_or_backup(&st.ctx.db, uid, totp_secret.as_deref(), code).await {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "needs2FA": true, "error": "Mã 2FA không đúng" })),
            )
                .into_response();
        }
    }

    let Ok(new_hash) = auth::hash_password(&body.new_password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Hash error"})),
        )
            .into_response();
    };

    // Consume reset session atomically BEFORE updating password_hash
    let consumed = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL",
            [rec_id],
        )
        .await
        .unwrap_or(0);
    if consumed != 1 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phiên khôi phục đã được sử dụng"})),
        )
            .into_response();
    }

    let changed_at = chrono::Utc::now().timestamp();
    if let Err(e) = conn
        .execute(
            "UPDATE web_users SET password_hash = ?1, password_changed_at = ?2 WHERE id = ?3",
            (new_hash.as_str(), changed_at, uid),
        )
        .await
    {
        tracing::error!("Failed updating password_hash for uid {}: {}", uid, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Không thể cập nhật mật khẩu, vui lòng thử lại."})),
        )
            .into_response();
    }
    let _ = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE user_id = ?1 AND used_at IS NULL",
            [uid],
        )
        .await;

    Json(json!({ "ok": true, "success": true })).into_response()
}

/// Verify that a web session token was not issued prior to the user's last password change/reset.
pub async fn is_web_token_valid_for_user(
    db: &zenne_core::db::DbManager,
    uid: i64,
    claims: &serde_json::Value,
) -> bool {
    let Ok(conn) = db.conn() else {
        return false;
    };
    if let Ok(mut rows) = conn
        .query(
            "SELECT COALESCE(password_changed_at, 0) FROM web_users WHERE id = ?1 LIMIT 1",
            [uid],
        )
        .await
    {
        if let Ok(Some(r)) = rows.next().await {
            let changed_at = r.get::<i64>(0).unwrap_or(0);
            if changed_at > 0 {
                let iat = claims["iat"].as_i64().unwrap_or(0);
                return iat >= changed_at;
            }
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_token_expiry_8_minutes() {
        let valid_exp = (chrono::Utc::now() + chrono::Duration::minutes(8)).to_rfc3339();
        assert!(!is_expired(&valid_exp));

        let expired_exp = (chrono::Utc::now() - chrono::Duration::minutes(9)).to_rfc3339();
        assert!(is_expired(&expired_exp));
        assert!(is_expired("invalid-timestamp"));
    }
}
