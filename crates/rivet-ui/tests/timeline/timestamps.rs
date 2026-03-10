use chrono::{Local, TimeZone};
use rivet_ui::timeline::common::model::{format_date_divider_at, format_message_timestamp};

#[test]
fn message_timestamp_is_time_only_for_today() {
    let now = Local.with_ymd_and_hms(2026, 3, 10, 22, 0, 0).unwrap();
    let message = Local.with_ymd_and_hms(2026, 3, 10, 1, 40, 0).unwrap();

    assert_eq!(format_message_timestamp(message, now), "1:40 AM");
}

#[test]
fn message_timestamp_uses_yesterday_label() {
    let now = Local.with_ymd_and_hms(2026, 3, 10, 22, 0, 0).unwrap();
    let message = Local.with_ymd_and_hms(2026, 3, 9, 8, 2, 0).unwrap();

    assert_eq!(
        format_message_timestamp(message, now),
        "Yesterday at 8:02 AM"
    );
}

#[test]
fn message_timestamp_uses_short_date_for_older_messages() {
    let now = Local.with_ymd_and_hms(2026, 3, 10, 22, 0, 0).unwrap();
    let message = Local.with_ymd_and_hms(2025, 12, 27, 9, 42, 0).unwrap();

    assert_eq!(format_message_timestamp(message, now), "12/27/25, 9:42 AM");
}

#[test]
fn date_divider_uses_today_and_yesterday_labels() {
    let now = Local.with_ymd_and_hms(2026, 3, 10, 17, 0, 0).unwrap();
    let today = Local.with_ymd_and_hms(2026, 3, 10, 9, 0, 0).unwrap();
    let yesterday = Local.with_ymd_and_hms(2026, 3, 9, 9, 0, 0).unwrap();

    assert_eq!(format_date_divider_at(today, now), "Today");
    assert_eq!(format_date_divider_at(yesterday, now), "Yesterday");
}
