pub mod watch_recipe;

/// Formats Unix seconds in the local timezone, or returns the translated unknown label.
pub fn format_timestamp(timestamp: u64) -> String {
    i64::try_from(timestamp)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_secs)
        .map(|datetime| {
            datetime
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| t!("generic.unknown").into_owned())
}

pub fn human_i_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    format!("{:.1} {}", value, UNITS[unit])
}
