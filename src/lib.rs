use std::time::Duration;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}, Mutex};
use std::thread::{self, sleep};

use rppal::gpio::Level;

mod door;
pub use door::Door;

mod garage_door;
pub use garage_door::GarageDoor;

mod waveshare_relay;
pub use waveshare_relay::WaveshareRelay;

pub trait StatefulDoor {
  fn on_change(&mut self, callback: impl FnMut(bool) + Send + 'static);

  fn is_closed(&self) -> bool;

  fn is_open(&self) -> bool;
}

impl<T> StatefulDoor for &mut T where T: StatefulDoor {
  fn on_change(&mut self, callback: impl FnMut(bool) + Send + 'static) {
    (**self).on_change(callback)
  }

  fn is_closed(&self) -> bool {
    (**self).is_closed()
  }

  fn is_open(&self) -> bool {
    (**self).is_open()
  }
}

pub fn on_change_debounce(callback: impl FnMut(bool) + Send + 'static) -> impl FnMut(Level) + Send + 'static {
  let last_value = Arc::new(AtomicUsize::new(0));
  let callback = Arc::new(Mutex::new(callback));

  move |level: Level| {
    let last_value = last_value.clone();
    let callback = callback.clone();

    let expected_value = last_value.fetch_add(1, Ordering::SeqCst).wrapping_add(1);

    thread::spawn(move || {
      sleep(Duration::from_millis(10));

      let current_value = last_value.load(Ordering::SeqCst);
      if current_value == expected_value {
        let closed = level == Level::Low;
        (callback.lock().unwrap())(closed);
      }
    });
  }
}
