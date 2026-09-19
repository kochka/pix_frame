//! WS2812-style LED transport over SPI with DMA.
//!
//! Each LED bit becomes four SPI bits at 3.2 MHz: `1000` for a 0 and `1100`
//! for a 1, which reproduces the 312 ns / 625 ns high pulses the LEDs
//! expect. The whole frame is one DMA transfer, so nothing running on the
//! CPU (scheduler, Wi-Fi driver, interrupts) can disturb the timing. Only
//! MOSI is used; it is routed to the data pin through the GPIO matrix.

use alloc::vec;
use alloc::vec::Vec;

use esp_hal::Blocking;
use esp_hal::dma::DmaBufError;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config, ConfigError, Spi, SpiDma};
use esp_hal::time::Rate;
use smart_leds::{RGB8, SmartLedsWrite};

/// SPI clock; one LED bit is four SPI bits, so 3.2 MHz gives 800 kbit/s.
const SPI_FREQUENCY_KHZ: u32 = 3_200;
/// Encoded bytes per LED: 24 bits × 4 SPI bits / 8.
const BYTES_PER_LED: usize = 12;
/// Zero bytes after the frame: 120 × 8 × 312.5 ns = 300 µs of low level,
/// the reset the LEDs need before latching.
const RESET_BYTES: usize = 120;

#[derive(Debug)]
pub enum Error {
    Config(ConfigError),
    Dma(DmaBufError),
    Spi(esp_hal::spi::Error),
    /// More LEDs than the frame buffer holds.
    TooManyLeds,
}

pub struct SpiLeds {
    spi: SpiDma<'static, Blocking>,
    frame: Vec<u8>,
    led_count: usize,
}

impl SpiLeds {
    /// Sets up SPI and DMA for `led_count` LEDs driven from `data_pin`.
    pub fn new(
        spi: esp_hal::peripherals::SPI2<'static>,
        dma: esp_hal::peripherals::DMA_CH0<'static>,
        data_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        led_count: usize,
    ) -> Result<Self, Error> {
        let config = Config::default()
            .with_frequency(Rate::from_khz(SPI_FREQUENCY_KHZ))
            .with_mode(Mode::_0);
        let frame_len = led_count * BYTES_PER_LED + RESET_BYTES;
        let rx_buf = esp_hal::dma_rx_buffer!(32).map_err(Error::Dma)?;
        let tx_buf = esp_hal::dma_tx_buffer!(4096).map_err(Error::Dma)?;
        let spi = Spi::new(spi, config)
            .map_err(Error::Config)?
            .with_mosi(data_pin)
            .with_dma(dma)
            .with_buffers(rx_buf, tx_buf);
        Ok(Self {
            spi,
            frame: vec![0; frame_len],
            led_count,
        })
    }

    /// Encodes one LED byte into four SPI bytes, most significant bit first.
    fn encode_byte(value: u8, out: &mut [u8]) {
        for (pair, slot) in out.iter_mut().enumerate() {
            let high = (value >> (7 - 2 * pair)) & 1;
            let low = (value >> (6 - 2 * pair)) & 1;
            *slot = (nibble(high) << 4) | nibble(low);
        }
    }
}

/// SPI pattern of one LED bit.
const fn nibble(bit: u8) -> u8 {
    if bit == 1 { 0b1100 } else { 0b1000 }
}

impl SmartLedsWrite for SpiLeds {
    type Error = Error;
    type Color = RGB8;

    /// Encodes the colors in GRB order and sends the frame in one DMA
    /// transfer. Missing LEDs are switched off; extra ones are an error.
    fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
    where
        T: IntoIterator<Item = I>,
        I: Into<RGB8>,
    {
        let led_bytes = self.led_count * BYTES_PER_LED;
        self.frame[..led_bytes].fill(nibble(0) << 4 | nibble(0));
        for (index, color) in iterator.into_iter().enumerate() {
            if index >= self.led_count {
                return Err(Error::TooManyLeds);
            }
            let color: RGB8 = color.into();
            let base = index * BYTES_PER_LED;
            Self::encode_byte(color.g, &mut self.frame[base..base + 4]);
            Self::encode_byte(color.r, &mut self.frame[base + 4..base + 8]);
            Self::encode_byte(color.b, &mut self.frame[base + 8..base + 12]);
        }
        self.spi.write(&self.frame).map_err(Error::Spi)
    }
}
