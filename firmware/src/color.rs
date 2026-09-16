use esp_hal::rng::Rng;
use smart_leds::{
    RGB8,
    hsv::{Hsv, hsv2rgb},
};

pub fn random_vivid(max_value: u8) -> RGB8 {
    let hue = Rng::new().random() as u8;

    hsv2rgb(Hsv {
        hue,
        sat: 255,
        val: max_value,
    })
}
