pub fn short_id(id: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        return "未知设备".to_string();
    }

    if id.chars().count() > 12 {
        let prefix = id.chars().take(6).collect::<String>();
        let suffix = id
            .chars()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        format!("{prefix}…{suffix}")
    } else {
        id.to_string()
    }
}

pub fn format_bytes(value: u64) -> String {
    if value == 0 {
        return "0 B".to_string();
    }

    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < units.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if size >= 10.0 || unit_index == 0 {
        format!("{size:.0} {}", units[unit_index])
    } else {
        format!("{size:.1} {}", units[unit_index])
    }
}

pub fn format_duration(seconds: u64, ready: bool) -> String {
    if seconds == 0 {
        return if ready {
            "刚刚启动"
        } else {
            "等待数据"
        }
        .to_string();
    }

    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;

    if days > 0 {
        format!("{days} 天 {hours} 小时")
    } else if hours > 0 {
        format!("{hours} 小时 {minutes} 分钟")
    } else {
        format!("{minutes} 分钟")
    }
}

pub fn split_ids(value: &str) -> Vec<String> {
    value
        .split([',', '\n', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn parse_addresses(value: &str) -> Vec<String> {
    let addresses = split_ids(value);
    if addresses.is_empty() {
        vec!["dynamic".to_string()]
    } else {
        addresses
    }
}
