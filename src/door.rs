use std::time::Duration;
use std::thread::sleep;

use rppal::gpio::{InputPin, OutputPin, Trigger, Level};

use crate::StatefulDoor;

#[derive(Debug)]
pub struct Door {
  output: OutputPin,
  input: InputPin,
}

impl Door {
  pub fn new(mut output: OutputPin, input: InputPin) -> Self {
    output.set_high();

    Self { output, input }
  }

  pub fn open(&mut self) {
    self.output.set_low();
    sleep(Duration::from_millis(250));
    self.output.set_high();
  }
}

impl StatefulDoor for Door {
  fn on_change(&mut self, mut callback: impl FnMut(bool) + Send + 'static) {
    self.input.set_async_interrupt(Trigger::Both, move |level| {
      let closed = match level {
        Level::Low => true,
        Level::High => false,
      };

      callback(closed);
    }).unwrap()
  }

  fn is_closed(&self) -> bool {
    self.input.is_low()
  }

  fn is_open(&self) -> bool {
    !self.is_closed()
  }
}
