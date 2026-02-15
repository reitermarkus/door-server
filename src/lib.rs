use std::{future::Future, sync::Arc};

use rppal::gpio::{Event, Trigger};
use tokio::sync::Mutex;

mod board;
pub use board::Board;

mod door;
pub use door::Door;

mod door_bell;
pub use door_bell::Button;

mod garage_door;
pub use garage_door::{GarageDoor, GarageDoorState};

pub mod led;

pub fn on_change_async<C, F>(callback: C) -> impl FnMut(Event) + Send + 'static
where
  F: Future + Send,
  C: (FnMut(bool) -> F) + Send + 'static,
{
  let callback = Arc::new(Mutex::new(callback));

  let handle = tokio::runtime::Handle::current();

  move |event: Event| {
    let callback = callback.clone();

    handle.block_on(async move {
      let closed = event.trigger == Trigger::FallingEdge;
      callback.lock().await(closed).await;
    });
  }
}
