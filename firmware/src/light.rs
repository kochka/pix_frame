//! Matrix brightness following the room light, from a GL5528 photoresistor
//! divider on GPIO2 (ADC1). See `doc/wiring.md` section 7.
//!
//! The sensor voltage is always measured and logged, so the calibration
//! thresholds can be read on the assembled frame. Brightness only follows it
//! once a calibration is configured; until then [`brightness`] returns
//! `None` and the caller keeps its fixed value.

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use defmt::{info, warn};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::Blocking;
use esp_hal::analog::adc::{Adc, AdcCalCurve, AdcConfig, AdcPin, Attenuation};
use esp_hal::peripherals::{ADC1, GPIO2};

use crate::config::Light;

/// One filter update and one brightness step per tick: a full 8 → 60 swing
/// takes about 1.3 s.
const TICK: Duration = Duration::from_millis(25);
/// Conversions averaged per tick.
const SAMPLES: u32 = 4;
/// Exponential filter weight, 1/2^n per tick: n = 5 smooths over about
/// 32 ticks (0.8 s), so a hand or a passing shadow is ignored.
const FILTER_SHIFT: u32 = 5;
/// The target only moves when the computed brightness differs by at least
/// this many levels, so a reading sitting on a boundary does not flicker.
const HYSTERESIS: u8 = 3;
const LOG_INTERVAL: Duration = Duration::from_secs(10);

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LEVEL: AtomicU8 = AtomicU8::new(0);

/// Current brightness, or `None` while the sensor is not calibrated.
pub fn brightness() -> Option<u8> {
    ACTIVE
        .load(Ordering::Acquire)
        .then(|| LEVEL.load(Ordering::Relaxed))
}

pub struct Sensor {
    adc: Adc<'static, ADC1<'static>, Blocking>,
    pin: AdcPin<GPIO2<'static>, ADC1<'static>, AdcCalCurve<ADC1<'static>>>,
}

impl Sensor {
    /// 11 dB attenuation covers the whole 0–3.3 V divider output; the curve
    /// calibration from eFuse returns millivolts.
    pub fn new(adc1: ADC1<'static>, pin: GPIO2<'static>) -> Self {
        let mut config = AdcConfig::new();
        let pin =
            config.enable_pin_with_cal::<_, AdcCalCurve<ADC1<'static>>>(pin, Attenuation::_11dB);
        Self {
            adc: Adc::new(adc1, config),
            pin,
        }
    }

    /// Mean of [`SAMPLES`] conversions, in millivolts. A conversion takes
    /// about 50 µs, short enough to wait for without yielding, and to run in
    /// a critical section so the radio cannot take the SAR power back midway.
    fn read_mv(&mut self) -> u16 {
        let mut sum = 0u32;
        for _ in 0..SAMPLES {
            let mv = critical_section::with(|_| {
                force_sar_power(true);
                let mv = loop {
                    if let Ok(mv) = self.adc.read_oneshot(&mut self.pin) {
                        break mv;
                    }
                };
                force_sar_power(false);
                mv
            });
            sum += u32::from(mv);
        }
        // The mean of u16 values fits in u16.
        (sum / SAMPLES) as u16
    }
}

