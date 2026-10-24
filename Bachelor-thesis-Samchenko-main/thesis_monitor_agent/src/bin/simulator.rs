use anyhow::Result;
use std::net::TcpStream;
use std::time::Duration;
use std::io::Write;

fn main() -> Result<()> {
    // Configure target (Device B)
    let target_ip = std::env::var("TARGET_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let target_port: u16 = std::env::var("TARGET_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let interval_ms: u64 = std::env::var("INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    println!(
        "Simulator targeting {}:{} every {} ms",
        target_ip, target_port, interval_ms
    );

    loop {
        match TcpStream::connect((target_ip.as_str(), target_port)) {
            Ok(mut stream) => {
                let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                let payload = b"SIMULATED_TRAFFIC\n";
                let _ = stream.write_all(payload);
            }
            Err(_) => {
                // Silently ignore connection errors (port closed or no listener)
            }
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
    }
}
