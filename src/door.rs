use std::time::Duration;

use rppal::gpio::{Bias, InputPin, IoPin, Mode, Trigger};
use tokio::{
  sync::RwLock,
  time::{Instant, sleep_until},
};

use super::*;

#[derive(Debug)]
struct DoorState {
  last_open_trigger: Option<Instant>,
  contact: InputPin,
}

impl DoorState {
  /// Duration for which the door remains open after being triggered.
  const OPEN_DURATION: Duration = Duration::from_secs(6);

  pub fn is_locked(&self) -> bool {
    if let Some(last_open_trigger) = self.last_open_trigger {
      return last_open_trigger.elapsed() > Self::OPEN_DURATION;
    }

    true
  }

  pub fn is_closed(&self) -> bool {
    self.is_locked() && self.contact.is_low()
  }
}

#[derive(Debug)]
pub struct Door<C> {
  trigger_open: IoPin,
  state: Arc<RwLock<DoorState>>,
  callback: Arc<Mutex<C>>,
}

impl<C, F> Door<C>
where
  F: Future + Send,
  C: (FnMut(bool) -> F) + Send + 'static,
{
  pub async fn new(mut trigger_open: IoPin, contact: InputPin, callback: C) -> Self {
    trigger_open.set_high();
    let last_open_trigger = if trigger_open.is_low() { Some(Instant::now()) } else { None };

    let callback = Arc::new(Mutex::new(callback));
    let callback_clone = callback.clone();

    let state = Arc::new(RwLock::new(DoorState { last_open_trigger, contact }));
    let state_clone = Arc::downgrade(&state); // Avoid strongly self-referencing callback.

    state
      .write()
      .await
      .contact
      .set_async_interrupt(
        Trigger::Both,
        Some(Duration::from_millis(50)),
        on_change_async(move |closed| {
          let state = state_clone.clone();
          let callback = callback_clone.clone();

          async move {
            if let Some(state) = state.upgrade() {
              let callback = &mut *callback.lock().await;
              let state = &*state.read().await;
              callback(state.is_locked() && closed).await;
            }
          }
        }),
      )
      .unwrap();

    Self { trigger_open, state, callback }
  }

  pub async fn open(&mut self) {
    log::info!("Door::open");
    self.trigger_open.set_mode(Mode::Output);

    self.trigger_open.set_low();
    let now = Instant::now();
    self.set_opening(now).await;
    sleep_until(now + Duration::from_millis(250)).await;
    self.trigger_open.set_high();

    log::info!("Door::open done");

    self.trigger_open.set_mode(Mode::Input);
    self.trigger_open.set_bias(Bias::PullUp);
  }

  pub async fn force_update(&mut self) {
    self.callback.lock().await(self.is_closed().await).await;
  }

  pub async fn set_opening(&mut self, time: Instant) {
    let state_clone = self.state.clone();
    let state = &mut *self.state.write().await;

    let last_open_trigger = time;
    state.last_open_trigger = Some(time);

    self.callback.lock().await(state.is_closed()).await;

    let callback_clone = self.callback.clone();

    // Refresh status after motor lock is closed again.
    tokio::spawn(async move {
      tokio::time::sleep_until(last_open_trigger + DoorState::OPEN_DURATION).await;

      let state = &*state_clone.read().await;
      let callback = &mut *callback_clone.lock().await;

      let closed = state.is_closed();
      log::info!("Door::set_opening callback: closed={closed}");
      callback(closed).await;
    });
  }

  pub async fn handle_external_open(&mut self) {
    self.set_opening(Instant::now()).await;
  }

  pub async fn is_closed(&self) -> bool {
    let state = self.state.read().await;
    state.is_closed()
  }
}
