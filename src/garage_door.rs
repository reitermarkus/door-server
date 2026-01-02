use std::time::Duration;

use actix_rt::time::sleep;
use esphome_native_api::proto::version_2025_12_1::CoverOperation;
use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};
use tokio::time::Instant;

use super::*;

#[derive(Debug, Clone, Copy)]
pub enum GarageDoorState {
  Open,
  Opening(f32),
  Stopped(f32),
  Closing(f32),
  Closed,
}

impl GarageDoorState {
  pub fn is_stopped(&self) -> bool {
    matches!(self, GarageDoorState::Open | GarageDoorState::Stopped(_) | GarageDoorState::Closed)
  }

  pub fn position(&self) -> f32 {
    match self {
      GarageDoorState::Open => 1.0,
      GarageDoorState::Opening(pos) => *pos,
      GarageDoorState::Stopped(pos) => *pos,
      GarageDoorState::Closing(pos) => *pos,
      GarageDoorState::Closed => 0.0,
    }
  }

  pub fn cover_operation(&self) -> CoverOperation {
    match self {
      GarageDoorState::Open => CoverOperation::Idle,
      GarageDoorState::Opening(_) => CoverOperation::IsOpening,
      GarageDoorState::Stopped(_) => CoverOperation::Idle,
      GarageDoorState::Closing(_) => CoverOperation::IsClosing,
      GarageDoorState::Closed => CoverOperation::Idle,
    }
  }
}

#[derive(Debug)]
pub struct GarageDoor {
  trigger_open: IoPin, // S2 - Button OPEN (normally open)
  last_open_trigger: Option<Instant>,
  trigger_stop: IoPin, // S0 - Button STOP (normally closed)
  last_stop_trigger: Option<Instant>,
  trigger_close: IoPin, // S4 - Button CLOSE (normally open)
  last_close_trigger: Option<Instant>,
  contact: InputPin, //      Door Contact
}

impl GarageDoor {
  const OPEN_DURATION: Duration = Duration::from_millis(14_400);
  const CLOSE_DURATION: Duration = Duration::from_millis(18_400);

  pub fn new(mut trigger_open: IoPin, mut trigger_stop: IoPin, mut trigger_close: IoPin, contact: InputPin) -> Self {
    trigger_open.set_high();
    trigger_open.set_bias(Bias::PullUp);
    trigger_stop.set_high();
    trigger_stop.set_bias(Bias::PullUp);
    trigger_close.set_high();
    trigger_close.set_bias(Bias::PullUp);

    Self {
      trigger_open,
      last_open_trigger: None,
      trigger_stop,
      last_stop_trigger: None,
      trigger_close,
      last_close_trigger: None,
      contact,
    }
  }

  pub async fn open(&mut self) {
    if self.is_open() {
      self.stop().await;
    }

    self.trigger_open.set_mode(Mode::Output);
    self.trigger_open.set_low();
    self.last_open_trigger = Some(Instant::now());
    sleep(Duration::from_millis(250)).await;
    self.trigger_open.set_high();

    self.trigger_open.set_mode(Mode::Input);
    self.trigger_open.set_bias(Bias::PullUp);
  }

  pub async fn stop(&mut self) {
    self.trigger_stop.set_mode(Mode::Output);
    self.trigger_stop.set_low();
    self.last_stop_trigger = Some(Instant::now());
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
    self.last_close_trigger = Some(Instant::now());
    sleep(Duration::from_millis(250)).await;
    self.trigger_close.set_high();

    self.trigger_close.set_mode(Mode::Input);
    self.trigger_close.set_bias(Bias::PullUp);
  }

  pub fn state(&self) -> GarageDoorState {
    if self.contact.is_low() {
      return GarageDoorState::Closed;
    }

    let opening = |open_time: Instant| {
      let open_duration = open_time.elapsed();

      if open_duration > Self::OPEN_DURATION {
        return GarageDoorState::Open;
      }

      GarageDoorState::Opening(open_duration.div_duration_f32(Self::OPEN_DURATION).min(1.0))
    };

    let closing = |close_time: Instant| {
      GarageDoorState::Closing(1.0 - close_time.elapsed().div_duration_f32(Self::CLOSE_DURATION).min(1.0))
    };

    let opening_stopped = |open_time: Instant, stop_time: Instant| {
      GarageDoorState::Stopped(
        stop_time.saturating_duration_since(open_time).div_duration_f32(Self::OPEN_DURATION).min(1.0),
      )
    };

    let closing_stopped = |close_time: Instant, stop_time: Instant| {
      GarageDoorState::Stopped(
        1.0 - stop_time.saturating_duration_since(close_time).div_duration_f32(Self::CLOSE_DURATION).min(1.0),
      )
    };

    match (self.last_open_trigger, self.last_stop_trigger, self.last_close_trigger) {
      (None, _, None) => GarageDoorState::Open,
      (Some(open_time), Some(stop_time), None) if stop_time > open_time => opening_stopped(open_time, stop_time),
      (Some(open_time), _, None) => opening(open_time),
      (None, Some(stop_time), Some(close_time)) if stop_time > close_time => closing_stopped(close_time, stop_time),
      (None, _, Some(close_time)) => closing(close_time),
      (Some(open_time), stop_time, Some(close_time)) => {
        if let Some(stop_time) = stop_time {
          if stop_time > open_time && open_time > close_time {
            return opening_stopped(open_time, stop_time);
          }

          if stop_time > close_time && close_time > open_time {
            return closing_stopped(close_time, stop_time);
          }
        }

        if open_time > close_time {
          return opening(open_time);
        }

        if close_time > open_time {
          return closing(close_time);
        }

        GarageDoorState::Open
      },
    }
  }
}

impl StatefulDoor for GarageDoor {
  fn on_change<C, F>(&mut self, callback: C)
  where
    F: Future,
    C: (FnMut(bool) -> F) + Send + 'static,
  {
    self.contact.set_async_interrupt(Trigger::Both, Some(Duration::from_millis(50)), on_change_async(callback)).unwrap()
  }

  fn is_closed(&self) -> bool {
    self.contact.is_low()
  }

  fn is_open(&self) -> bool {
    !self.is_closed()
  }
}
