mod config;
mod metrics;
mod logs;
mod notifier;
mod telegram;

use config::load_config;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::task;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Логгер
    env_logger::init();

    // Загружаем конфигурацию
    let config = load_config("Config.toml");
    let running = Arc::new(AtomicBool::new(true));

    // Запускаем Telegram poller
    {
        let cfg = config.clone();
        let run = running.clone();
        task::spawn(async move {
            telegram::poll_updates(cfg, run).await;
        });
    }

    // Основной цикл мониторинга
    let sampling_period = config.sampling.period_secs;
    let mut prev = metrics::capture_network_totals();

    loop {
        if running.load(Ordering::SeqCst) {
            // Сбор метрик
            let summary = metrics::collect_system_metrics();
            notifier::send_telegram_alert(&config, &summary).await;

            // Ждём период
            tokio::time::sleep(Duration::from_secs(sampling_period)).await;

            // Сравнение сетевых показателей
            let after = metrics::capture_network_totals();
            if let Some(msg) =
                telegram::diff_and_alert(&prev, &after, sampling_period, &config)
            {
                notifier::send_telegram_alert(&config, msg.as_str()).await;
            }

            prev = after;
        } else {
            // Агент остановлен — ждём
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}
