//! Wall-clock time kept by the RTC timer.
//!
//! The RTC counts from an arbitrary origin until [`Clock::set_unix_time`] is
//! called (by a future SNTP sync); until then [`Clock::local_time`] returns
//! `None` and screens that need the time stay unavailable.

use esp_hal::rtc_cntl::Rtc;

pub struct Clock {
    rtc: Rtc<'static>,
    synced: bool,
    /// Local time minus UTC, in seconds.
    utc_offset: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Clock {
    pub fn new(rtc: Rtc<'static>) -> Self {
        Self {
            rtc,
            synced: false,
            utc_offset: 0,
        }
    }

    /// Sets the current UTC time as seconds since 1970-01-01.
    pub fn set_unix_time(&mut self, seconds: u64) {
        self.rtc
            .set_current_time_us(seconds.saturating_mul(1_000_000));
        self.synced = true;
    }

    /// Sets the offset between local time and UTC, in seconds.
    pub fn set_utc_offset(&mut self, seconds: i32) {
        self.utc_offset = seconds;
    }

    pub fn is_synced(&self) -> bool {
        self.synced
    }

    /// Local time of day, or `None` before the first sync.
    pub fn local_time(&self) -> Option<LocalTime> {
        if !self.synced {
            return None;
        }
        let unix = self.rtc.current_time_us() / 1_000_000;
        Some(time_of_day(unix, self.utc_offset))
    }
}

/// Splits a UTC timestamp shifted by `utc_offset` into hours, minutes, seconds.
pub fn time_of_day(unix_seconds: u64, utc_offset: i32) -> LocalTime {
    const DAY: i64 = 86_400;
    let local = i64::try_from(unix_seconds).unwrap_or(i64::MAX) + i64::from(utc_offset);
    let of_day = local.rem_euclid(DAY);
    LocalTime {
        // Each value is below 60 or 24, so the casts are lossless.
        hour: (of_day / 3600) as u8,
        minute: (of_day % 3600 / 60) as u8,
        second: (of_day % 60) as u8,
    }
}
