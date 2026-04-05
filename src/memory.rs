//! Lightweight process memory instrumentation for Linux.

use std::fs;

#[derive(Debug, Clone, Copy)]
pub struct MemorySnapshot {
    pub rss_kb: u64,
    pub hwm_kb: u64,
}

impl MemorySnapshot {
    pub fn format_megabytes(self) -> String {
        format!(
            "rss={:.1}MB peak={:.1}MB",
            self.rss_kb as f64 / 1024.0,
            self.hwm_kb as f64 / 1024.0
        )
    }
}

fn parse_status_value_kb(contents: &str, key: &str) -> Option<u64> {
    contents
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u64>().ok())
}

pub fn current_memory_snapshot() -> Option<MemorySnapshot> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let rss_kb = parse_status_value_kb(&status, "VmRSS:")?;
    let hwm_kb = parse_status_value_kb(&status, "VmHWM:").unwrap_or(rss_kb);
    Some(MemorySnapshot { rss_kb, hwm_kb })
}

pub fn memory_logging_enabled() -> bool {
    matches!(
        std::env::var("CITYSTAC_LOG_MEMORY").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

pub fn memory_log_interval(default_value: usize) -> usize {
    std::env::var("CITYSTAC_LOG_MEMORY_EVERY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default_value)
}

pub fn log_memory(label: impl AsRef<str>) {
    if !memory_logging_enabled() {
        return;
    }

    if let Some(snapshot) = current_memory_snapshot() {
        eprintln!("[mem] {} {}", label.as_ref(), snapshot.format_megabytes());
    } else {
        eprintln!("[mem] {} unavailable", label.as_ref());
    }
}
