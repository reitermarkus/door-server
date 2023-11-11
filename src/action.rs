use std::{
  any::Any,
  sync::{Arc, RwLock, Weak},
  thread,
};

use uuid::Uuid;
use webthing::{Action, BaseAction, Thing};

use door_server::{Door, GarageDoor};

macro_rules! action {
  ($ty:ident, $action_name:expr, $method:expr) => {
    pub struct $ty {
      action: BaseAction,
      door: Arc<RwLock<Box<dyn Any + Send + Sync>>>,
    }

    impl $ty {
      pub fn new(thing: Weak<RwLock<Box<dyn Thing>>>, door: Arc<RwLock<Box<dyn Any + Send + Sync>>>) -> Self {
        Self { action: BaseAction::new(Uuid::new_v4().to_string(), $action_name.to_owned(), None, thing), door }
      }
    }

    impl Action for $ty {
      fn set_href_prefix(&mut self, prefix: String) {
        self.action.set_href_prefix(prefix)
      }

      fn get_id(&self) -> String {
        self.action.get_id()
      }

      fn get_name(&self) -> String {
        self.action.get_name()
      }

      fn get_href(&self) -> String {
        self.action.get_href()
      }

      fn get_status(&self) -> String {
        self.action.get_status()
      }

      fn get_time_requested(&self) -> String {
        self.action.get_time_requested()
      }

      fn get_time_completed(&self) -> Option<String> {
        self.action.get_time_completed()
      }

      fn get_input(&self) -> Option<serde_json::Map<String, serde_json::Value>> {
        self.action.get_input()
      }

      fn get_thing(&self) -> Option<Arc<RwLock<Box<dyn Thing>>>> {
        self.action.get_thing()
      }

      fn set_status(&mut self, status: String) {
        self.action.set_status(status)
      }

      fn start(&mut self) {
        self.action.start()
      }

      fn perform_action(&mut self) {
        let thing = if let Some(thing) = self.get_thing() { thing.clone() } else { return };
        let action_name = self.get_name();
        let id = self.get_id();
        let door = self.door.clone();

        thread::spawn(move || {
          let mut door = door.write().unwrap();

          $method(&mut *door);

          let mut thing = thing.write().unwrap();
          thing.finish_action(action_name, id);
        });
      }

      fn cancel(&mut self) {
        self.action.cancel()
      }

      fn finish(&mut self) {
        self.action.finish()
      }
    }
  };
}

action!(UnlockAction, "unlock", |door: &mut Box<dyn Any + Send + Sync>| {
  if let Some(ref mut door) = door.downcast_mut::<Door>() {
    door.open();
  } else if let Some(ref mut door) = door.downcast_mut::<GarageDoor>() {
    door.open();
  } else {
    unreachable!()
  }
});

action!(LockAction, "lock", |door: &mut Box<dyn Any + Send + Sync>| {
  if let Some(ref mut door) = door.downcast_mut::<GarageDoor>() {
    door.close();
  } else {
    unreachable!()
  }
});
