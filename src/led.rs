use embedded_hal::digital::OutputPin;
use rppal::spi::{Bus, Mode as SpiMode, SlaveSelect, Spi};
use smart_leds::{RGB8, SmartLedsWrite};
use ws2812_spi::hosted::Ws2812;

use crate::ContactState;

/// A single RGB LED controlled by three digital output pins.
pub struct RgbLed<P: OutputPin> {
  red: P,
  green: P,
  blue: P,
}

impl<P: OutputPin> RgbLed<P> {
  /// Creates an `RgbLed` from the red, green, and blue output pins.
  pub fn new(red: P, green: P, blue: P) -> Self {
    Self { red, green, blue }
  }

  /// Sets the LED to color.
  pub fn set_color(&mut self, color: RGB8) {
    let _ = self.red.set_state((color.r != 0).into());
    let _ = self.green.set_state((color.g != 0).into());
    let _ = self.blue.set_state((color.b != 0).into());
  }
}

pub struct RgbRing {
  inner: Ws2812<Spi>,
  colors: [RGB8; 12],
}

impl RgbRing {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    // On Raspberry Pi, `core_freq=250` must be set in `/boot/config.txt` in order to have a stable SPI frequency.
    let spi_freq = 800_000 * 3;

    let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, spi_freq, SpiMode::Mode0).unwrap();
    let ws2812 = Ws2812::new(spi);

    Self { inner: ws2812, colors: Default::default() }
  }

  pub fn set_top_right(&mut self, color: RGB8) {
    self.colors[0..=2].fill(color);
  }

  pub fn set_bottom_right(&mut self, color: RGB8) {
    self.colors[3..=5].fill(color);
  }

  pub fn set_bottom_left(&mut self, color: RGB8) {
    self.colors[6..=8].fill(color);
  }

  pub fn set_top_left(&mut self, color: RGB8) {
    self.colors[9..=11].fill(color);
  }

  pub fn render(&mut self) {
    self.inner.write(self.colors.iter().cloned()).unwrap();
  }
}

pub fn state_to_color(state: ContactState) -> RGB8 {
  match state {
    ContactState::Open => RGB8 { r: 0x14, g: 0x00, b: 0x00 },
    ContactState::Closed => RGB8 { r: 0x00, g: 0x0f, b: 0x01 },
  }
}
