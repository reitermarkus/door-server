use std::{sync::Arc, time::Duration};

use rppal::gpio::{InputPin, Trigger};
use tokio::sync::Mutex;

use crate::{door::ContactState, on_change_async};

#[derive(Debug)]
pub enum ButtonState {
  Pressed,
  Released,
}

#[derive(Debug)]
pub struct Button {
  pin: InputPin,
}

impl Button {
  pub fn new(pin: InputPin) -> Self {
    Self { pin }
  }
}

impl Button {
  pub fn on_change<C, F>(&mut self, callback: C)
  where
    F: Future + Send,
    C: (FnMut(ButtonState) -> F) + Send + 'static,
  {
    let callback = Arc::new(Mutex::new(callback));

    self
      .pin
      .set_async_interrupt(
        Trigger::Both,
        Some(Duration::from_millis(50)),
        on_change_async(move |contact_state| {
          let callback = callback.clone();

          async move {
            let callback = &mut *callback.lock().await;
            callback(match contact_state {
              ContactState::Open => ButtonState::Released,
              ContactState::Closed => ButtonState::Pressed,
            })
            .await;
          }
        }),
      )
      .unwrap()
  }
}
