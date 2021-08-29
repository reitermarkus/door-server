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
