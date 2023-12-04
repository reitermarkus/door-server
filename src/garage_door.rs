use std::time::Duration;

use actix_rt::time::sleep;
use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};

use super::*;

#[derive(Debug)]
pub struct GarageDoor {
  trigger_open: IoPin,  // S2 - Button OPEN (normally open)
  trigger_stop: IoPin,  // S0 - Button STOP (normally closed)
  trigger_close: IoPin, // S4 - Button CLOSE (normally open)
  contact: InputPin,    //      Door Contact
}

impl GarageDoor {
  pub fn new(mut trigger_open: IoPin, mut trigger_stop: IoPin, mut trigger_close: IoPin, contact: InputPin) -> Self {
    trigger_open.set_high();
    trigger_open.set_bias(Bias::PullUp);
    trigger_stop.set_high();
    trigger_stop.set_bias(Bias::PullUp);
    trigger_close.set_high();
    trigger_close.set_bias(Bias::PullUp);

    Self { trigger_stop, trigger_open, trigger_close, contact }
  }

  pub async fn open(&mut self) {
    if self.is_open() {
      self.stop().await;
    }

    self.trigger_open.set_mode(Mode::Output);
    self.trigger_open.set_low();
    sleep(Duration::from_millis(250)).await;
    self.trigger_open.set_high();

    self.trigger_open.set_mode(Mode::Input);
    self.trigger_open.set_bias(Bias::PullUp);
  }

  pub async fn stop(&mut self) {
    self.trigger_stop.set_mode(Mode::Output);
    self.trigger_stop.set_low();
    sleep(Duration::from_millis(250)).await;
    self.trigger_stop.set_high();
    sleep(Duration::from_millis(500)).await;

    self.trigger_stop.set_mode(Mode::Input);
    self.trigger_stop.set_bias(Bias::PullUp);
  }

  pub async fn close(&mut self) {
    if self.is_open() {
      self.stop().await;
    }

    self.trigger_close.set_mode(Mode::Output);
    self.trigger_close.set_low();
    sleep(Duration::from_millis(250)).await;
    self.trigger_close.set_high();

    self.trigger_close.set_mode(Mode::Input);
    self.trigger_close.set_bias(Bias::PullUp);
  }
}

impl StatefulDoor for GarageDoor {
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
