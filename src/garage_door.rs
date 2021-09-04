use std::time::Duration;
use std::thread::sleep;

use rppal::gpio::{InputPin, OutputPin, Trigger};

use super::*;

pub struct GarageDoor {
  s0: OutputPin,   // S0 - Button STOP (normally closed)
  s2: OutputPin,   // S2 - Button OPEN (normally open)
  s4: OutputPin,   // S4 - Button CLOSE (normally open)
  input: InputPin, //      Door Contact
}

impl GarageDoor {
  pub fn new(mut s0: OutputPin, mut s2: OutputPin, mut s4: OutputPin, input: InputPin) -> Self {
    s0.set_high();
    s2.set_high();
    s4.set_high();

    Self { s0, s2, s4, input }
  }

  pub fn open(&mut self) {
    if self.is_open() {
      self.stop();
    }

    self.s0.set_high();
    self.s4.set_high();

    self.s2.set_low();
    sleep(Duration::from_millis(250));
    self.s2.set_high();
  }

  pub fn stop(&mut self) {
    self.s2.set_high();
    self.s4.set_high();

    self.s0.set_low();
    sleep(Duration::from_millis(250));
    self.s0.set_high();
    sleep(Duration::from_millis(500));
  }

  pub fn close(&mut self) {
    if self.is_open() {
      self.stop();
    }

    self.s0.set_high();
    self.s2.set_high();

    self.s4.set_low();
    sleep(Duration::from_millis(250));
    self.s4.set_high();
  }
}

impl StatefulDoor for GarageDoor {
  fn on_change(&mut self, callback: impl FnMut(bool) + Send + 'static) {
    self.input.set_async_interrupt(
      Trigger::Both,
      on_change_debounce(callback),
    ).unwrap()
  }

  fn is_closed(&self) -> bool {
    self.input.is_low()
  }

  fn is_open(&self) -> bool {
    !self.is_closed()
  }
}
