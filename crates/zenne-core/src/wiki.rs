use crate::cache::CacheManager;
use crate::db::DbManager;
use rand::seq::SliceRandom;
use reqwest::Client;
use serde_json::Value;

pub fn normalize_word(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

pub fn first_syl(w: &str, lang: &str) -> String {
    let w = w.trim().to_lowercase();
    if w.is_empty() {
        return w;
    }
    if lang.eq_ignore_ascii_case("vi") {
        w.split_whitespace().next().unwrap_or("").to_string()
    } else {
        w.chars().next().map(|c| c.to_string()).unwrap_or_default()
    }
}

pub fn last_syl(w: &str, lang: &str) -> String {
    let w = w.trim().to_lowercase();
    if w.is_empty() {
        return w;
    }
    if lang.eq_ignore_ascii_case("vi") {
        w.split_whitespace().last().unwrap_or("").to_string()
    } else {
        w.chars()
            .next_back()
            .map(|c| c.to_string())
            .unwrap_or_default()
    }
}

pub fn valid_chain(prev: &str, nxt: &str, lang: &str) -> bool {
    let l = last_syl(prev, lang);
    let f = first_syl(nxt, lang);
    !l.is_empty() && l == f
}

pub fn is_valid_for_lang(word: &str, _lang: &str) -> bool {
    let w = word.trim();
    if w.is_empty() || w.len() > 80 {
        return false;
    }
    // Từ phải chứa ít nhất 1 chữ cái
    w.chars().any(|c| c.is_alphabetic())
}

pub async fn check_word(
    http: &Client,
    db: &DbManager,
    cache: &CacheManager,
    guild_id: i64,
    lang: &str,
    raw_word: &str,
) -> (bool, Option<String>) {
    let word = normalize_word(raw_word);
    if !is_valid_for_lang(&word, lang) {
        return (false, None);
    }

    // 1. Word bank của server
    if db.word_exists(guild_id, &word).await {
        return (
            true,
            Some(format!("Từ **{word}** có trong Ngân hàng từ của server.")),
        );
    }

    // 2. Cache Redis / In-Memory
    let ckey = format!("wiki:{}:{}", lang, word);
    if let Some(val) = cache.get(&ckey).await {
        if val == "1" {
            return (true, None);
        } else if val == "0" {
            return (false, None);
        }
    }

    // 3. Cache SQLite/libSQL
    if db.cache_exists(lang, &word).await {
        cache.set(&ckey, "1", 86400).await;
        return (true, None);
    }

    // 4. Tra Wikipedia REST API + Wiktionary
    let encoded = urlencoding_simple(&word);
    let wiki_url = format!(
        "https://{}.wikipedia.org/api/rest_v1/page/summary/{}",
        lang, encoded
    );
    if let Ok(res) = http.get(&wiki_url).send().await {
        if res.status().is_success() {
            if let Ok(json) = res.json::<Value>().await {
                if json.get("type").and_then(|v| v.as_str())
                    != Some("https://mediawiki.org/wiki/HyperSwitch/errors/not_found")
                {
                    let extract = json
                        .get("extract")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    db.cache_add(lang, &word).await;
                    cache.set(&ckey, "1", 86400).await;
                    return (true, extract);
                }
            }
        }
    }

    let wikt_url = format!(
        "https://{}.wiktionary.org/w/api.php?action=query&titles={}&format=json",
        lang, encoded
    );
    if let Ok(res) = http.get(&wikt_url).send().await {
        if res.status().is_success() {
            if let Ok(json) = res.json::<Value>().await {
                if let Some(pages) = json["query"]["pages"].as_object() {
                    if pages.keys().any(|k| k != "-1") {
                        db.cache_add(lang, &word).await;
                        cache.set(&ckey, "1", 86400).await;
                        return (true, None);
                    }
                }
            }
        }
    }

    cache.set(&ckey, "0", 3600).await;
    (false, None)
}

pub async fn random_word(http: &Client, db: &DbManager, lang: &str) -> String {
    // Thử lấy ngẫu nhiên từ wiki_cache trước để siêu nhanh
    if let Some(w) = db.random_cached_word(lang).await {
        return w;
    }

    let url = format!(
        "https://{}.wikipedia.org/api/rest_v1/page/random/summary",
        lang
    );
    if let Ok(res) = http.get(&url).send().await {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(title) = json["title"].as_str() {
                let norm = normalize_word(title);
                if norm.split_whitespace().count() <= 3 && is_valid_for_lang(&norm, lang) {
                    return norm;
                }
            }
        }
    }

    let fallbacks_vi = [
        "hòa bình",
        "bình minh",
        "minh bạch",
        "bạch mã",
        "mã đáo",
        "hoa hồng",
        "hồng hào",
    ];
    let fallbacks_en = [
        "apple", "elephant", "tiger", "river", "rainbow", "window", "water",
    ];
    let mut rng = rand::thread_rng();
    if lang.eq_ignore_ascii_case("vi") {
        fallbacks_vi
            .choose(&mut rng)
            .unwrap_or(&"hòa bình")
            .to_string()
    } else {
        fallbacks_en
            .choose(&mut rng)
            .unwrap_or(&"apple")
            .to_string()
    }
}

pub async fn find_continuations(
    db: &DbManager,
    guild_id: i64,
    lang: &str,
    prefix_syl: &str,
    used_words: &[String],
    limit: usize,
) -> Vec<String> {
    let mut results = db
        .words_starting_with(guild_id, lang, prefix_syl, limit + used_words.len())
        .await;
    results.retain(|w| !used_words.iter().any(|u| u.eq_ignore_ascii_case(w)));
    results.truncate(limit);
    results
}

pub fn urlencoding_simple(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('_'),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_chain_vietnamese_and_english() {
        assert_eq!(normalize_word("  Hòa   Bình "), "hòa bình");
        assert_eq!(first_syl("hòa bình", "vi"), "hòa");
        assert_eq!(last_syl("hòa bình", "vi"), "bình");
        assert!(valid_chain("hòa bình", "bình minh", "vi"));
        assert!(!valid_chain("hòa bình", "minh bạch", "vi"));

        assert!(valid_chain("apple", "elephant", "en"));
        assert!(!valid_chain("apple", "banana", "en"));
    }
}
