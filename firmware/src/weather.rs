//! Current outdoor weather from Open-Meteo, and the tiny JSON lookups that
//! extract it. The network side lives in `net.rs`; this module is
//! hardware-free.

use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};

use heapless::String;

use crate::config::Location;

pub const HOST: &str = "api.open-meteo.com";

/// Coarse sky condition, derived from the WMO weather code Open-Meteo returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Condition {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Rain,
    Snow,
    Thunderstorm,
}

impl Condition {
    /// Every condition, in display order.
    pub const ALL: [Self; 7] = [
        Self::Clear,
        Self::PartlyCloudy,
        Self::Cloudy,
        Self::Fog,
        Self::Rain,
        Self::Snow,
        Self::Thunderstorm,
    ];

    /// WMO 4677 weather interpretation codes, as used by Open-Meteo.
    pub fn from_wmo(code: u8) -> Self {
        match code {
            0 => Self::Clear,
            1 | 2 => Self::PartlyCloudy,
            3 => Self::Cloudy,
            45 | 48 => Self::Fog,
            51..=67 | 80..=82 => Self::Rain,
            71..=77 | 85 | 86 => Self::Snow,
            95..=99 => Self::Thunderstorm,
            _ => Self::Cloudy,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Degrees Celsius × 10.
    pub temperature_dc: i32,
    pub condition: Condition,
    /// Local time minus UTC, in seconds, for the configured time zone.
    pub utc_offset: i32,
}

static VALID: AtomicBool = AtomicBool::new(false);
static TEMPERATURE_DC: AtomicI32 = AtomicI32::new(0);
static CONDITION: AtomicU8 = AtomicU8::new(0);
static UTC_OFFSET: AtomicI32 = AtomicI32::new(0);
/// Unix time of the last successful fetch; `u32` is fine until 2106 and is
/// the widest atomic on this 32-bit core.
static FETCHED_AT: AtomicU32 = AtomicU32::new(0);

/// How long an observation stays displayable without a refresh.
pub const MAX_AGE_SECONDS: u64 = 60 * 60;

/// Publishes a fresh observation for the display.
pub fn publish(observation: Observation, now_unix: u64) {
    TEMPERATURE_DC.store(observation.temperature_dc, Ordering::Relaxed);
    CONDITION.store(observation.condition as u8, Ordering::Relaxed);
    UTC_OFFSET.store(observation.utc_offset, Ordering::Relaxed);
    FETCHED_AT.store(
        u32::try_from(now_unix).unwrap_or(u32::MAX),
        Ordering::Relaxed,
    );
    VALID.store(true, Ordering::Release);
}

/// The last observation if it is younger than [`MAX_AGE_SECONDS`].
pub fn current(now_unix: Option<u64>) -> Option<Observation> {
    if !VALID.load(Ordering::Acquire) {
        return None;
    }
    let age = now_unix?.saturating_sub(u64::from(FETCHED_AT.load(Ordering::Relaxed)));
    if age > MAX_AGE_SECONDS {
        return None;
    }
    Some(Observation {
        temperature_dc: TEMPERATURE_DC.load(Ordering::Relaxed),
        condition: condition_from_u8(CONDITION.load(Ordering::Relaxed)),
        utc_offset: UTC_OFFSET.load(Ordering::Relaxed),
    })
}

fn condition_from_u8(value: u8) -> Condition {
    match value {
        1 => Condition::PartlyCloudy,
        2 => Condition::Cloudy,
        3 => Condition::Fog,
        4 => Condition::Rain,
        5 => Condition::Snow,
        6 => Condition::Thunderstorm,
        _ => Condition::Clear,
    }
}

/// Request path for the current conditions at `location`.
pub fn request_path(location: &Location) -> Option<String<256>> {
    let mut path = String::new();
    write!(
        path,
        "/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code&timezone={}",
        Degrees(location.latitude_e6),
        Degrees(location.longitude_e6),
        location.timezone.as_str()
    )
    .ok()?;
    Some(path)
}

/// Formats micro-degrees as decimal degrees without floating point.
struct Degrees(i32);

impl core::fmt::Display for Degrees {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let magnitude = self.0.unsigned_abs();
        write!(
            f,
            "{sign}{}.{:06}",
            magnitude / 1_000_000,
            magnitude % 1_000_000
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum ParseError {
    MissingTemperature,
    MissingWeatherCode,
    MissingUtcOffset,
}

/// Extracts the three fields used by the firmware from an Open-Meteo reply.
pub fn parse(body: &[u8]) -> Result<Observation, ParseError> {
    let temperature =
        find_number(body, b"\"temperature_2m\":").ok_or(ParseError::MissingTemperature)?;
    let code = find_number(body, b"\"weather_code\":").ok_or(ParseError::MissingWeatherCode)?;
    let offset =
        find_number(body, b"\"utc_offset_seconds\":").ok_or(ParseError::MissingUtcOffset)?;
    Ok(Observation {
        temperature_dc: temperature.tenths(),
        condition: Condition::from_wmo(u8::try_from(code.integer()).unwrap_or(u8::MAX)),
        utc_offset: offset.integer(),
    })
}

/// A decimal number read from JSON, kept exact as integer and tenths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Number {
    negative: bool,
    integer: u32,
    /// First fractional digit, already rounded from the second one.
    tenths: u32,
}

impl Number {
    fn integer(self) -> i32 {
        let value = i32::try_from(self.integer).unwrap_or(i32::MAX);
        if self.negative { -value } else { value }
    }

    fn tenths(self) -> i32 {
        let value = i32::try_from(self.integer * 10 + self.tenths).unwrap_or(i32::MAX);
        if self.negative { -value } else { value }
    }
}

/// Finds the last occurrence of `key` and parses the number that follows.
///
/// The last one is used because `current_units` precedes `current` in the
/// reply and repeats the field names with string values.
fn find_number(body: &[u8], key: &[u8]) -> Option<Number> {
    let mut best = None;
    let mut from = 0;
    while let Some(pos) = body[from..].windows(key.len()).position(|w| w == key) {
        let start = from + pos + key.len();
        if let Some(number) = parse_number(&body[start..]) {
            best = Some(number);
        }
        from = start;
    }
    best
}

fn parse_number(bytes: &[u8]) -> Option<Number> {
    let mut i = 0;
    while bytes.get(i) == Some(&b' ') {
        i += 1;
    }
    let negative = bytes.get(i) == Some(&b'-');
    if negative {
        i += 1;
    }
    let mut integer: u32 = 0;
    let mut digits = 0;
    while let Some(d) = bytes.get(i).filter(|b| b.is_ascii_digit()) {
        integer = integer.checked_mul(10)?.checked_add(u32::from(d - b'0'))?;
        i += 1;
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    let mut tenths = 0;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let first = bytes.get(i).filter(|b| b.is_ascii_digit())?;
        tenths = u32::from(first - b'0');
        i += 1;
        if let Some(second) = bytes.get(i).filter(|b| b.is_ascii_digit())
            && *second >= b'5'
        {
            tenths += 1;
        }
    }
    if tenths == 10 {
        tenths = 0;
        integer = integer.checked_add(1)?;
    }
    Some(Number {
        negative,
        integer,
        tenths,
    })
}
