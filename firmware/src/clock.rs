//! Wall-clock time kept by the RTC timer.
//!
//! The RTC counts from an arbitrary origin until [`Clock::set_unix_time`] is
//! called by the SNTP task; until then [`Clock::local_time`] returns `None`
//! and screens that need the time stay unavailable. The handle is `Copy` so
//! the playback loop and the network tasks can share it; the sync state and
//! the UTC offset live in atomics.

use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use esp_hal::rtc_cntl::Rtc;

static SYNCED: AtomicBool = AtomicBool::new(false);
static UTC_OFFSET: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Copy)]
pub struct Clock {
    rtc: &'static Rtc<'static>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Clock {
    pub fn new(rtc: &'static Rtc<'static>) -> Self {
        Self { rtc }
    }

    /// Sets the current UTC time as seconds since 1970-01-01.
    pub fn set_unix_time(&self, seconds: u64) {
        self.rtc
            .set_current_time_us(seconds.saturating_mul(1_000_000));
        SYNCED.store(true, Ordering::Release);
    }

    /// Sets the offset between local time and UTC, in seconds.
    pub fn set_utc_offset(&self, seconds: i32) {
        UTC_OFFSET.store(seconds, Ordering::Relaxed);
    }

    pub fn is_synced(&self) -> bool {
        SYNCED.load(Ordering::Acquire)
    }

    /// Seconds since 1970-01-01 UTC, or `None` before the first sync.
    pub fn unix_time(&self) -> Option<u64> {
        self.is_synced()
            .then(|| self.rtc.current_time_us() / 1_000_000)
    }

    /// Local time of day, or `None` before the first sync.
    pub fn local_time(&self) -> Option<LocalTime> {
        let unix = self.unix_time()?;
        Some(time_of_day(unix, UTC_OFFSET.load(Ordering::Relaxed)))
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
