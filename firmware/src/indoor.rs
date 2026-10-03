//! Indoor temperature and humidity from an SHT3x on I2C.
//!
//! The sensor is read with single-shot measurements, heater off, so the die
//! does not warm itself. Each reading is published through atomics for the
//! information screens. A missing or failing sensor only hides the screen.

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};

use defmt::{info, warn};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::Async;
use esp_hal::i2c::master::I2c;

/// I2C address with ADDR tied to GND, as on most breakout boards.
pub const ADDRESS: u8 = 0x44;

/// Single shot, high repeatability, clock stretching disabled.
const MEASURE: [u8; 2] = [0x24, 0x00];
/// Maximum high-repeatability measurement time is 15.5 ms.
const MEASURE_TIME: Duration = Duration::from_millis(20);
const READ_INTERVAL: Duration = Duration::from_secs(60);

/// How long a reading stays displayable without a refresh.
pub const MAX_AGE_SECONDS: u64 = 5 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    /// Degrees Celsius × 10.
    pub temperature_dc: i32,
    /// Relative humidity in whole percent, 0..=100.
    pub humidity: u8,
}

static VALID: AtomicBool = AtomicBool::new(false);
static TEMPERATURE_DC: AtomicI32 = AtomicI32::new(0);
static HUMIDITY: AtomicU8 = AtomicU8::new(0);
/// Seconds since boot of the last successful reading; the sensor does not
/// need the wall clock.
static READ_AT: AtomicU32 = AtomicU32::new(0);

fn uptime_seconds() -> u32 {
    u32::try_from(Instant::now().as_secs()).unwrap_or(u32::MAX)
}

fn publish(reading: Reading) {
    TEMPERATURE_DC.store(reading.temperature_dc, Ordering::Relaxed);
    HUMIDITY.store(reading.humidity, Ordering::Relaxed);
    READ_AT.store(uptime_seconds(), Ordering::Relaxed);
    VALID.store(true, Ordering::Release);
}

/// The last reading if it is younger than [`MAX_AGE_SECONDS`].
pub fn current() -> Option<Reading> {
    if !VALID.load(Ordering::Acquire) {
        return None;
    }
    let age = uptime_seconds().saturating_sub(READ_AT.load(Ordering::Relaxed));
    if u64::from(age) > MAX_AGE_SECONDS {
        return None;
    }
    Some(Reading {
        temperature_dc: TEMPERATURE_DC.load(Ordering::Relaxed),
        humidity: HUMIDITY.load(Ordering::Relaxed),
    })
}

#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum SensorError {
    I2c(esp_hal::i2c::master::Error),
    Crc,
}

/// Reads the sensor every minute; failures are logged once until it recovers.
#[embassy_executor::task]
pub async fn indoor_task(mut i2c: I2c<'static, Async>) -> ! {
    let mut failing = false;
    loop {
        match measure(&mut i2c).await {
            Ok(reading) => {
                publish(reading);
                if failing {
                    info!("indoor sensor back");
                    failing = false;
                }
                info!(
                    "indoor: {}.{}°C, {}%",
                    reading.temperature_dc / 10,
                    reading.temperature_dc.unsigned_abs() % 10,
                    reading.humidity
                );
            }
            Err(err) => {
                if !failing {
                    warn!("indoor sensor read failed: {}", err);
                    failing = true;
                }
            }
        }
        Timer::after(READ_INTERVAL).await;
    }
}

async fn measure(i2c: &mut I2c<'static, Async>) -> Result<Reading, SensorError> {
    i2c.write_async(ADDRESS, &MEASURE)
        .await
        .map_err(SensorError::I2c)?;
    Timer::after(MEASURE_TIME).await;
    let mut frame = [0u8; 6];
    i2c.read_async(ADDRESS, &mut frame)
        .await
        .map_err(SensorError::I2c)?;
    parse(&frame)
}

/// Decodes a measurement frame: temperature word, CRC, humidity word, CRC.
pub fn parse(frame: &[u8; 6]) -> Result<Reading, SensorError> {
    if crc8(&frame[0..2]) != frame[2] || crc8(&frame[3..5]) != frame[5] {
        return Err(SensorError::Crc);
    }
    let raw_t = u32::from(u16::from_be_bytes([frame[0], frame[1]]));
    let raw_rh = u32::from(u16::from_be_bytes([frame[3], frame[4]]));
    // T = -45 + 175 × raw / 65535 and RH = 100 × raw / 65535, rounded.
    // Both products fit in u32 for any 16-bit raw value.
    let tenths = (1750 * raw_t + 65535 / 2) / 65535;
    let humidity = (100 * raw_rh + 65535 / 2) / 65535;
    Ok(Reading {
        // At most 1750, so the casts are lossless.
        temperature_dc: tenths as i32 - 450,
        humidity: humidity as u8,
    })
}

/// CRC-8 of the SHT3x datasheet: polynomial 0x31, initial value 0xFF.
fn crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0xFFu8;
    for &byte in bytes {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x31
            } else {
                crc << 1
            };
        }
    }
    crc
}
