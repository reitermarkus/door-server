use std::time::Duration;
use std::thread::sleep;
use std::sync::{Arc, RwLock};

use debounced_pin::{Debounce, DebounceState, DebouncedInputPin, ActiveLow};
use rppal::gpio::{InputPin, OutputPin, Trigger, Level};
use embedded_hal::digital::InputPin as _;

use crate::StatefulDoor;

// #[derive(Debug)]
// pub struct Debounce {
//   time: Duration,
//   last_update: Option<Instant>,
// }

pub struct GarageDoor {
  s0: OutputPin, // S0 - Taster HALT (normally closed)
  s2: OutputPin, // S2 - Taster AUF (normally open)
  s4: OutputPin, // S4 - Taster ZU (normally open)
  input: Arc<RwLock<DebouncedInputPin<InputPin, ActiveLow>>>,
}

impl GarageDoor {
  pub fn new(mut s0:  OutputPin, mut s2: OutputPin, mut s4: OutputPin, input: InputPin) -> Self {
    s0.set_high();
    s2.set_high();
    s4.set_high();

    Self { s0, s2, s4, input: Arc::new(RwLock::new(DebouncedInputPin::new(input, ActiveLow))) }
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
  fn on_change(&mut self, mut callback: impl FnMut(bool) + Send + 'static) {
    let debounced_pin = self.input.clone();

    let callback = move |_| {
      let mut pin = debounced_pin.write().unwrap();
      if pin.update().unwrap() == DebounceState::Debouncing {
        return
      }

      let closed = pin.try_is_low().unwrap_or(false);
      callback(closed);
    };

    self.input.write().unwrap().pin
      .set_async_interrupt(Trigger::Both, callback).unwrap()
  }

  fn is_closed(&self) -> bool {
    self.input.read().unwrap().try_is_low().unwrap()
  }

  fn is_open(&self) -> bool {
    !self.is_closed()
  }
}
