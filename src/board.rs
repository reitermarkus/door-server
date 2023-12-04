use rppal::gpio::{Bias, Gpio, InputPin, IoPin, Mode, OutputPin, Pin};
use smart_leds::RGB8;

use crate::led::RgbRing;

pub struct Board {
  pub main_door_open: IoPin,
  pub main_door_bell: InputPin,
  pub main_door_contact: InputPin,
  pub cellar_door_open: IoPin,
  pub cellar_door_contact: InputPin,
  pub garage_door_1_open: IoPin,
  pub garage_door_1_stop: IoPin,
  pub garage_door_1_close: IoPin,
  pub garage_door_1_contact: InputPin,
  pub garage_door_2_open: IoPin,
  pub garage_door_2_stop: IoPin,
  pub garage_door_2_close: IoPin,
  pub garage_door_2_contact: InputPin,
  pub garage_door_button: InputPin,
  pub led: (OutputPin, OutputPin, OutputPin),
  pub ring: RgbRing,
}

impl Board {
  pub fn new(gpio: Gpio) -> Self {
    let into_input_pullup = |pin: Pin| {
      let mut io_pin = pin.into_io(Mode::Input);
      io_pin.set_bias(Bias::PullUp);
      io_pin
    };

    Self {
      main_door_open: into_input_pullup(gpio.get(19).unwrap()),
      main_door_bell: gpio.get(0).unwrap().into_input_pullup(),
      main_door_contact: gpio.get(17).unwrap().into_input_pullup(),
      cellar_door_open: into_input_pullup(gpio.get(26).unwrap()),
      cellar_door_contact: gpio.get(1).unwrap().into_input_pullup(),
      garage_door_1_open: into_input_pullup(gpio.get(13).unwrap()),
      garage_door_1_stop: into_input_pullup(gpio.get(6).unwrap()),
      garage_door_1_close: into_input_pullup(gpio.get(5).unwrap()),
      garage_door_1_contact: gpio.get(2).unwrap().into_input_pullup(),
      garage_door_2_open: into_input_pullup(gpio.get(21).unwrap()),
      garage_door_2_stop: into_input_pullup(gpio.get(20).unwrap()),
      garage_door_2_close: into_input_pullup(gpio.get(16).unwrap()),
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
