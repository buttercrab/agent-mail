use chrono::{DateTime, Utc};

pub fn format_time(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%dT%H:%M:%S.%9fZ").to_string()
}

pub fn now_parts() -> (String, i64) {
    let now = Utc::now();
    let nanos = now
        .timestamp_nanos_opt()
        .expect("current timestamp should fit in i64 nanos");
    (format_time(now), nanos)
}
