use std::{future::Future, sync::Arc, thread};

use rppal::gpio::{Event, Trigger};
use tokio::{runtime::Runtime, sync::Mutex};

mod board;
pub use board::Board;

mod door;
pub use door::Door;

mod garage_door;
pub use garage_door::GarageDoor;

pub mod led;

pub trait StatefulDoor {
  fn on_change<C, F>(&mut self, callback: C)
  where
    F: Future,
    C: (FnMut(bool) -> F) + Send + 'static;

  fn is_closed(&self) -> bool;

  fn is_open(&self) -> bool;
}

impl<T: StatefulDoor> StatefulDoor for &mut T {
  fn on_change<C, F>(&mut self, callback: C)
  where
    F: Future,
    C: (FnMut(bool) -> F) + Send + 'static,
  {
    (**self).on_change(callback)
  }

  fn is_closed(&self) -> bool {
    (**self).is_closed()
  }

  fn is_open(&self) -> bool {
    (**self).is_open()
  }
}

pub fn on_change_async<C, F>(callback: C) -> impl FnMut(Event) + Send + 'static
where
  F: Future,
  C: (FnMut(bool) -> F) + Send + 'static,
{
  let callback = Arc::new(Mutex::new(callback));

  move |event: Event| {
    let callback = callback.clone();

    thread::spawn(move || {
      let rt: Runtime = Runtime::new().unwrap();
      rt.block_on(async move {
        let closed = event.trigger == Trigger::FallingEdge;
        callback.lock().await(closed).await;
      })
    });
  }
}
