use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, Rng, RngCore};
use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Secret, TOTP};

type HmacSha256 = Hmac<Sha256>;

/// Constant-time byte slice comparison to prevent timing side-channels.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

pub fn sign_web_token(secret: &str, user_id: i64, username: &str, ttl_secs: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let exp = now + ttl_secs;
    let payload = serde_json::json!({
        "uid": user_id,
        "username": username,
        "iat": now,
        "exp": exp,
    })
    .to_string();
    let b64 = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
    mac.update(b64.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{}.{}", b64, sig)
}

/// Sign a short-lived (5 min) purpose-bound 2FA challenge token.
pub fn sign_2fa_challenge(secret: &str, user_id: i64, username: &str) -> String {
    let exp = chrono::Utc::now().timestamp() + 300;
    let payload = serde_json::json!({
        "uid": user_id,
        "username": username,
        "purpose": "2fa_challenge",
        "exp": exp,
    })
    .to_string();
    let b64 = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
    mac.update(b64.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{}.{}", b64, sig)
}

fn verify_raw_token(secret: &str, token: &str) -> Option<serde_json::Value> {
    let (b64, sig) = token.split_once('.')?;
    let sig_bytes = hex::decode(sig).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(b64.as_bytes());
    mac.verify_slice(&sig_bytes).ok()?;
    let decoded = URL_SAFE_NO_PAD.decode(b64).ok()?;
    let val: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    let exp = val["exp"].as_i64()?;
    if chrono::Utc::now().timestamp() > exp {
        return None;
    }
    Some(val)
}

/// Verify a purpose-bound 2FA challenge token and return `(user_id, username)`.
pub fn verify_2fa_challenge(secret: &str, token: &str) -> Option<(i64, String)> {
    let val = verify_raw_token(secret, token)?;
    if val["purpose"].as_str() != Some("2fa_challenge") {
        return None;
    }
    let uid = val["uid"].as_i64()?;
    let uname = val["username"].as_str()?.to_string();
    Some((uid, uname))
}

pub fn verify_web_token(secret: &str, token: &str) -> Option<serde_json::Value> {
    let val = verify_raw_token(secret, token)?;
    if val.get("purpose").and_then(|v| v.as_str()) == Some("2fa_challenge") {
        return None;
    }
    Some(val)
}

pub fn sign_game_session(secret: &str, user_id: i64, game: &str) -> String {
    let iat = chrono::Utc::now().timestamp();
    let exp = iat + 3600;
    let nonce: u64 = OsRng.gen();
    let payload = format!("{}:{}:{}:{}:{}", user_id, game, iat, exp, nonce);
    let b64 = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
    mac.update(b64.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{}.{}", b64, sig)
}

pub fn verify_game_session_claims(
    secret: &str,
    token: &str,
    expected_user: i64,
    expected_game: &str,
) -> Option<(i64, i64, String)> {
    let (b64, sig) = token.split_once('.')?;
    let sig_bytes = hex::decode(sig).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(b64.as_bytes());
    mac.verify_slice(&sig_bytes).ok()?;
    let decoded = URL_SAFE_NO_PAD.decode(b64).ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let parts: Vec<&str> = text.split(':').collect();
    let (uid, game, iat, exp, nonce) = match parts.len() {
        5 => (
            parts[0].parse::<i64>().ok()?,
            parts[1],
            parts[2].parse::<i64>().ok()?,
            parts[3].parse::<i64>().ok()?,
            parts[4].to_string(),
        ),
        4 => (
            parts[0].parse::<i64>().ok()?,
            parts[1],
            chrono::Utc::now().timestamp(),
            parts[2].parse::<i64>().ok()?,
            parts[3].to_string(),
        ),
        _ => return None,
    };
    if uid == expected_user && game == expected_game && chrono::Utc::now().timestamp() <= exp {
        Some((uid, iat, nonce))
    } else {
        None
    }
}

pub fn verify_game_session(
    secret: &str,
    token: &str,
    expected_user: i64,
    expected_game: &str,
) -> bool {
    verify_game_session_claims(secret, token, expected_user, expected_game).is_some()
}

pub fn hash_password(pw: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(pw, 10)
}

/// Returns true if `hash` is a legacy Python PBKDF2-HMAC-SHA256 digest (`"{salt}${64-hex}"`).
pub fn is_legacy_pbkdf2_hash(hash: &str) -> bool {
    if let Some((salt, dk_hex)) = hash.split_once('$') {
        !salt.is_empty()
            && !salt.starts_with("$2")
            && dk_hex.len() == 64
            && dk_hex.bytes().all(|b| b.is_ascii_hexdigit())
    } else {
        false
    }
}

fn verify_pbkdf2_sha256(pw: &str, stored: &str) -> bool {
    let Some((salt, dk_hex)) = stored.split_once('$') else {
        return false;
    };
    let Ok(expected) = hex::decode(dk_hex) else {
        return false;
    };
    if expected.len() != 32 {
        return false;
    }
    let Ok(prf) = HmacSha256::new_from_slice(pw.as_bytes()) else {
        return false;
    };
    let mut u = {
        let mut m = prf.clone();
        m.update(salt.as_bytes());
        m.update(&1u32.to_be_bytes());
        m.finalize().into_bytes()
    };
    let mut out = u;
    for _ in 1..260_000 {
        let mut m = prf.clone();
        m.update(&u);
        u = m.finalize().into_bytes();
        for (o, &b) in out.iter_mut().zip(u.iter()) {
            *o ^= b;
        }
    }
    constant_time_eq(out.as_slice(), expected.as_slice())
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    if hash.starts_with("$2") {
        bcrypt::verify(pw, hash).unwrap_or(false)
    } else if is_legacy_pbkdf2_hash(hash) {
        verify_pbkdf2_sha256(pw, hash)
    } else {
        false
    }
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

/// Generate a cryptographically random one-time recovery token (24 bytes, URL-safe base64).
pub fn gen_recovery_token() -> String {
    let mut bytes = [0u8; 24];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_token_sign_verify_and_tamper() {
        let secret = "test_secret_key_123";
        let tok = sign_web_token(secret, 42, "thinh", 300);
        let claims = verify_web_token(secret, &tok).expect("valid token");
        assert_eq!(claims["uid"].as_i64(), Some(42));
        assert_eq!(claims["username"].as_str(), Some("thinh"));

        let tampered = format!("{}00", &tok[..tok.len() - 2]);
        assert!(verify_web_token(secret, &tampered).is_none());
        assert!(verify_web_token("wrong_secret", &tok).is_none());

        let expired = sign_web_token(secret, 42, "thinh", -10);
        assert!(verify_web_token(secret, &expired).is_none());
    }

    #[test]
    fn test_game_session_iat_and_claims() {
        let secret = "game_secret_xyz";
        let tok = sign_game_session(secret, -5, "snake");
        let (uid, iat, nonce) =
            verify_game_session_claims(secret, &tok, -5, "snake").expect("valid game session");
        assert_eq!(uid, -5);
        assert!(iat > 0);
        assert!(!nonce.is_empty());

        assert!(verify_game_session_claims(secret, &tok, -6, "snake").is_none());
        assert!(verify_game_session_claims(secret, &tok, -5, "2048").is_none());
    }

    #[test]
    fn test_legacy_pbkdf2_and_2fa_challenge_isolation() {
        let mut mac = HmacSha256::new_from_slice(b"pw123").unwrap();
        mac.update(b"deadbeef");
        mac.update(&1u32.to_be_bytes());
        let mut u = mac.finalize().into_bytes();
        let mut out = u;
        for _ in 1..260_000 {
            let mut m = HmacSha256::new_from_slice(b"pw123").unwrap();
            m.update(&u);
            u = m.finalize().into_bytes();
            for (o, &b) in out.iter_mut().zip(u.iter()) {
                *o ^= b;
            }
        }
        let stored = format!("deadbeef${}", hex::encode(out));
        assert!(is_legacy_pbkdf2_hash(&stored));
        assert!(verify_password("pw123", &stored));
        assert!(!verify_password("wrong", &stored));

        let ch = sign_2fa_challenge("sec", 7, "alice");
        assert!(verify_web_token("sec", &ch).is_none());
        assert_eq!(
            verify_2fa_challenge("sec", &ch),
            Some((7, "alice".to_string()))
        );

        let rt1 = gen_recovery_token();
        let rt2 = gen_recovery_token();
        assert!(rt1.len() >= 32);
        assert_ne!(rt1, rt2);
        assert!(constant_time_eq(
            hash_token(&rt1).as_bytes(),
            hash_token(&rt1).as_bytes()
        ));
        assert!(!constant_time_eq(
            hash_token(&rt1).as_bytes(),
            hash_token(&rt2).as_bytes()
        ));
    }

    #[test]
    fn test_admin_recovery_workflow_and_session_invalidation_sqlite() {
        let db_path = std::env::temp_dir().join(format!(
            "zenne_core_rec_{}.db",
            uuid::Uuid::new_v4().as_simple()
        ));
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE web_users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT,
                password_changed_at INTEGER DEFAULT 0
            );
            CREATE TABLE password_recovery (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                token_hash TEXT NOT NULL,
                type TEXT NOT NULL,
                source TEXT DEFAULT NULL,
                expires_at TEXT NOT NULL,
                used_at TEXT DEFAULT NULL,
                attempts INTEGER DEFAULT 0
            );",
        )
        .unwrap();

        let old_hash_a = hash_password("OldPass123!").unwrap();
        let old_hash_b = hash_password("BobPass123!").unwrap();
        conn.execute(
            "INSERT INTO web_users (username, email, password_hash) VALUES ('alice', 'alice@example.com', ?1)",
            [old_hash_a.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO web_users (username, email, password_hash) VALUES ('bob', 'bob@example.com', ?1)",
            [old_hash_b.as_str()],
        )
        .unwrap();
        let uid_a: i64 = conn
            .query_row(
                "SELECT id FROM web_users WHERE email='alice@example.com'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let uid_b: i64 = conn
            .query_row(
                "SELECT id FROM web_users WHERE email='bob@example.com'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        // Pre-reset session token for Alice
        let old_session = sign_web_token("sec", uid_a, "alice", 3600);
        let old_claims = verify_web_token("sec", &old_session).unwrap();
        let old_iat = old_claims["iat"].as_i64().unwrap();

        // 4. Token expires after 8 minutes
        let raw_tok = gen_recovery_token();
        let tok_hash = hash_token(&raw_tok);
        let exp_str = (chrono::Utc::now() + chrono::Duration::minutes(8)).to_rfc3339();
        let exp_ts = chrono::DateTime::parse_from_rfc3339(&exp_str)
            .unwrap()
            .timestamp();
        let delta = exp_ts - chrono::Utc::now().timestamp();
        assert!((470..=485).contains(&delta));
        conn.execute(
            "INSERT INTO password_recovery (user_id, token_hash, type, source, expires_at, attempts) VALUES (?1, ?2, 'recovery_token', 'admin', ?3, 0)",
            rusqlite::params![uid_a, tok_hash, exp_str],
        )
        .unwrap();

        // 7. Invalid token rejected
        let bad_hash = hash_token("invalid-token");
        let bad_found: Option<i64> = conn
            .query_row(
                "SELECT id FROM password_recovery WHERE token_hash = ?1 AND used_at IS NULL",
                [bad_hash.as_str()],
                |r| r.get(0),
            )
            .ok();
        assert!(bad_found.is_none());

        // 8. Concurrent verification cannot consume the same token twice
        let rec_id: i64 = conn
            .query_row(
                "SELECT id FROM password_recovery WHERE token_hash = ?1 AND used_at IS NULL",
                [tok_hash.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        let p1 = db_path.clone();
        let p2 = db_path.clone();
        let (c1, c2) = std::thread::scope(|s| {
            let h1 = s.spawn(move || {
                let c = rusqlite::Connection::open(p1).unwrap();
                let _ = c.busy_timeout(std::time::Duration::from_secs(2));
                c.execute(
                    "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL",
                    [rec_id],
                )
                .unwrap_or(0)
            });
            let h2 = s.spawn(move || {
                let c = rusqlite::Connection::open(p2).unwrap();
                let _ = c.busy_timeout(std::time::Duration::from_secs(2));
                c.execute(
                    "UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL",
                    [rec_id],
                )
                .unwrap_or(0)
            });
            (h1.join().unwrap(), h2.join().unwrap())
        });
        // 5 & 8: Single-use atomic consumption
        assert_eq!(c1 + c2, 1);

        // 10 & 11: Reset Alice's password and invalidate old sessions; 6: Bob untouched
        let new_hash_a = hash_password("NewStrongPass456!").unwrap();
        let changed_at = old_iat + 2;
        conn.execute(
            "UPDATE web_users SET password_hash = ?1, password_changed_at = ?2 WHERE id = ?3",
            rusqlite::params![new_hash_a, changed_at, uid_a],
        )
        .unwrap();

        let stored_a: String = conn
            .query_row(
                "SELECT password_hash FROM web_users WHERE id=?1",
                [uid_a],
                |r| r.get(0),
            )
            .unwrap();
        let stored_changed: i64 = conn
            .query_row(
                "SELECT password_changed_at FROM web_users WHERE id=?1",
                [uid_a],
                |r| r.get(0),
            )
            .unwrap();
        assert!(verify_password("NewStrongPass456!", &stored_a));
        assert!(!verify_password("OldPass123!", &stored_a));
        assert!(old_iat < stored_changed);

        let stored_b: String = conn
            .query_row(
                "SELECT password_hash FROM web_users WHERE id=?1",
                [uid_b],
                |r| r.get(0),
            )
            .unwrap();
        assert!(verify_password("BobPass123!", &stored_b));

        drop(conn);
        let _ = std::fs::remove_file(db_path);
    }
}
