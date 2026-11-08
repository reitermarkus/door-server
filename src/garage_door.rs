use std::time::Duration;

use esphome_native_api::proto::CoverOperation;
use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};
use tokio::{
  sync::RwLock,
  task::AbortHandle,
  time::{Instant, sleep, sleep_until},
};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GarageDoorState {
  Open,
  Opening(f32),
  Stopped(f32),
  Closing(f32),
  Closed,
}

impl GarageDoorState {
  pub fn is_stopped(&self) -> bool {
    matches!(self, Self::Open | Self::Stopped(_) | Self::Closed)
  }

  pub fn is_closed(&self) -> bool {
    matches!(self, Self::Closed)
  }

  pub fn position(&self) -> f32 {
    match self {
      Self::Open => 1.0,
      Self::Opening(pos) => *pos,
      Self::Stopped(pos) => *pos,
      Self::Closing(pos) => *pos,
      Self::Closed => 0.0,
    }
  }

  pub fn cover_operation(&self) -> CoverOperation {
    match self {
      Self::Open => CoverOperation::Idle,
      Self::Opening(_) => CoverOperation::IsOpening,
      Self::Stopped(_) => CoverOperation::Idle,
      Self::Closing(_) => CoverOperation::IsClosing,
      Self::Closed => CoverOperation::Idle,
    }
  }
}

#[derive(Debug)]
struct State {
  contact: InputPin, // Door Contact
  last_open_trigger: Option<Instant>,
  last_stop_trigger: Option<Instant>,
  last_close_trigger: Option<Instant>,
}

impl State {
  const OPEN_DURATION: Duration = Duration::from_millis(14_700);
  const CLOSE_DURATION: Duration = Duration::from_millis(18_700);

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

  pub fn is_closed(&self) -> bool {
    self.contact.is_low()
  }

  pub fn is_open(&self) -> bool {
    self.state() == GarageDoorState::Open
  }
}

#[derive(Debug)]
struct IoTrigger {
  pin: IoPin,
}

impl IoTrigger {
  const TRIGGER_DURATION: Duration = Duration::from_millis(250);

  pub fn new(pin: IoPin) -> Self {
    let mut trigger = Self { pin };
    trigger.to_input();
    trigger
  }

  fn to_input(&mut self) {
    self.pin.set_mode(Mode::Input);
    self.pin.set_bias(Bias::PullUp);
  }

  pub async fn trigger(&mut self) -> Instant {
    self.pin.set_mode(Mode::Output);
    self.pin.set_low();
    let now = Instant::now();
    sleep_until(now + Self::TRIGGER_DURATION).await;
    self.pin.set_high();

    self.to_input();

    now
  }
}

#[derive(Debug)]
pub struct GarageDoor<C> {
  trigger_open: IoTrigger,  // S2 - Button OPEN (normally open)
  trigger_stop: IoTrigger,  // S0 - Button STOP (normally closed)
  trigger_close: IoTrigger, // S4 - Button CLOSE (normally open)
  callback_and_state: Arc<RwLock<(C, State)>>,
  moving_abort_handle: Option<AbortHandle>,
}

