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
pub const KEY_LIGHT_DARK_MV: &str = "light_dark_mv";
pub const KEY_LIGHT_BRIGHT_MV: &str = "light_bright_mv";
pub const KEY_LIGHT_MIN: &str = "light_min";
pub const KEY_LIGHT_MAX: &str = "light_max";
pub const KEY_MIN_PLAY_MS: &str = "min_play_ms";
pub const KEY_TEMP_UNIT: &str = "temp_unit";
pub const KEY_HOUR_COLOR: &str = "hour_color";
pub const KEY_MINUTE_COLOR: &str = "minute_color";

/// 802.11 limits.
const MAX_SSID_BYTES: usize = 32;
const MAX_PASSWORD_BYTES: usize = 64;
/// Longest IANA zone name is 32 characters today; leave headroom.
const MAX_TIMEZONE_BYTES: usize = 48;
/// Upper bound of `display.min_play_seconds`, ten minutes.
const MAX_MIN_PLAY_SECONDS: f64 = 600.0;

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub display: Display,
    pub clock: Option<Clock>,
    pub wifi: Option<Wifi>,
    pub location: Option<Location>,
    pub light: Option<Light>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Display {
    /// Show an information screen after this many animations; 0 disables them.
    #[serde(default)]
    pub info_every: u8,
    /// An animation loops until at least this long has been scheduled;
    /// the firmware default applies when unset.
    pub min_play_seconds: Option<f64>,
    /// Unit of every temperature shown; Celsius when unset.
    pub temperature_unit: Option<TemperatureUnit>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum TemperatureUnit {
    #[serde(rename = "C")]
    Celsius,
    #[serde(rename = "F")]
    Fahrenheit,
}

impl TemperatureUnit {
    /// NVS encoding, shared with the firmware.
    fn to_u8(self) -> u8 {
        match self {
            Self::Celsius => 0,
            Self::Fahrenheit => 1,
        }
    }

    fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Celsius),
            1 => Ok(Self::Fahrenheit),
            _ => bail!("unknown temperature unit {value}"),
        }
    }
}

/// Clock screen colors; the firmware defaults apply to unset ones.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    pub hour_color: Option<Color>,
    pub minute_color: Option<Color>,
}

/// `#RRGGBB` in the TOML file, `0xRRGGBB` in NVS.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(try_from = "String")]
pub struct Color(pub u32);

impl TryFrom<String> for Color {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.strip_prefix('#')
            .filter(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map(Self)
            .ok_or_else(|| format!("invalid color {text:?}, expected \"#RRGGBB\""))
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:06X}", self.0)
    }
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

