use sysinfo::{System, Disks, Networks};

pub fn collect_system_metrics() -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_usage();
    let total_mem_mb = sys.total_memory() / 1024;
    let used_mem_mb = sys.used_memory() / 1024;

    let disks = Disks::new_with_refreshed_list();
    let total_disk_gb: u64 = disks.iter().map(|d| d.total_space()).sum::<u64>() / 1_073_741_824;

    format!(
        "📊 Metrics\nCPU: {:.2}%\nMemory: {}/{} MB\nDisk total: {} GB",
        cpu_usage, used_mem_mb, total_mem_mb, total_disk_gb
    )
}

#[derive(Debug, Clone)]
pub struct NetTotals {
    pub iface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

pub fn capture_network_totals() -> Vec<NetTotals> {
    let nets = Networks::new_with_refreshed_list();
    nets.iter()
        .map(|(name, data)| NetTotals {
            iface: name.to_string(),
            rx_bytes: data.total_received(),
            tx_bytes: data.total_transmitted(),
        })
        .collect()
}
