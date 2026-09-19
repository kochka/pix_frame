//! Wi-Fi station, network stack and time synchronisation tasks.
//!
//! Everything here is optional: the tasks are only spawned when the
//! configuration has a `[wifi]` section. The connection status is published
//! through an atomic so the playback loop and the self-test can show it.

use alloc::string::String;
use core::sync::atomic::{AtomicU8, Ordering};

use defmt::{info, warn};
use embassy_net::{Runner, Stack};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{Interface, WifiController};

use crate::clock::Clock;
use crate::config::Location;
use crate::screens::WifiStatus;
use crate::{http, sntp, tz, weather};

const NTP_SERVER: &str = "pool.ntp.org";
const SYNC_INTERVAL: Duration = Duration::from_secs(60 * 60);
const SYNC_RETRY: Duration = Duration::from_secs(30);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const WEATHER_INTERVAL: Duration = Duration::from_secs(15 * 60);
const WEATHER_RETRY: Duration = Duration::from_secs(60);
/// Open-Meteo replies with the requested fields are well under this.
const WEATHER_BUFFER: usize = 2048;

const NO_ANNOUNCEMENT: u8 = u8::MAX;

static STATUS: AtomicU8 = AtomicU8::new(WifiStatus::Off as u8);
static ANNOUNCEMENT: AtomicU8 = AtomicU8::new(NO_ANNOUNCEMENT);

/// Current connection status.
pub fn wifi_status() -> WifiStatus {
    WifiStatus::from_u8(STATUS.load(Ordering::Relaxed))
}

/// The first definitive status (connected or failed), reported once so the
/// playback loop can flash it on the matrix after boot.
pub fn take_announcement() -> Option<WifiStatus> {
    let value = ANNOUNCEMENT.swap(NO_ANNOUNCEMENT, Ordering::Relaxed);
    (value != NO_ANNOUNCEMENT).then(|| WifiStatus::from_u8(value))
}

fn set_status(status: WifiStatus) {
    STATUS.store(status as u8, Ordering::Relaxed);
}

/// Drives the network stack; never returns.
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, Interface>) -> ! {
    runner.run().await
}

/// Keeps the station connected, reconnecting after failures and drops.
#[embassy_executor::task]
pub async fn wifi_task(mut controller: WifiController<'static>) -> ! {
    let mut announced = false;
    loop {
        set_status(WifiStatus::Connecting);
        match controller.connect_async().await {
            Ok(_) => {
                info!("wifi connected");
                set_status(WifiStatus::Connected);
                if !announced {
                    announced = true;
                    ANNOUNCEMENT.store(WifiStatus::Connected as u8, Ordering::Relaxed);
                }
                match controller.wait_for_disconnect_async().await {
                    Ok(_) => warn!("wifi disconnected"),
                    Err(err) => warn!("wifi disconnected: {:?}", err),
                }
            }
            Err(err) => {
                warn!("wifi connection failed: {:?}", err);
                set_status(WifiStatus::Failed);
                if !announced {
                    announced = true;
                    ANNOUNCEMENT.store(WifiStatus::Failed as u8, Ordering::Relaxed);
                }
            }
        }
        Timer::after(RECONNECT_DELAY).await;
    }
}

/// Sets the clock from SNTP once the network is up, then every hour.
#[embassy_executor::task]
pub async fn time_task(stack: Stack<'static>, clock: Clock, timezone: Option<String>) -> ! {
    loop {
        stack.wait_config_up().await;
        match sntp::query(stack, NTP_SERVER).await {
            Ok(unix) => {
                clock.set_unix_time(unix);
                if let Some(zone) = &timezone {
                    match tz::utc_offset_seconds(zone, unix) {
                        Some(offset) => clock.set_utc_offset(offset),
                        None => warn!("unknown time zone {}, showing UTC", zone.as_str()),
                    }
                }
                if let Some(time) = clock.local_time() {
                    info!(
                        "time synchronised: {:02}:{:02}:{:02} local",
                        time.hour, time.minute, time.second
                    );
                }
                Timer::after(SYNC_INTERVAL).await;
            }
            Err(err) => {
                warn!("time sync failed: {}", err);
                Timer::after(SYNC_RETRY).await;
            }
        }
    }
}

/// Fetches the current weather every quarter hour once the network is up.
/// The reply also carries the UTC offset of the configured zone, which
/// replaces the built-in rules for zones `tz` does not know.
#[embassy_executor::task]
pub async fn weather_task(stack: Stack<'static>, clock: Clock, location: Location) -> ! {
    let Some(path) = weather::request_path(&location) else {
        warn!("weather request does not fit, weather screen disabled");
        loop {
            Timer::after(Duration::from_secs(3600)).await;
        }
    };
    let mut buffer = alloc::vec![0u8; WEATHER_BUFFER];
    loop {
        stack.wait_config_up().await;
        let fetched = http::get(stack, weather::HOST, path.as_str(), &mut buffer)
            .await
            .map_err(WeatherError::Http)
            .and_then(|body| weather::parse(body).map_err(WeatherError::Parse));
        match fetched {
            Ok(observation) => {
                clock.set_utc_offset(observation.utc_offset);
                match clock.unix_time() {
                    Some(now) => {
                        weather::publish(observation, now);
                        info!(
                            "weather: {}.{}°C, {}",
                            observation.temperature_dc / 10,
                            observation.temperature_dc.unsigned_abs() % 10,
                            observation.condition
                        );
                        Timer::after(WEATHER_INTERVAL).await;
                    }
                    None => {
                        // Cannot age the observation without a clock; retry soon.
                        Timer::after(WEATHER_RETRY).await;
                    }
                }
            }
            Err(err) => {
                warn!("weather fetch failed: {}", err);
                Timer::after(WEATHER_RETRY).await;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, defmt::Format)]
enum WeatherError {
    Http(http::HttpError),
    Parse(weather::ParseError),
}
