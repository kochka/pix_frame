//! Device configuration read from the `nvs` partition at boot.
//!
//! The image is produced on the PC by `pixframe config build`; the namespace,
//! keys and types below are the contract with `tools/pixframe/src/config.rs`
//! and must change together. Everything except `info_every` is optional:
//! features whose section is missing simply stay off.

use alloc::string::String;

use esp_nvs::error::Error as NvsError;
use esp_nvs::platform::Platform;
use esp_nvs::{Get, Key, Nvs};

const NAMESPACE: &str = "pixframe";
const SCHEMA_VERSION: u8 = 1;

const KEY_VERSION: &str = "version";
const KEY_INFO_EVERY: &str = "info_every";
const KEY_WIFI_SSID: &str = "wifi_ssid";
const KEY_WIFI_PASSWORD: &str = "wifi_pass";
const KEY_LATITUDE_E6: &str = "lat_e6";
const KEY_LONGITUDE_E6: &str = "lon_e6";
const KEY_TIMEZONE: &str = "timezone";

#[derive(Debug, Default)]
pub struct Config {
    /// Show an information screen after this many animations; 0 disables them.
    pub info_every: u8,
    pub wifi: Option<Wifi>,
    pub location: Option<Location>,
}

#[derive(Debug)]
pub struct Wifi {
    pub ssid: String,
    pub password: String,
}

#[derive(Clone, Debug)]
pub struct Location {
    /// Degrees × 1,000,000.
    pub latitude_e6: i32,
    pub longitude_e6: i32,
    /// IANA time zone name.
    pub timezone: String,
}

#[derive(Debug, defmt::Format)]
pub enum ConfigError {
    /// No configuration has been written to the device.
    NotConfigured,
    /// The image was written by an incompatible tool version.
    UnsupportedVersion(u8),
    /// A key exists but could not be read.
    Nvs { key: &'static str, error: NvsError },
}

impl Config {
    /// Reads the configuration; a missing namespace or version key means the
    /// device has never been configured.
    pub fn load<P: Platform>(nvs: &mut Nvs<P>) -> Result<Self, ConfigError> {
        let ns = Key::from_str(NAMESPACE);
        let version: u8 = match nvs.get(&ns, &Key::from_str(KEY_VERSION)) {
            Ok(version) => version,
            Err(NvsError::KeyNotFound | NvsError::NamespaceNotFound) => {
                return Err(ConfigError::NotConfigured);
            }
            Err(error) => {
                return Err(ConfigError::Nvs {
                    key: KEY_VERSION,
                    error,
                });
            }
        };
        if version != SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedVersion(version));
        }

        let info_every = optional(nvs, &ns, KEY_INFO_EVERY)?.unwrap_or(0);
        let wifi = match optional::<P, String>(nvs, &ns, KEY_WIFI_SSID)? {
            Some(ssid) => Some(Wifi {
                ssid,
                password: optional(nvs, &ns, KEY_WIFI_PASSWORD)?.unwrap_or_default(),
            }),
            None => None,
        };
        let location = match optional::<P, String>(nvs, &ns, KEY_TIMEZONE)? {
            Some(timezone) => Some(Location {
                latitude_e6: optional(nvs, &ns, KEY_LATITUDE_E6)?.unwrap_or(0),
                longitude_e6: optional(nvs, &ns, KEY_LONGITUDE_E6)?.unwrap_or(0),
                timezone,
            }),
            None => None,
        };

        Ok(Self {
            info_every,
            wifi,
            location,
        })
    }
}

fn optional<P: Platform, T>(
    nvs: &mut Nvs<P>,
    ns: &Key,
    key: &'static str,
) -> Result<Option<T>, ConfigError>
where
    Nvs<P>: Get<T>,
{
    match nvs.get(ns, &Key::from_str(key)) {
        Ok(value) => Ok(Some(value)),
        Err(NvsError::KeyNotFound) => Ok(None),
        Err(error) => Err(ConfigError::Nvs { key, error }),
    }
}
