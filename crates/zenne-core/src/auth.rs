use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Secret, TOTP};

type HmacSha256 = Hmac<Sha256>;

pub fn sign_web_token(secret: &str, user_id: i64, username: &str, ttl_secs: i64) -> String {
    let exp = chrono::Utc::now().timestamp() + ttl_secs;
    let payload = serde_json::json!({
        "uid": user_id,
        "username": username,
        "exp": exp,
    })
    .to_string();
    let b64 = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
    mac.update(b64.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{}.{}", b64, sig)
}

pub fn verify_web_token(secret: &str, token: &str) -> Option<serde_json::Value> {
    let (b64, sig) = token.split_once('.')?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(b64.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if expected != sig {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(b64).ok()?;
    let val: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let exp = val["exp"].as_i64()?;
    if chrono::Utc::now().timestamp() > exp {
        return None;
    }
    Some(val)
}

pub fn sign_game_session(secret: &str, user_id: i64, game: &str) -> String {
    let exp = chrono::Utc::now().timestamp() + 3600;
    let nonce: u64 = rand::thread_rng().gen();
    let payload = format!("{}:{}:{}:{}", user_id, game, exp, nonce);
    let b64 = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
    mac.update(b64.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{}.{}", b64, sig)
}

pub fn verify_game_session(
    secret: &str,
    token: &str,
    expected_user: i64,
    expected_game: &str,
) -> bool {
    let Some((b64, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(b64.as_bytes());
    if hex::encode(mac.finalize().into_bytes()) != sig {
        return false;
    }
    let Ok(decoded) = URL_SAFE_NO_PAD.decode(b64) else {
        return false;
    };
    let Ok(text) = String::from_utf8(decoded) else {
        return false;
    };
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() != 4 {
        return false;
    }
    let uid: i64 = parts[0].parse().unwrap_or(-1);
    let exp: i64 = parts[2].parse().unwrap_or(0);
    uid == expected_user && parts[1] == expected_game && chrono::Utc::now().timestamp() <= exp
}

pub fn hash_password(pw: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(pw, 10)
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    bcrypt::verify(pw, hash).unwrap_or(false)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn gen_totp_secret() -> String {
    Secret::generate_secret().to_encoded().to_string()
}

pub fn verify_totp_code(secret_b32: &str, code: &str) -> bool {
    let Ok(secret) = Secret::Encoded(secret_b32.to_string()).to_bytes() else {
        return false;
    };
    let Ok(totp) = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        Some("ZenneNoitu".to_string()),
        "user".to_string(),
    ) else {
        return false;
    };
    totp.check_current(code.trim()).unwrap_or(false)
}

pub fn totp_uri(secret_b32: &str, username: &str) -> String {
    format!(
        "otpauth://totp/ZenneNoitu:{}?secret={}&issuer=ZenneNoitu",
        username, secret_b32
    )
}
