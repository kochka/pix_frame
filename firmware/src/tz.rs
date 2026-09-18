//! UTC offset of a few IANA time zones, computed from the daylight-saving
//! rules rather than a time zone database.
//!
//! Covers the European Union rule (clocks change on the last Sundays of
//! March and October at 01:00 UTC) and UTC itself. Anything else is unknown
//! and the caller falls back to UTC. A later version will take the offset
//! from the weather service instead. Hardware-free.

const HOUR: i32 = 3600;
const DAY: i64 = 86_400;

/// Offset of local time from UTC in seconds at the instant `unix_seconds`,
/// or `None` for a zone this module does not know.
pub fn utc_offset_seconds(zone: &str, unix_seconds: u64) -> Option<i32> {
    let unix = i64::try_from(unix_seconds).ok()?;
    match zone {
        "UTC" | "Etc/UTC" | "GMT" | "Etc/GMT" => Some(0),
        "Europe/Moscow" | "Europe/Istanbul" | "Europe/Minsk" => Some(3 * HOUR),
        zone if zone.starts_with("Europe/") => {
            let base = match zone {
                "Europe/London" | "Europe/Dublin" | "Europe/Lisbon" | "Europe/Belfast"
                | "Europe/Guernsey" | "Europe/Jersey" | "Europe/Isle_of_Man" => 0,
                "Europe/Athens" | "Europe/Bucharest" | "Europe/Helsinki" | "Europe/Kyiv"
                | "Europe/Kiev" | "Europe/Riga" | "Europe/Sofia" | "Europe/Tallinn"
                | "Europe/Vilnius" | "Europe/Chisinau" | "Europe/Mariehamn" => 2 * HOUR,
                _ => HOUR,
            };
            let dst = if eu_summer_time(unix) { HOUR } else { 0 };
            Some(base + dst)
        }
        _ => None,
    }
}

/// True between the last Sunday of March 01:00 UTC and the last Sunday of
/// October 01:00 UTC of the same year.
fn eu_summer_time(unix: i64) -> bool {
    let (year, _, _) = civil_from_days(unix.div_euclid(DAY));
    let start = last_sunday(year, 3) * DAY + i64::from(HOUR);
    let end = last_sunday(year, 10) * DAY + i64::from(HOUR);
    (start..end).contains(&unix)
}

/// Day number (since 1970-01-01) of the last Sunday of `month` in `year`.
fn last_sunday(year: i64, month: u8) -> i64 {
    let last_day = days_from_civil(year, month, days_in_month(year, month));
    // 1970-01-01 was a Thursday; with Sunday as 0 that is weekday 4.
    let weekday = (last_day + 4).rem_euclid(7);
    last_day - weekday
}

fn days_in_month(year: i64, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        _ => 28,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: u8, day: u8) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let month = i64::from(month);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]: `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
