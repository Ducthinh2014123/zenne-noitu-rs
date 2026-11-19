use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct LangEntry {
    pub code: String,
    pub name_en: String,
    pub name_native: String,
}

static LANG_CATALOG_RAW: &str = include_str!("../../../languages.json");

pub static LANG_CATALOG: Lazy<Vec<LangEntry>> =
    Lazy::new(|| serde_json::from_str(LANG_CATALOG_RAW).unwrap_or_default());

pub static LANG_MAP: Lazy<HashMap<String, LangEntry>> = Lazy::new(|| {
    LANG_CATALOG
        .iter()
        .map(|e| (e.code.to_lowercase(), e.clone()))
        .collect()
});

pub fn is_known_ui_lang(code: &str) -> bool {
    let c = code.trim().to_lowercase();
    LANG_MAP.contains_key(&c)
        || matches!(
            c.as_str(),
            "vi" | "en"
                | "ja"
                | "ko"
                | "zh"
                | "fr"
                | "de"
                | "es"
                | "pt"
                | "ru"
                | "th"
                | "id"
                | "ar"
                | "tr"
        )
}

pub fn lang_display_name(code: &str) -> String {
    let c = code.trim().to_lowercase();
    if let Some(entry) = LANG_MAP.get(&c) {
        if entry.name_native.is_empty() || entry.name_native == entry.name_en {
            entry.name_en.clone()
        } else {
            format!("{} ({})", entry.name_en, entry.name_native)
        }
    } else {
        c.to_uppercase()
    }
}

pub fn lang_catalog_search(query: &str, limit: usize) -> Vec<&'static LangEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return LANG_CATALOG.iter().take(limit).collect();
    }
    LANG_CATALOG
        .iter()
        .filter(|e| {
            e.code.to_lowercase().contains(&q)
                || e.name_en.to_lowercase().contains(&q)
                || e.name_native.to_lowercase().contains(&q)
        })
        .take(limit)
        .collect()
}

pub fn t(lang: &str, key: &str, vars: &[(&str, &str)]) -> String {
    let l = lang.trim().to_lowercase();
    let template = match (l.as_str(), key) {
        ("vi", "wrong_chain") => "❌ **{name}**, `{word}` phải bắt đầu bằng **`{syl}`**!{hint}",
        ("vi", "word_used") => "🔁 Từ **`{word}`** đã dùng rồi!",
        ("vi", "wiki_not_found") => "❓ **`{word}`** không tìm thấy trên Wikipedia {langname}!\n💡 Từ có thật? Dùng `!gopthemtu {word}` để đề nghị thêm.{hint}",
        ("vi", "valid_word_field") => "✅ Từ hợp lệ",
        ("vi", "next_start_field") => "🔗 Tiếp theo bắt đầu",
        ("vi", "next_turn_field") => "🎯 Đến lượt",
        ("vi", "hint_label") => "💡 Gợi ý",
        ("vi", "setuilang_success") => "✅ Đã đổi ngôn ngữ giao diện bot sang **{lang}**!",
        ("vi", "setuilang_invalid") => "❌ Mã ngôn ngữ không hợp lệ! Dùng `!listlang <tìm>` để tra mã.",
        ("vi", "listlang_title") => "🌐 Danh sách mã ngôn ngữ",
        ("en", "wrong_chain") => "❌ **{name}**, `{word}` must start with **`{syl}`**!{hint}",
        ("en", "word_used") => "🔁 Word **`{word}`** was already used!",
        ("en", "wiki_not_found") => "❓ **`{word}`** was not found on {langname} Wikipedia!\n💡 Real word? Use `!gopthemtu {word}` to suggest adding it.{hint}",
        ("en", "valid_word_field") => "✅ Valid word",
        ("en", "next_start_field") => "🔗 Next starts with",
        ("en", "next_turn_field") => "🎯 Next turn",
        ("en", "hint_label") => "💡 Hint",
        ("en", "setuilang_success") => "✅ Bot UI language changed to **{lang}**!",
        ("en", "setuilang_invalid") => "❌ Invalid language code! Use `!listlang <search>` to look it up.",
        ("en", "listlang_title") => "🌐 Language code list",
        (_, "wrong_chain") => "❌ **{name}**, `{word}` must start with **`{syl}`**!{hint}",
        (_, "word_used") => "🔁 Word **`{word}`** was already used!",
        (_, "wiki_not_found") => "❓ **`{word}`** was not found on {langname} Wikipedia!\n💡 Real word? Use `!gopthemtu {word}` to suggest adding it.{hint}",
        (_, "valid_word_field") => "✅ Valid word",
        (_, "next_start_field") => "🔗 Next starts with",
        (_, "next_turn_field") => "🎯 Next turn",
        (_, "hint_label") => "💡 Hint",
        (_, "setuilang_success") => "✅ Bot UI language changed to **{lang}**!",
        (_, "setuilang_invalid") => "❌ Invalid language code!",
        (_, "listlang_title") => "🌐 Language code list",
        _ => return key.to_string(),
    };

    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{}}}", k), v);
    }
    out
}
