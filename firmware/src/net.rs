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
use crate::screens::WifiStatus;
use crate::{sntp, tz};

const NTP_SERVER: &str = "pool.ntp.org";
const SYNC_INTERVAL: Duration = Duration::from_secs(60 * 60);
const SYNC_RETRY: Duration = Duration::from_secs(30);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

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
