//! Device configuration read from the `nvs` partition at boot.
//!
//! The image is produced on the PC by `pixframe config build`; the namespace,
//! keys and types below are the contract with `tools/pixframe/src/config.rs`
//! and must change together. Everything except `info_every` is optional:
//! features whose section is missing simply stay off, display settings
//! whose key is missing keep their default.

use alloc::string::String;

use smart_leds::RGB8;

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
const KEY_LIGHT_DARK_MV: &str = "light_dark_mv";
const KEY_LIGHT_BRIGHT_MV: &str = "light_bright_mv";
const KEY_LIGHT_MIN: &str = "light_min";
const KEY_LIGHT_MAX: &str = "light_max";
const KEY_MIN_PLAY_MS: &str = "min_play_ms";
const KEY_TEMP_UNIT: &str = "temp_unit";
const KEY_HOUR_COLOR: &str = "hour_color";
const KEY_MINUTE_COLOR: &str = "minute_color";

/// An animation loops until at least this long has been scheduled.
const DEFAULT_MIN_PLAY_MS: u32 = 3_000;

#[derive(Debug)]
pub struct Config {
    /// Show an information screen after this many animations; 0 disables them.
    pub info_every: u8,
    /// An animation loops until at least this many milliseconds have been
    /// scheduled at a loop boundary.
    pub min_play_ms: u32,
    pub style: Style,
    pub wifi: Option<Wifi>,
    pub location: Option<Location>,
    pub light: Option<Light>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            info_every: 0,
            min_play_ms: DEFAULT_MIN_PLAY_MS,
            style: Style::default(),
            wifi: None,
            location: None,
            light: None,
        }
    }
}

/// How information screens look.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub temperature_unit: TemperatureUnit,
    pub hour_color: RGB8,
    pub minute_color: RGB8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            temperature_unit: TemperatureUnit::Celsius,
            hour_color: RGB8::new(255, 40, 130),
            minute_color: RGB8::new(0, 210, 255),
        }
    }
}

/// Stored as a `u8`: 0 Celsius, 1 Fahrenheit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
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

/// Ambient light calibration: sensor voltages mapped to the brightness range.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// Reading at or below which the matrix runs at `min_brightness`.
    pub dark_mv: u16,
    /// Reading at or above which the matrix runs at `max_brightness`.
    pub bright_mv: u16,
    pub min_brightness: u8,
    pub max_brightness: u8,
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
        let min_play_ms = optional(nvs, &ns, KEY_MIN_PLAY_MS)?.unwrap_or(DEFAULT_MIN_PLAY_MS);
        let mut style = Style::default();
        if let Some(unit) = optional::<P, u8>(nvs, &ns, KEY_TEMP_UNIT)? {
            style.temperature_unit = match unit {
                1 => TemperatureUnit::Fahrenheit,
                _ => TemperatureUnit::Celsius,
            };
        }
        if let Some(rgb) = optional(nvs, &ns, KEY_HOUR_COLOR)? {
            style.hour_color = unpack_rgb(rgb);
        }
        if let Some(rgb) = optional(nvs, &ns, KEY_MINUTE_COLOR)? {
            style.minute_color = unpack_rgb(rgb);
        }
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
        let light = match optional::<P, u16>(nvs, &ns, KEY_LIGHT_DARK_MV)? {
            Some(dark_mv) => Some(Light {
                dark_mv,
                bright_mv: optional(nvs, &ns, KEY_LIGHT_BRIGHT_MV)?.unwrap_or(u16::MAX),
                min_brightness: optional(nvs, &ns, KEY_LIGHT_MIN)?.unwrap_or(0),
                max_brightness: optional(nvs, &ns, KEY_LIGHT_MAX)?.unwrap_or(u8::MAX),
            }),
            None => None,
        };

        Ok(Self {
            info_every,
            min_play_ms,
            style,
            wifi,
            location,
            light,
        })
    }
}

/// `0xRRGGBB` to a color.
fn unpack_rgb(rgb: u32) -> RGB8 {
    let [_, r, g, b] = rgb.to_be_bytes();
    RGB8::new(r, g, b)
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
