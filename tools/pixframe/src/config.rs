//! Device configuration: the TOML file edited by the user and its NVS
//! representation read by the firmware.
//!
//! The NVS layout (namespace, keys, types) is the contract with
//! `firmware/src/config.rs`; change both together. Keys are limited to 15
//! characters by the NVS format.

use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use esp_nvs::error::Error;
use esp_nvs::mem_flash::MemFlash;
use esp_nvs::{Get, Key, Nvs};
use serde::Deserialize;

/// NVS namespace holding every pix-frame key.
pub const NAMESPACE: &str = "pixframe";
/// Bumped when a key changes meaning; the firmware refuses unknown versions.
pub const SCHEMA_VERSION: u8 = 1;

pub const KEY_VERSION: &str = "version";
pub const KEY_INFO_EVERY: &str = "info_every";
pub const KEY_WIFI_SSID: &str = "wifi_ssid";
pub const KEY_WIFI_PASSWORD: &str = "wifi_pass";
pub const KEY_LATITUDE_E6: &str = "lat_e6";
pub const KEY_LONGITUDE_E6: &str = "lon_e6";
pub const KEY_TIMEZONE: &str = "timezone";

/// 802.11 limits.
const MAX_SSID_BYTES: usize = 32;
const MAX_PASSWORD_BYTES: usize = 64;
/// Longest IANA zone name is 32 characters today; leave headroom.
const MAX_TIMEZONE_BYTES: usize = 48;

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub display: Display,
    pub wifi: Option<Wifi>,
    pub location: Option<Location>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Display {
    /// Show an information screen after this many animations; 0 disables them.
    #[serde(default)]
    pub info_every: u8,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Wifi {
    pub ssid: String,
    pub password: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    /// IANA time zone name, e.g. `Europe/Paris`.
    pub timezone: String,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text).context("invalid configuration")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if let Some(wifi) = &self.wifi {
            if wifi.ssid.is_empty() || wifi.ssid.len() > MAX_SSID_BYTES {
                bail!("wifi.ssid must be 1 to {MAX_SSID_BYTES} bytes");
            }
            if wifi.password.len() > MAX_PASSWORD_BYTES {
                bail!("wifi.password must be at most {MAX_PASSWORD_BYTES} bytes");
            }
        }
        if let Some(location) = &self.location {
            if !(-90.0..=90.0).contains(&location.latitude) {
                bail!("location.latitude must be between -90 and 90");
            }
            if !(-180.0..=180.0).contains(&location.longitude) {
                bail!("location.longitude must be between -180 and 180");
            }
            if location.timezone.is_empty() || location.timezone.len() > MAX_TIMEZONE_BYTES {
                bail!("location.timezone must be 1 to {MAX_TIMEZONE_BYTES} bytes");
            }
        }
        Ok(())
    }

    /// Builds a fresh NVS partition image of `size` bytes holding this configuration.
    pub fn to_nvs_image(&self, size: usize) -> Result<Vec<u8>> {
        let sector = esp_nvs::FLASH_SECTOR_SIZE;
        if size == 0 || !size.is_multiple_of(sector) {
            bail!("partition size must be a non-zero multiple of {sector} bytes");
        }
        let mut nvs =
            Nvs::new(0, size, MemFlash::new(size / sector)).map_err(|e| nvs_error("init", e))?;
        let ns = Key::from_str(NAMESPACE);
        let set_str = |nvs: &mut Nvs<MemFlash>, key: &str, value: &str| {
            nvs.set(&ns, &Key::from_str(key), value)
                .map_err(|e| nvs_error(key, e))
        };

        nvs.set(&ns, &Key::from_str(KEY_VERSION), SCHEMA_VERSION)
            .map_err(|e| nvs_error(KEY_VERSION, e))?;
        nvs.set(&ns, &Key::from_str(KEY_INFO_EVERY), self.display.info_every)
            .map_err(|e| nvs_error(KEY_INFO_EVERY, e))?;
        if let Some(wifi) = &self.wifi {
            set_str(&mut nvs, KEY_WIFI_SSID, &wifi.ssid)?;
            set_str(&mut nvs, KEY_WIFI_PASSWORD, &wifi.password)?;
        }
        if let Some(location) = &self.location {
            nvs.set(
                &ns,
                &Key::from_str(KEY_LATITUDE_E6),
                to_e6(location.latitude),
            )
            .map_err(|e| nvs_error(KEY_LATITUDE_E6, e))?;
            nvs.set(
                &ns,
                &Key::from_str(KEY_LONGITUDE_E6),
                to_e6(location.longitude),
            )
            .map_err(|e| nvs_error(KEY_LONGITUDE_E6, e))?;
            set_str(&mut nvs, KEY_TIMEZONE, &location.timezone)?;
        }
        Ok(nvs.into_inner().into_inner())
    }

    /// Reads a configuration back from an NVS image, for `config dump` and tests.
    pub fn from_nvs_image(image: Vec<u8>) -> Result<Self> {
        let sector = esp_nvs::FLASH_SECTOR_SIZE;
        if image.is_empty() || !image.len().is_multiple_of(sector) {
            bail!("image size must be a non-zero multiple of {sector} bytes");
        }
        let size = image.len();
        let mut nvs =
            Nvs::new(0, size, MemFlash::from_bytes(image)).map_err(|e| nvs_error("init", e))?;
        let ns = Key::from_str(NAMESPACE);
        let version: u8 = nvs
            .get(&ns, &Key::from_str(KEY_VERSION))
            .map_err(|e| nvs_error(KEY_VERSION, e))?;
        if version != SCHEMA_VERSION {
            bail!("unsupported configuration version {version}");
        }
        let info_every: u8 = nvs
            .get(&ns, &Key::from_str(KEY_INFO_EVERY))
            .map_err(|e| nvs_error(KEY_INFO_EVERY, e))?;
        let wifi = match optional::<String>(&mut nvs, &ns, KEY_WIFI_SSID)? {
            Some(ssid) => Some(Wifi {
                ssid,
                password: optional(&mut nvs, &ns, KEY_WIFI_PASSWORD)?.unwrap_or_default(),
            }),
            None => None,
        };
        let location = match optional::<String>(&mut nvs, &ns, KEY_TIMEZONE)? {
            Some(timezone) => Some(Location {
                latitude: from_e6(optional(&mut nvs, &ns, KEY_LATITUDE_E6)?.unwrap_or(0)),
                longitude: from_e6(optional(&mut nvs, &ns, KEY_LONGITUDE_E6)?.unwrap_or(0)),
                timezone,
            }),
            None => None,
        };
        Ok(Self {
            display: Display { info_every },
            wifi,
            location,
        })
    }

    /// Human-readable summary; the Wi-Fi password is never printed.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "info screen every: {}", self.display.info_every);
        match &self.wifi {
            Some(wifi) => {
                let _ = writeln!(
                    out,
                    "wifi: {} (password: {} bytes)",
                    wifi.ssid,
                    wifi.password.len()
                );
            }
            None => {
                let _ = writeln!(out, "wifi: not configured");
            }
        }
        match &self.location {
            Some(location) => {
                let _ = writeln!(
                    out,
                    "location: {:.4}, {:.4} ({})",
                    location.latitude, location.longitude, location.timezone
                );
            }
            None => {
                let _ = writeln!(out, "location: not configured");
            }
        }
        out
    }
}