/// Ambient light calibration, read from the firmware logs on the assembled
/// frame: sensor voltages mapped linearly onto the brightness range.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Light {
    /// Reading at or below which the matrix runs at `min_brightness`.
    pub dark_mv: u16,
    /// Reading at or above which the matrix runs at `max_brightness`.
    pub bright_mv: u16,
    pub min_brightness: u8,
    pub max_brightness: u8,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text).context("invalid configuration")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if let Some(seconds) = self.display.min_play_seconds
            && !(0.0..=MAX_MIN_PLAY_SECONDS).contains(&seconds)
        {
            bail!("display.min_play_seconds must be between 0 and {MAX_MIN_PLAY_SECONDS}");
        }
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
        if let Some(light) = &self.light {
            if light.dark_mv >= light.bright_mv {
                bail!("light.dark_mv must be below light.bright_mv");
            }
            if light.min_brightness > light.max_brightness {
                bail!("light.min_brightness must not exceed light.max_brightness");
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
        if let Some(seconds) = self.display.min_play_seconds {
            nvs.set(&ns, &Key::from_str(KEY_MIN_PLAY_MS), to_ms(seconds))
                .map_err(|e| nvs_error(KEY_MIN_PLAY_MS, e))?;
        }
        if let Some(unit) = self.display.temperature_unit {
            nvs.set(&ns, &Key::from_str(KEY_TEMP_UNIT), unit.to_u8())
                .map_err(|e| nvs_error(KEY_TEMP_UNIT, e))?;
        }
        if let Some(clock) = &self.clock {
            if let Some(color) = clock.hour_color {
                nvs.set(&ns, &Key::from_str(KEY_HOUR_COLOR), color.0)
                    .map_err(|e| nvs_error(KEY_HOUR_COLOR, e))?;
            }
            if let Some(color) = clock.minute_color {
                nvs.set(&ns, &Key::from_str(KEY_MINUTE_COLOR), color.0)
                    .map_err(|e| nvs_error(KEY_MINUTE_COLOR, e))?;
            }
        }
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
        if let Some(light) = &self.light {
            nvs.set(&ns, &Key::from_str(KEY_LIGHT_DARK_MV), light.dark_mv)
                .map_err(|e| nvs_error(KEY_LIGHT_DARK_MV, e))?;
            nvs.set(&ns, &Key::from_str(KEY_LIGHT_BRIGHT_MV), light.bright_mv)
                .map_err(|e| nvs_error(KEY_LIGHT_BRIGHT_MV, e))?;
            nvs.set(&ns, &Key::from_str(KEY_LIGHT_MIN), light.min_brightness)
                .map_err(|e| nvs_error(KEY_LIGHT_MIN, e))?;
            nvs.set(&ns, &Key::from_str(KEY_LIGHT_MAX), light.max_brightness)
                .map_err(|e| nvs_error(KEY_LIGHT_MAX, e))?;
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
        let min_play_seconds = optional::<u32>(&mut nvs, &ns, KEY_MIN_PLAY_MS)?.map(from_ms);
        let temperature_unit = optional::<u8>(&mut nvs, &ns, KEY_TEMP_UNIT)?
            .map(TemperatureUnit::from_u8)
            .transpose()?;
        let hour_color = optional::<u32>(&mut nvs, &ns, KEY_HOUR_COLOR)?.map(Color);
        let minute_color = optional::<u32>(&mut nvs, &ns, KEY_MINUTE_COLOR)?.map(Color);
        let clock = (hour_color.is_some() || minute_color.is_some()).then_some(Clock {
            hour_color,
            minute_color,
        });
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
        let light = match optional::<u16>(&mut nvs, &ns, KEY_LIGHT_DARK_MV)? {
            Some(dark_mv) => Some(Light {
                dark_mv,
                bright_mv: optional(&mut nvs, &ns, KEY_LIGHT_BRIGHT_MV)?.unwrap_or(u16::MAX),
                min_brightness: optional(&mut nvs, &ns, KEY_LIGHT_MIN)?.unwrap_or(0),
                max_brightness: optional(&mut nvs, &ns, KEY_LIGHT_MAX)?.unwrap_or(u8::MAX),
            }),
            None => None,
        };
        Ok(Self {
            display: Display {
                info_every,
                min_play_seconds,
                temperature_unit,
            },
            clock,
            wifi,
            location,
            light,
        })
    }

    /// Human-readable summary; the Wi-Fi password is never printed.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "info screen every: {}", self.display.info_every);
        match self.display.min_play_seconds {
            Some(seconds) => {
                let _ = writeln!(out, "min play time: {seconds} s");
            }
            None => {
                let _ = writeln!(out, "min play time: firmware default");
            }
        }
        let unit = match self.display.temperature_unit {
            Some(TemperatureUnit::Fahrenheit) => "F",
            Some(TemperatureUnit::Celsius) | None => "C",
        };
        let _ = writeln!(out, "temperature unit: {unit}");
        let color = |color: Option<Color>| color.map_or("default".to_string(), |c| c.to_string());
        let clock = self.clock.as_ref();
        let _ = writeln!(
            out,
            "clock colors: hours {}, minutes {}",
            color(clock.and_then(|c| c.hour_color)),
            color(clock.and_then(|c| c.minute_color))
        );
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
        match &self.light {
            Some(light) => {
                let _ = writeln!(
                    out,
                    "light: {}..{} mV -> brightness {}..{}",
                    light.dark_mv, light.bright_mv, light.min_brightness, light.max_brightness
                );
            }
            None => {
                let _ = writeln!(out, "light: not calibrated, fixed brightness");
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

/// Seconds to milliseconds; the range is validated so the cast cannot overflow.
fn to_ms(seconds: f64) -> u32 {
    (seconds * 1000.0).round() as u32
}

fn from_ms(ms: u32) -> f64 {
    f64::from(ms) / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r##"
[display]
info_every = 5
min_play_seconds = 4.5
temperature_unit = "F"
[clock]
hour_color = "#FF2882"
minute_color = "#00d2ff"
[wifi]
ssid = "Home"
password = "hunter2!"
[location]
latitude = 48.8566
longitude = 2.3522
timezone = "Europe/Paris"
[light]
dark_mv = 400
bright_mv = 2500
min_brightness = 8
max_brightness = 60
"##;

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
        assert!(Config::parse("[display]\nmin_play_seconds = -1").is_err());
        assert!(Config::parse("[display]\ntemperature_unit = \"K\"").is_err());
        assert!(Config::parse("[clock]\nhour_color = \"red\"").is_err());
        assert!(Config::parse("[clock]\nhour_color = \"#12345\"").is_err());
        assert!(
            Config::parse(
                "[light]\ndark_mv = 2500\nbright_mv = 400\nmin_brightness = 8\nmax_brightness = 60"
            )
            .is_err()
        );
        assert!(
            Config::parse(
                "[light]\ndark_mv = 400\nbright_mv = 2500\nmin_brightness = 60\nmax_brightness = 8"
            )
            .is_err()
        );
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
