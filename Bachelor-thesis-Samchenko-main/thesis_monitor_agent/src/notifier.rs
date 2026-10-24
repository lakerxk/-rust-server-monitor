use reqwest::Client;
use serde_json::json;
use crate::config::Config;

pub async fn send_telegram_alert(config: &Config, message: &str) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_token);
    let params = json!({ "chat_id": config.telegram_chat_id, "text": message });

    let client = Client::new();
    let _ = client.post(&url).json(&params).send().await;
}

pub async fn send_with_buttons(config: &Config, message: &str) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_token);

    let keyboard = json!({
        "inline_keyboard": [[
            { "text": "Start", "callback_data": "start" },
            { "text": "Stop",  "callback_data": "stop"  }
        ]]
    });

    let params = json!({
        "chat_id": config.telegram_chat_id,
        "text": message,
        "reply_markup": keyboard
    });

    let client = Client::new();
    let _ = client.post(&url).json(&params).send().await;
}