impl<C, F> GarageDoor<C>
where
  F: Future + Send,
  C: (FnMut(GarageDoorState) -> F) + Send + Sync + 'static,
{
  pub async fn new(
    trigger_open: IoPin,
    trigger_stop: IoPin,
    trigger_close: IoPin,
    contact: InputPin,
    callback: C,
  ) -> Self {
    let trigger_open = IoTrigger::new(trigger_open);
    let trigger_stop = IoTrigger::new(trigger_stop);
    let trigger_close = IoTrigger::new(trigger_close);

    let callback_and_state = Arc::new(RwLock::new((
      callback,
      State { contact, last_open_trigger: None, last_stop_trigger: None, last_close_trigger: None },
    )));
    let weak_callback_and_state = Arc::downgrade(&callback_and_state);

    callback_and_state
      .write()
      .await
      .1
      .contact
      .set_async_interrupt(
        Trigger::Both,
        Some(Duration::from_millis(50)),
        on_change_async(move |_closed| {
          let weak_callback_and_state = weak_callback_and_state.clone();

          async move {
            if let Some(callback_and_state) = weak_callback_and_state.upgrade() {
              let (callback, state) = &mut *callback_and_state.write().await;
              callback(state.state()).await;
            }
          }
        }),
      )
      .unwrap();

    Self { trigger_open, trigger_stop, trigger_close, callback_and_state, moving_abort_handle: None }
  }

  fn set_moving(&mut self, max_duration: Duration, target_state: GarageDoorState) {
    log::trace!("GarageDoor::set_moving");

    let start_time = Instant::now();
    let max_duration = max_duration.mul_f32(1.1) + Duration::from_secs(1);

    let weak_callback_and_state = Arc::downgrade(&self.callback_and_state);

    if let Some(moving_abort_handle) = self.moving_abort_handle.take() {
      moving_abort_handle.abort();
    }

    let join_handle = tokio::spawn(async move {
      while start_time.elapsed() <= max_duration {
        sleep(Duration::from_millis(100)).await;

        if let Some(callback_and_state) = weak_callback_and_state.upgrade() {
          let (callback, state) = &mut *callback_and_state.write().await;

          let state = state.state();
          callback(state).await;

          if state == target_state {
            log::debug!("Reached target state.");
            return;
          }
        } else {
          return;
        }
      }

      log::error!("Garage door movement did not finish in time.");
    });
    self.moving_abort_handle = Some(join_handle.abort_handle());
  }

  pub async fn open(&mut self) {
    log::trace!("GarageDoor::open");

    let callback_and_state_clone = self.callback_and_state.clone();
    let (callback, state) = &mut *callback_and_state_clone.write().await;

    if !state.is_closed() {
      self.stop_with_callback_and_state(callback, state).await;
    }

    state.last_open_trigger = Some(self.trigger_open.trigger().await);
    callback(state.state());
    self.set_moving(State::OPEN_DURATION, GarageDoorState::Open);
  }

  pub async fn handle_external_open(&mut self, delay: Duration) {
    let callback_and_state_clone = self.callback_and_state.clone();
    let (_callback, state) = &mut *callback_and_state_clone.write().await;

    state.last_open_trigger = Instant::now().checked_add(delay);
    self.set_moving(State::OPEN_DURATION, GarageDoorState::Open);
  }

  pub async fn stop(&mut self) {
    log::trace!("GarageDoor::stop");

    let callback_and_state_clone = self.callback_and_state.clone();
    let (callback, state) = &mut *callback_and_state_clone.write().await;

    self.stop_with_callback_and_state(callback, state).await;
  }

  async fn stop_with_callback_and_state(&mut self, callback: &mut C, state: &mut State) {
    log::trace!("GarageDoor::stop_with_callback_and_state");

    if let Some(moving_abort_handle) = self.moving_abort_handle.take() {
      moving_abort_handle.abort();
    }

    state.last_stop_trigger = Some(self.trigger_stop.trigger().await);
    callback(state.state()).await;
    sleep(Duration::from_millis(500)).await;
    callback(state.state()).await;
  }

  pub async fn handle_external_stop(&mut self, delay: Duration) {
    let callback_and_state_clone = self.callback_and_state.clone();
    let (callback, state) = &mut *callback_and_state_clone.write().await;

    state.last_stop_trigger = Instant::now().checked_add(delay);
    callback(state.state()).await;
  }

  pub async fn close(&mut self) {
    log::trace!("GarageDoor::close");

    let callback_and_state_clone = self.callback_and_state.clone();
    let (callback, state) = &mut *callback_and_state_clone.write().await;

    if !state.is_closed() {
      self.stop_with_callback_and_state(callback, state).await;
    }

    state.last_close_trigger = Some(self.trigger_close.trigger().await);
    callback(state.state()).await;
    self.set_moving(State::CLOSE_DURATION, GarageDoorState::Closed);
  }

  pub async fn force_update(&self) {
    log::trace!("GarageDoor::force_update");

    let callback_and_state_clone = self.callback_and_state.clone();
    let (callback, state) = &mut *callback_and_state_clone.write().await;

    callback(state.state()).await;
  }

  pub async fn is_closed(&self) -> bool {
    let callback_and_state_clone = self.callback_and_state.clone();
    let (_callback, state) = &mut *callback_and_state_clone.write().await;

    state.is_closed()
  }

  pub async fn is_open(&self) -> bool {
    let callback_and_state_clone = self.callback_and_state.clone();
    let (_callback, state) = &mut *callback_and_state_clone.write().await;

    state.is_open()
  }

  pub async fn state(&self) -> GarageDoorState {
    let callback_and_state_clone = self.callback_and_state.clone();
    let (_callback, state) = &mut *callback_and_state_clone.write().await;

    state.state()
  }
}
