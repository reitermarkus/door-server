use rppal::gpio::{Gpio, InputPin, OutputPin};
use smart_leds::RGB8;

use crate::led::RgbRing;

pub struct Board {
  pub main_door_open: OutputPin,
  pub main_door_bell: InputPin,
  pub main_door_contact: InputPin,
  pub cellar_door_open: OutputPin,
  pub cellar_door_contact: InputPin,
  pub garage_door_1_open: OutputPin,
  pub garage_door_1_stop: OutputPin,
  pub garage_door_1_close: OutputPin,
  pub garage_door_1_contact: InputPin,
  pub garage_door_2_open: OutputPin,
  pub garage_door_2_stop: OutputPin,
  pub garage_door_2_close: OutputPin,
  pub garage_door_2_contact: InputPin,
  pub garage_door_button: InputPin,
  pub led: (OutputPin, OutputPin, OutputPin),
  pub ring: RgbRing,
}

impl Board {
  pub fn new(gpio: Gpio) -> Self {
    Self {
      main_door_open: gpio.get(19).unwrap().into_output_high(),
      main_door_bell: gpio.get(0).unwrap().into_input_pullup(),
      main_door_contact: gpio.get(17).unwrap().into_input_pullup(),
      cellar_door_open: gpio.get(26).unwrap().into_output_high(),
      cellar_door_contact: gpio.get(1).unwrap().into_input_pullup(),
      garage_door_1_open: gpio.get(13).unwrap().into_output_high(),
      garage_door_1_stop: gpio.get(6).unwrap().into_output_high(),
      garage_door_1_close: gpio.get(5).unwrap().into_output_high(),
      garage_door_1_contact: gpio.get(2).unwrap().into_input_pullup(),
      garage_door_2_open: gpio.get(21).unwrap().into_output_high(),
      garage_door_2_stop: gpio.get(20).unwrap().into_output_high(),
      garage_door_2_close: gpio.get(16).unwrap().into_output_high(),
      garage_door_2_contact: gpio.get(25).unwrap().into_input_pullup(),
      garage_door_button: gpio.get(24).unwrap().into_input_pullup(),
      led: (
        gpio.get(23).unwrap().into_output_low(),
        gpio.get(3).unwrap().into_output_low(),
        gpio.get(4).unwrap().into_output_low(),
      ),
      ring: {
        let mut ring = RgbRing::new();
        ring.set_bottom_left(RGB8 { r: 0x01, g: 0x01, b: 0x01 });
        ring
      },
    }
  }
}
