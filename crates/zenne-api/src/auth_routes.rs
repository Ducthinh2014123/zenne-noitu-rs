use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use zenne_core::{auth, AppContext};

use crate::ApiState;

fn gen_6_digit_otp() -> String {
    let code: u32 = rand::thread_rng().gen_range(100_000..=999_999);
    code.to_string()
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
        .from(from_addr.parse().map_err(|e| format!("Invalid from: {}", e))?)
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
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

pub async fn handle_login(st: &Arc<ApiState>, body: LoginBody) -> axum::response::Response {
    let Some(user) = st
        .ctx
        .db
        .find_user_by_username_or_email(&body.username)
        .await
    else {
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

    if user.totp_enabled == 1 {
        match body.totp_code.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => {
                return Json(json!({
                    "requires_2fa": true,
                    "username": user.username,
                    "message": "Vui lòng nhập mã xác thực 2FA"
                }))
                .into_response();
            }
            Some(code) => {
                let valid = user
                    .totp_secret
                    .as_deref()
                    .map(|s| auth::verify_totp_code(s, code))
                    .unwrap_or(false);
                if !valid {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"error": "Mã 2FA không đúng"})),
                    )
                        .into_response();
                }
            }
        }
    }

    let token = auth::sign_web_token(
        &st.ctx.config.web_token_secret,
        user.id,
        &user.username,
        86400 * 7,
    );
    Json(json!({
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
                "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'register_verify', ?3, ?4)",
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

    let token = auth::sign_web_token(&st.ctx.config.web_token_secret, u.id, &u.username, 86400 * 7);
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
    let Some(u) = st
        .ctx
        .db
        .find_user_by_username_or_email(body.email.trim())
        .await
    else {
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
        let token = auth::sign_web_token(&st.ctx.config.web_token_secret, u.id, &u.username, 86400 * 7);
        return Json(json!({ "ok": true, "token": token })).into_response();
    }

    let code_hash = auth::hash_token(body.code.trim());
    let mut rows = match conn
        .query(
            "SELECT id, expires_at FROM password_recovery WHERE user_id = ?1 AND type = 'register_verify' AND token_hash = ?2 AND used_at IS NULL ORDER BY id DESC LIMIT 1",
            (u.id, code_hash.as_str()),
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
    let exp_str: String = r.get(1).unwrap_or_default();
    if let Ok(exp_dt) = chrono::DateTime::parse_from_rfc3339(&exp_str) {
        if chrono::Utc::now() > exp_dt {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Mã OTP đã hết hạn"})),
            )
                .into_response();
        }
    }

    let _ = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1",
            [rec_id],
        )
        .await;
    let _ = conn
        .execute("UPDATE web_users SET email_verified = 1 WHERE id = ?1", [u.id])
        .await;

    let token = auth::sign_web_token(&st.ctx.config.web_token_secret, u.id, &u.username, 86400 * 7);
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
    let Some(u) = st
        .ctx
        .db
        .find_user_by_username_or_email(body.email.trim())
        .await
    else {
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
                "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'register_verify', ?3, ?4)",
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
    pub old_password: Option<String>,
    pub new_password: String,
}

pub async fn handle_change_password(
    headers: &HeaderMap,
    st: &Arc<ApiState>,
    body: ChangePwBody,
    uid: i64,
) -> axum::response::Response {
    let _ = headers;
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
    let _ = conn
        .execute(
            "UPDATE web_users SET password_hash = ?1 WHERE id = ?2",
            (hash.as_str(), uid),
        )
        .await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct TotpCodeBody {
    pub code: Option<String>,
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

    let code = body.and_then(|b| b.code).unwrap_or_default();
    if !code.trim().is_empty() {
        if let Ok(mut rows) = conn
            .query("SELECT totp_secret FROM web_users WHERE id = ?1", [uid])
            .await
        {
            if let Ok(Some(r)) = rows.next().await {
                let sec: Option<String> = r.get(0).ok();
                if let Some(s) = sec {
                    if !auth::verify_totp_code(&s, code.trim()) {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": "Mã 2FA không chính xác"})),
                        )
                            .into_response();
                    }
                }
            }
        }
    }

    if enable {
        let _ = conn
            .execute("UPDATE web_users SET totp_enabled = 1 WHERE id = ?1", [uid])
            .await;
    } else {
        let _ = conn
            .execute(
                "UPDATE web_users SET totp_enabled = 0, totp_secret = NULL WHERE id = ?1",
                [uid],
            )
            .await;
    }
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

    if let Some(u) = st.ctx.db.find_user_by_username_or_email(&ident).await {
        let otp = gen_6_digit_otp();
        let otp_hash = auth::hash_token(&otp);
        let exp = (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339();
        if let Ok(conn) = st.ctx.db.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'email_otp', ?3, ?4)",
                    (u.id, otp_hash.as_str(), u.email.as_str(), exp.as_str()),
                )
                .await;
        }
        let _ = send_smtp_email(
            &st.ctx,
            &u.email,
            "Khôi phục mật khẩu Zenne Nối Từ",
            &format!(
                "Xin chào {},\n\nMã OTP khôi phục mật khẩu của bạn là: {}\nMã có hiệu lực trong 15 phút.",
                u.username, otp
            ),
        )
        .await;
    }
    Json(json!({ "ok": true, "expires_in": 900 })).into_response()
}