fn optional<T>(nvs: &mut Nvs<MemFlash>, ns: &Key, key: &str) -> Result<Option<T>>
where
    Nvs<MemFlash>: Get<T>,
{
    match nvs.get(ns, &Key::from_str(key)) {
        Ok(value) => Ok(Some(value)),
        Err(Error::KeyNotFound) => Ok(None),
        Err(e) => Err(nvs_error(key, e)),
    }
}

fn nvs_error(what: &str, error: Error) -> anyhow::Error {
    anyhow::anyhow!("nvs {what}: {error:?}")
}

/// Degrees to micro-degrees; the range is validated so the cast cannot overflow.
fn to_e6(degrees: f64) -> i32 {
    (degrees * 1_000_000.0).round() as i32
}

fn from_e6(micro: i32) -> f64 {
    f64::from(micro) / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
[display]
info_every = 5
[wifi]
ssid = "Home"
password = "hunter2!"
[location]
latitude = 48.8566
longitude = 2.3522
timezone = "Europe/Paris"
"#;

    #[test]
    fn roundtrips_through_nvs() {
        let config = Config::parse(FULL).unwrap();
        let image = config.to_nvs_image(0x6000).unwrap();
        assert_eq!(image.len(), 0x6000);
        let back = Config::from_nvs_image(image).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn minimal_config_has_no_optional_sections() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.display.info_every, 0);
        let back = Config::from_nvs_image(config.to_nvs_image(0x3000).unwrap()).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn rejects_bad_values() {
        assert!(Config::parse("[wifi]\nssid = \"\"\npassword = \"\"").is_err());
        assert!(
            Config::parse("[location]\nlatitude = 91\nlongitude = 0\ntimezone = \"UTC\"").is_err()
        );
        assert!(Config::parse("[display]\ninfo_every = 300").is_err());
        assert!(Config::parse("[unknown]\nx = 1").is_err());
    }

    #[test]
    fn rejects_blank_image() {
        assert!(Config::from_nvs_image(vec![0xFF; 0x3000]).is_err());
    }

    #[test]
    fn summary_hides_the_password() {
        let summary = Config::parse(FULL).unwrap().summary();
        assert!(summary.contains("Home"));
        assert!(!summary.contains("hunter2"));
    }
}
