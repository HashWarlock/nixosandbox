use chrono::Utc;

/// Return current UTC time as ISO 8601 string.
pub fn now_iso8601() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
