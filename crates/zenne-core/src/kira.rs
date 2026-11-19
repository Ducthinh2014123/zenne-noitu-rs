use reqwest::Client;
use serde_json::json;

pub async fn ask_ai_kira(
    http: &Client,
    api_endpoint: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    system_prompt: Option<&str>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("Chưa cấu hình KIRA_API_KEY trong file .env!".to_string());
    }
    let endpoint = format!("{}/chat/completions", api_endpoint.trim_end_matches('/'));
    let sys = system_prompt.unwrap_or(
        "Bạn là Zenne AI, trợ lý thông minh, thân thiện và hài hước trên bot Zenne Nối Từ. Trả lời ngắn gọn, tự nhiên bằng ngôn ngữ của người dùng.",
    );

    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": sys },
            { "role": "user", "content": prompt }
        ],
        "max_tokens": 800,
        "temperature": 0.7
    });

    let res = http
        .post(&endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Lỗi kết nối Kira AI: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Kira AI trả về lỗi HTTP {}: {}", status, text));
    }

    let data: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Lỗi đọc JSON từ Kira AI: {}", e))?;

    data["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "Kira AI không trả về nội dung.".to_string())
}
