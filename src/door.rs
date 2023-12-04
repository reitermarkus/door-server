use std::time::Duration;

use actix_rt::time::sleep;
use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};

use super::*;

#[derive(Debug)]
pub struct Door {
  trigger_open: IoPin,
  contact: InputPin,
}

impl Door {
  pub fn new(mut trigger_open: IoPin, contact: InputPin) -> Self {
    trigger_open.set_high();

    Self { trigger_open, contact }
  }

  pub async fn open(&mut self) {
    self.trigger_open.set_mode(Mode::Output);

    self.trigger_open.set_low();
    sleep(Duration::from_millis(250)).await;
    self.trigger_open.set_high();

    self.trigger_open.set_mode(Mode::Input);
    self.trigger_open.set_bias(Bias::PullUp);
  }
}

impl StatefulDoor for Door {
  fn on_change<C, F>(&mut self, callback: C)
  where
    F: Future,
    C: (FnMut(bool) -> F) + Send + 'static,
  {
    self.contact.set_async_interrupt(Trigger::Both, on_change_debounce(callback)).unwrap()
  }

  fn is_closed(&self) -> bool {
    self.contact.is_low()
  }

  fn is_open(&self) -> bool {
    !self.is_closed()
  }
}
