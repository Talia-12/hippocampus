use chrono::{NaiveDateTime, Timelike, Utc};

/// Returns the current UTC time truncated to millisecond precision
///
/// SQLite triggers in this project write timestamps via
/// `strftime('%Y-%m-%d %H:%M:%f', 'now')`, which has millisecond precision.
/// Rust-originated timestamps (`Utc::now()`) carry nanosecond precision.
/// To make `>` / `<` comparisons between trigger-set and Rust-set timestamps
/// behave consistently (no false-equal due to truncation), all Rust-side
/// writes use this helper.
pub fn now_ms() -> NaiveDateTime {
	truncate_to_ms(Utc::now().naive_utc())
}

/// Truncates a `NaiveDateTime` to millisecond precision
pub fn truncate_to_ms(t: NaiveDateTime) -> NaiveDateTime {
	t.with_nanosecond((t.nanosecond() / 1_000_000) * 1_000_000)
		.unwrap_or(t)
}
