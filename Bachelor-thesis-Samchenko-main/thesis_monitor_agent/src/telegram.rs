use crate::{config::Config, logs, metrics, notifier};
use crate::metrics::NetTotals;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use sysinfo::{Disks, Networks, ProcessesToUpdate, System};

/// Telegram Update objects
#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
    callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize)]
struct Message {
    chat: Chat,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Chat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    id: String,
    from: Chat,
    data: Option<String>,
}

/// Основной цикл long-poll Telegram: подтягивает апдейты и вызывает обработчики
pub async fn poll_updates(config: Config, running: Arc<AtomicBool>) {
    let client = Client::new();
    let mut offset: i64 = 0;

    loop {
        match get_updates(&client, &config.telegram_token, offset).await {
            Ok(updates) => {
                for upd in updates {
                    offset = upd.update_id + 1;

                    // Сообщения с текстом (команды)
                    if let Some(msg) = upd.message {
                        if let Some(text) = msg.text {
                            // Фильтрация по целевому чату (если нужно жёстко ограничить)
                            // if msg.chat.id.to_string() != config.telegram_chat_id { continue; }
                            handle_command(&config, &text, running.clone()).await;
                        }
                    }

                    // Нажатия на inline-кнопки
                    if let Some(cb) = upd.callback_query {
                        if let Some(data) = cb.data {
                            match data.as_str() {
                                "start" => {
                                    running.store(true, Ordering::SeqCst);
                                    notifier::send_telegram_alert(&config, "▶️ Агент запущен").await;
                                }
                                "stop" => {
                                    running.store(false, Ordering::SeqCst);
                                    notifier::send_telegram_alert(&config, "⏹️ Агент остановлен").await;
                                }
                                _ => {
                                    notifier::send_telegram_alert(&config, "❓ Неизвестная кнопка").await;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // Не шумим чрезмерно: отправим один раз в лог
                log::warn!("Telegram polling error: {}", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }

        // Long-poll пауза (Telegram сам держит соединение, но небольшая пауза не повредит)
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Запрос новых апдейтов у Telegram
async fn get_updates(client: &Client, token: &str, offset: i64) -> anyhow::Result<Vec<Update>> {
    let url = format!("https://api.telegram.org/bot{}/getUpdates", token);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "offset": offset,
            "timeout": 20,
            "allowed_updates": ["message", "callback_query"]
        }))
        .send()
        .await?;

    let val: serde_json::Value = resp.json().await?;
    if !val["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!("Telegram getUpdates returned not ok: {}", val);
    }
    let updates: Vec<Update> = serde_json::from_value(val["result"].clone())?;
    Ok(updates)
}

/// Асинхронная обработка команд
pub async fn handle_command(config: &Config, text: &str, running: Arc<AtomicBool>) {
    match text.trim() {
        "/start" => {
            notifier::send_with_buttons(config, r#"👋 Agent control: press  ▶️ Start or ⏹️ Stop"#).await;
        }
        "/ping" => {
            notifier::send_telegram_alert(config, "🏓 Pong").await;
        }
        "/metrics" => {
            if running.load(Ordering::SeqCst) {
                let m = metrics::collect_system_metrics();
                notifier::send_telegram_alert(config, &m).await;
            } else {
                notifier::send_telegram_alert(config, "⏹️ Агент остановлен").await;
            }
        }
        "/cpu" => {
            let mut sys = System::new_all();
            sys.refresh_cpu_all();
            notifier::send_telegram_alert(config, &format!("💻 CPU: {:.2}%", sys.global_cpu_usage()))
                .await;
        }
        "/mem" => {
            let mut sys = System::new_all();
            sys.refresh_memory();
            notifier::send_telegram_alert(
                config,
                &format!(
                    "🧠 Memory: {}/{} MB",
                    sys.used_memory() / 1024,
                    sys.total_memory() / 1024
                ),
            )
            .await;
        }
        "/disk" => {
            let disks = Disks::new_with_refreshed_list();
            let mut msg = String::from("💾 Disks:\n");
            for d in disks.iter() {
                msg.push_str(&format!(
                    "{}: {} GB total, {} GB free\n",
                    d.name().to_string_lossy(),
                    d.total_space() / 1_073_741_824,
                    d.available_space() / 1_073_741_824
                ));
            }
            notifier::send_telegram_alert(config, &msg).await;
        }
        "/net" => {
            let nets = Networks::new_with_refreshed_list();
            let mut msg = String::from("🌐 Network:\n");
            for (name, data) in nets.iter() {
                msg.push_str(&format!(
                    "{}: rx {} KB, tx {} KB\n",
                    name,
                    data.total_received() / 1024,
                    data.total_transmitted() / 1024
                ));
            }
            notifier::send_telegram_alert(config, &msg).await;
        }
        "/uptime" => {
            let secs = System::uptime();
            let d = Duration::from_secs(secs);
            notifier::send_telegram_alert(
                config,
                &format!("⏱️ Uptime: {}h {}m", d.as_secs() / 3600, (d.as_secs() % 3600) / 60),
            )
            .await;
        }
        "/top" => {
            let mut sys = System::new_all();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let mut processes: Vec<_> = sys.processes().values().collect();
            processes.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap());

            let mut msg = String::from("🔥 Top 5 processes:\n");
            for p in processes.iter().take(5) {
                msg.push_str(&format!(
                    "{} (pid {}): {:.2}% CPU, {} KB mem\n",
                    p.name().to_string_lossy(),
                    p.pid(),
                    p.cpu_usage(),
                    p.memory()
                ));
            }
            notifier::send_telegram_alert(config, &msg).await;
        }
        "/logs" => {
            if running.load(Ordering::SeqCst) {
                let results = logs::scan_logs(&config.log_paths);
                if results.is_empty() {
                    notifier::send_telegram_alert(config, "✅ No errors found in logs.").await;
                } else {
                    let summary = results
                        .iter()
                        .take(10)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n");
                    notifier::send_telegram_alert(
                        config,
                        &format!("⚠️ Log matches:\n{}", summary),
                    )
                    .await;
                }
            } else {
                notifier::send_telegram_alert(config, "⏹️ Agent stopped").await;
            }
        }
        "/monitor" => {
            // Однократная выборка сетевых тоталов → пауза → дифф → алерт
            let period_secs = config.sampling.period_secs.max(1);
            let before = metrics::capture_network_totals();
            tokio::time::sleep(Duration::from_secs(period_secs)).await;
            let after = metrics::capture_network_totals();

            if let Some(msg) = diff_and_alert(&before, &after, period_secs, config) {
                notifier::send_telegram_alert(config, &msg).await;
            } else {
                notifier::send_telegram_alert(config, "✅ No anomalies detected").await;
            }
        }
        _ => {
            notifier::send_telegram_alert(config, "❓ Unknown command").await;
        }
    }
}

/// Сравнение сетевых счётчиков и формирование сообщения об аномалиях
pub fn diff_and_alert(
    before: &[NetTotals],
    after: &[NetTotals],
    period_secs: u64,
    config: &Config,
) -> Option<String> {
    let mut bmap = HashMap::new();
    for b in before {
        bmap.insert(&b.iface, (b.rx_bytes, b.tx_bytes));
    }

    let mut messages = Vec::new();

    for a in after {
        if let Some((brx, btx)) = bmap.get(&a.iface) {
            let drx = a.rx_bytes.saturating_sub(*brx);
            let dtx = a.tx_bytes.saturating_sub(*btx);
            let rx_kb_s = (drx / 1024) / period_secs.max(1);
            let tx_kb_s = (dtx / 1024) / period_secs.max(1);

            if rx_kb_s > config.thresholds.rx_kb_per_sec
                || tx_kb_s > config.thresholds.tx_kb_per_sec
            {
                messages.push(format!(
                    "⚠️ Network anomaly on {}: inbound {} KB/s, outbound {} KB/s",
                    a.iface, rx_kb_s, tx_kb_s
                ));
            }
        }
    }

    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}
