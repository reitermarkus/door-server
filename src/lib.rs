use std::{
  future::Future,
  sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
  },
  thread,
  time::Duration,
};

use actix_rt::time::sleep;
use rppal::gpio::Level;
use tokio::{runtime::Runtime, sync::Mutex};

mod board;
pub use board::Board;

mod door;
pub use door::Door;

mod garage_door;
pub use garage_door::GarageDoor;

pub mod led;

mod waveshare_relay;
pub use waveshare_relay::WaveshareRelay;

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

pub fn on_change_debounce<C, F>(callback: C) -> impl FnMut(Level) + Send + 'static
where
  F: Future,
  C: (FnMut(bool) -> F) + Send + 'static,
{
  let last_value = Arc::new(AtomicUsize::new(0));
  let callback = Arc::new(Mutex::new(callback));

  move |level: Level| {
    let last_value = last_value.clone();
    let callback = callback.clone();

    let expected_value = last_value.fetch_add(1, Ordering::SeqCst).wrapping_add(1);

    thread::spawn(move || {
      let rt: Runtime = Runtime::new().unwrap();
      rt.block_on(async move {
        sleep(Duration::from_millis(50)).await;

        let current_value = last_value.load(Ordering::SeqCst);
        if current_value == expected_value {
          let closed = level == Level::Low;
          callback.lock().await(closed).await;
        }
      })
    });
  }
}
