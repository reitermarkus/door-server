use std::{future::Future, sync::Arc};

use rppal::gpio::{Event, Trigger};
use tokio::sync::Mutex;

mod board;
pub use board::Board;

mod door;
pub use door::{ContactState, Door};

mod door_bell;
pub use door_bell::{Button, ButtonState};

mod garage_door;
pub use garage_door::{GarageDoor, GarageDoorState};

pub mod led;

pub fn on_change_async<C, F>(callback: C) -> impl FnMut(Event) + Send + 'static
where
  F: Future + Send,
  C: (FnMut(ContactState) -> F) + Send + 'static,
{
  let callback = Arc::new(Mutex::new(callback));

  let handle = tokio::runtime::Handle::current();

  move |event: Event| {
    let callback = callback.clone();

    handle.block_on(async move {
      let contact_state = match event.trigger {
        Trigger::FallingEdge => ContactState::Closed,
        Trigger::RisingEdge => ContactState::Open,
        _ => unreachable!(),
      };
      callback.lock().await(contact_state).await;
    });
  }
}
