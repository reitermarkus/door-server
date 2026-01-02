use std::time::Duration;

use actix_rt::time::sleep;
use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};
use tokio::time::Instant;

use super::*;

#[derive(Debug)]
pub struct Door {
  trigger_open: IoPin,
  last_open_trigger: Option<Instant>,
  contact: InputPin,
}

impl Door {
  /// Duration for which the door remains open after being triggered.
  const OPEN_DURATION: Duration = Duration::from_secs(5);

  pub fn new(mut trigger_open: IoPin, contact: InputPin) -> Self {
    trigger_open.set_high();
    let last_open_trigger = if trigger_open.is_low() { Some(Instant::now()) } else { None };

    Self { trigger_open, last_open_trigger, contact }
  }

  pub async fn open(&mut self) {
    self.trigger_open.set_mode(Mode::Output);

    self.trigger_open.set_low();
    self.last_open_trigger = Some(Instant::now());
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
    self.contact.set_async_interrupt(Trigger::Both, Some(Duration::from_millis(50)), on_change_async(callback)).unwrap()
  }

  fn is_closed(&self) -> bool {
    if let Some(last_open_trigger) = self.last_open_trigger
      && last_open_trigger.elapsed() < Self::OPEN_DURATION
    {
      return false;
    }

    self.contact.is_low()
  }

  fn is_open(&self) -> bool {
    !self.is_closed()
  }
}
