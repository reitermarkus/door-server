use std::time::Duration;

use esphome_native_api::proto::version_2025_12_1::CoverOperation;
use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};
use tokio::{
  sync::RwLock,
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
}

#[derive(Debug)]
pub struct GarageDoor<C> {
  trigger_open: IoPin,  // S2 - Button OPEN (normally open)
  trigger_stop: IoPin,  // S0 - Button STOP (normally closed)
  trigger_close: IoPin, // S4 - Button CLOSE (normally open)
  state: Arc<RwLock<State>>,
  callback: Arc<Mutex<C>>,
}

impl<C, F> GarageDoor<C>
where
  F: Future + Send,
  C: (FnMut(GarageDoorState) -> F) + Send + 'static,
{
  pub async fn new(
    mut trigger_open: IoPin,
    mut trigger_stop: IoPin,
    mut trigger_close: IoPin,
    contact: InputPin,
    callback: C,
  ) -> Self {
    trigger_open.set_high();
    trigger_open.set_bias(Bias::PullUp);
    trigger_stop.set_high();
    trigger_stop.set_bias(Bias::PullUp);
    trigger_close.set_high();
    trigger_close.set_bias(Bias::PullUp);

    let callback = Arc::new(Mutex::new(callback));
    let weak_callback = Arc::downgrade(&callback);

    let state = Arc::new(RwLock::new(State {
      contact,
      last_open_trigger: None,
      last_stop_trigger: None,
      last_close_trigger: None,
    }));
    let weak_state = Arc::downgrade(&state);

    state
      .write()
      .await
      .contact
      .set_async_interrupt(
        Trigger::Both,
        Some(Duration::from_millis(50)),
        on_change_async(move |_closed| {
          let weak_callback = weak_callback.clone();
          let weak_state = weak_state.clone();

          async move {
            if let Some((callback, state)) = weak_callback.upgrade().zip(weak_state.upgrade()) {
              let state = state.read().await.state();
              callback.lock().await(state).await;
            }
          }
        }),
      )
      .unwrap();

    Self { trigger_open, trigger_stop, trigger_close, state, callback }
  }

  fn set_moving(&mut self, max_duration: Duration, target_state: GarageDoorState) {
    let start_time = Instant::now();
    let max_duration = max_duration.mul_f32(1.1) + Duration::from_secs(1);

    let weak_callback = Arc::downgrade(&self.callback);
    let weak_state = Arc::downgrade(&self.state);

    tokio::spawn(async move {
      while start_time.elapsed() <= max_duration {
        sleep(Duration::from_millis(100)).await;

        if let Some((callback, state)) = weak_callback.upgrade().zip(weak_state.upgrade()) {
          let state = state.read().await.state();
          callback.lock().await(state).await;

          if state == target_state {
            return;
          }
        } else {
          return;
        }
      }

      log::error!("Garage door movement did not finish in time.");
    });
  }

  pub async fn open(&mut self) {
    if !self.is_closed().await {
      self.stop().await;
    }

    let state_clone = self.state.clone();
    let state = &mut *state_clone.write().await;

    self.trigger_open.set_mode(Mode::Output);
    self.trigger_open.set_low();
    let now = Instant::now();
    state.last_open_trigger = Some(now);
    self.callback.lock().await(state.state()).await;
    self.set_moving(State::OPEN_DURATION, GarageDoorState::Open);
    sleep_until(now + Duration::from_millis(250)).await;
    self.trigger_open.set_high();

    self.trigger_open.set_mode(Mode::Input);
    self.trigger_open.set_bias(Bias::PullUp);
  }

  pub async fn handle_external_open(&mut self, delay: Duration) {
    let state_clone = self.state.clone();
    let state = &mut *state_clone.write().await;

    state.last_open_trigger = Instant::now().checked_add(delay);
    self.set_moving(State::OPEN_DURATION, GarageDoorState::Open);
  }

  pub async fn stop(&mut self) {
    let state_clone = self.state.clone();
    let state = &mut *state_clone.write().await;

    self.trigger_stop.set_mode(Mode::Output);
    self.trigger_stop.set_low();
    let now = Instant::now();
    state.last_stop_trigger = Some(now);
    self.callback.lock().await(state.state()).await;
    sleep_until(now + Duration::from_millis(250)).await;
    self.trigger_stop.set_high();
    sleep(Duration::from_millis(500)).await;
    self.callback.lock().await(state.state()).await;

    self.trigger_stop.set_mode(Mode::Input);
    self.trigger_stop.set_bias(Bias::PullUp);
  }

  pub async fn handle_external_stop(&mut self, delay: Duration) {
    let state_clone = self.state.clone();
    let state = &mut *state_clone.write().await;

    state.last_stop_trigger = Instant::now().checked_add(delay);
    self.callback.lock().await(state.state()).await;
  }

  pub async fn close(&mut self) {
    if !self.is_closed().await {
      self.stop().await;
    }

    let state_clone = self.state.clone();
    let state = &mut *state_clone.write().await;

    self.trigger_close.set_mode(Mode::Output);
    self.trigger_close.set_low();
    let now = Instant::now();
    state.last_close_trigger = Some(now);
    self.callback.lock().await(state.state()).await;
    self.set_moving(State::CLOSE_DURATION, GarageDoorState::Closed);
    sleep_until(now + Duration::from_millis(250)).await;
    self.trigger_close.set_high();

    self.trigger_close.set_mode(Mode::Input);
    self.trigger_close.set_bias(Bias::PullUp);
  }

  pub async fn force_update(&mut self) {
    self.callback.lock().await(self.state.read().await.state()).await;
  }

  pub async fn state(&self) -> GarageDoorState {
    self.state.read().await.state()
  }

  pub async fn is_closed(&self) -> bool {
    self.state.read().await.is_closed()
  }
}