/// Measures the room light and, when `calibration` is set, moves the
/// published brightness one level per tick towards the mapped value.
#[embassy_executor::task]
pub async fn light_task(mut sensor: Sensor, calibration: Option<Light>) -> ! {
    let calibration = calibration.filter(|light| {
        let valid = light.dark_mv < light.bright_mv && light.min_brightness <= light.max_brightness;
        if !valid {
            warn!("invalid light calibration, brightness stays fixed");
        }
        valid
    });
    if calibration.is_none() {
        info!("light sensor not calibrated, brightness stays fixed");
    }

    // Filtered voltage with FILTER_SHIFT fractional bits, seeded with a
    // first reading so the filter does not start from darkness.
    let first = sensor.read_mv();
    let mut filtered = u32::from(first) << FILTER_SHIFT;
    let mut target = calibration.map_or(0, |light| map(first, &light));
    let mut level = target;
    LEVEL.store(level, Ordering::Relaxed);
    ACTIVE.store(calibration.is_some(), Ordering::Release);
    let mut next_log = Instant::now();

    loop {
        Timer::after(TICK).await;
        let mv = u32::from(sensor.read_mv());
        filtered = filtered - (filtered >> FILTER_SHIFT) + mv;
        // At most u16::MAX after the shift.
        let mv = (filtered >> FILTER_SHIFT) as u16;

        if let Some(light) = &calibration {
            let mapped = map(mv, light);
            let at_limit = mapped == light.min_brightness || mapped == light.max_brightness;
            if mapped.abs_diff(target) >= HYSTERESIS || (at_limit && mapped != target) {
                target = mapped;
            }
            if level != target {
                level = if level < target { level + 1 } else { level - 1 };
                LEVEL.store(level, Ordering::Relaxed);
            }
        }

        if Instant::now() >= next_log {
            next_log += LOG_INTERVAL;
            match calibration {
                Some(_) => info!("light: {} mV, brightness {}", mv, level),
                None => info!("light: {} mV", mv),
            }
        }
    }
}

/// Linear map of the sensor voltage from `dark_mv..=bright_mv` onto
/// `min_brightness..=max_brightness`, clamped at both ends. The calibration
/// must be valid: `dark_mv < bright_mv` and `min_brightness <= max_brightness`.
fn map(mv: u16, light: &Light) -> u8 {
    let span = u32::from(light.bright_mv - light.dark_mv);
    let offset = u32::from(mv.clamp(light.dark_mv, light.bright_mv) - light.dark_mv);
    let range = u32::from(light.max_brightness - light.min_brightness);
    // range × offset ≤ 255 × 65535 fits in u32; the quotient is ≤ range.
    light.min_brightness + ((range * offset + span / 2) / span) as u8
}

/// Forces the SAR ADC on for a conversion, or hands it back to the radio.
///
/// Radio initialisation hands SAR power to the PHY's power detector, which
/// powers the SAR down between its own measurements: conversions then
/// saturate at full scale whatever the input. Like ESP-IDF's oneshot driver,
/// this forces the power on through `PWDET_CONF` around each conversion, then
/// releases it: left forced, the power detector cannot run its own cycle and
/// the transmit power control suffers (first connection attempts fail).
/// Workaround for esp-hal 1.2, whose ADC driver does not do it; remove once a
/// release includes https://github.com/esp-rs/esp-hal/pull/6412.
///
/// See `sar_ctrl_ll_set_power_mode_from_pwdet` in ESP-IDF
/// `components/hal/esp32c6/include/hal/sar_ctrl_ll.h`.
fn force_sar_power(on: bool) {
    // The PAC does not describe the power detector.
    const PWDET_CONF: *mut u32 = 0x600A_0810 as *mut u32;
    const SAR_POWER_FORCE: u32 = 1 << 24;
    const SAR_POWER_CNTL: u32 = 1 << 23;

    // PWDET_CONF is clocked by the modem front-end APB clock, which the radio
    // gates while its PHY is off; writes are dropped while it is gated. The
    // clock is enabled only around the write, inside a critical section so
    // the radio finds it the way it left it.
    critical_section::with(|_| {
        let clk_conf1 = esp_hal::peripherals::MODEM_SYSCON::regs().clk_conf1();
        let fe_apb_clock_enabled = clk_conf1.read().clk_fe_apb_en().bit();
        clk_conf1.modify(|_, w| w.clk_fe_apb_en().set_bit());
        // SAFETY: PWDET_CONF is a valid, always-mapped 32-bit register on the
        // ESP32-C6, and only the two SAR power bits are changed.
        unsafe {
            let conf = PWDET_CONF.read_volatile();
            let conf = if on {
                conf | SAR_POWER_FORCE | SAR_POWER_CNTL
            } else {
                // Power detector (FSM) control, the radio's default.
                conf & !(SAR_POWER_FORCE | SAR_POWER_CNTL)
            };
            PWDET_CONF.write_volatile(conf);
        }
        clk_conf1.modify(|_, w| w.clk_fe_apb_en().bit(fe_apb_clock_enabled));
    });
}