#[derive(Deserialize)]
pub struct RecoveryVerifyBody {
    pub email: Option<String>,
    pub identifier: Option<String>,
    pub code: Option<String>,
    pub token: Option<String>,
}

pub async fn handle_recovery_verify(
    st: &Arc<ApiState>,
    body: RecoveryVerifyBody,
) -> axum::response::Response {
    let ident = body
        .email
        .or(body.identifier)
        .unwrap_or_default()
        .trim()
        .to_string();
    let code = body.code.or(body.token).unwrap_or_default().trim().to_string();
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

    let code_hash = auth::hash_token(&code);
    let mut rows = match conn
        .query(
            "SELECT id FROM password_recovery WHERE user_id = ?1 AND token_hash = ?2 AND used_at IS NULL ORDER BY id DESC LIMIT 1",
            (u.id, code_hash.as_str()),
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
            Json(json!({"error": "Mã xác thực không đúng hoặc đã hết hạn"})),
        )
            .into_response();
    };
    let rec_id: i64 = r.get(0).unwrap_or(0);
    let _ = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1",
            [rec_id],
        )
        .await;

    let reset_token = uuid::Uuid::new_v4().to_string();
    let reset_hash = auth::hash_token(&reset_token);
    let exp = (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339();
    let _ = conn
        .execute(
            "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at) VALUES (?1, ?2, 'reset_token', ?3, ?4)",
            (u.id, reset_hash.as_str(), u.email.as_str(), exp.as_str()),
        )
        .await;

    Json(json!({ "ok": true, "reset_token": reset_token })).into_response()
}

#[derive(Deserialize)]
pub struct RecoveryResetBody {
    pub reset_token: Option<String>,
    pub token: Option<String>,
    pub new_password: String,
}

pub async fn handle_recovery_reset_password(
    st: &Arc<ApiState>,
    body: RecoveryResetBody,
) -> axum::response::Response {
    let tok = body
        .reset_token
        .or(body.token)
        .unwrap_or_default()
        .trim()
        .to_string();
    if tok.is_empty() || body.new_password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Token hoặc mật khẩu mới không hợp lệ"})),
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
            "SELECT id, user_id FROM password_recovery WHERE token_hash = ?1 AND type = 'reset_token' AND used_at IS NULL ORDER BY id DESC LIMIT 1",
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
    let Ok(new_hash) = auth::hash_password(&body.new_password) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Hash error"})),
        )
            .into_response();
    };
    let _ = conn
        .execute(
            "UPDATE web_users SET password_hash = ?1 WHERE id = ?2",
            (new_hash.as_str(), uid),
        )
        .await;
    let _ = conn
        .execute(
            "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1",
            [rec_id],
        )
        .await;
    Json(json!({ "ok": true })).into_response()
}
