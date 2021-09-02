use std::any::Any;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock, Weak};

use rppal::gpio::Gpio;
use serde_json::json;

use webthing::{
  Action, BaseProperty, BaseThing, Thing, ThingsType, WebThingServer,
  server::ActionGenerator,
};

use door_server::{Door, GarageDoor, WaveshareRelay, StatefulDoor};

mod action;
use action::{LockAction, UnlockAction};

struct Generator {
  doors: HashMap<String, Arc<RwLock<Box<dyn Any + Send + Sync>>>>,
}

impl ActionGenerator for Generator {
  fn generate(
      &self,
      thing: Weak<RwLock<Box<dyn Thing>>>,
      name: String,
      _input: Option<&serde_json::Value>,
  ) -> Option<Box<dyn Action>> {
    let id = {
      let thing = thing.upgrade()?;
      let thing = thing.read().unwrap();
      thing.get_id()
    };
    let door = self.doors.get(&id)?.clone();

    match name.as_str() {
      "lock" => Some(Box::new(LockAction::new(thing, door))),
      "unlock" => Some(Box::new(UnlockAction::new(thing, door))),
      _ => None,
    }
  }
}

fn door_state(locked: Option<bool>) -> serde_json::Value {
  json!(match locked {
    Some(true) => "locked",
    Some(false) => "unlocked",
    None => "unknown",
  })
}

fn set_property<T: DerefMut<Target = Box<dyn Thing + 'static>>>(mut thing: T, property: &str, value: serde_json::Value) {
  let locked_property = thing.find_property(&property.into()).unwrap();
  locked_property.set_cached_value(value.clone()).unwrap();
  thing.property_notify(property.into(), value);
}

fn make_door_thing(mut door: impl StatefulDoor, id: &str, name: &str, supports_locking: bool) -> Arc<RwLock<Box<dyn Thing + 'static>>> {
  let mut door_thing = BaseThing::new(
    format!("urn:dev:ops:32473-{}", id),
    name.to_owned(),
    Some(vec!["Lock".to_owned()]),
    Some("Door Opener and Contact Sensor".to_owned()),
  );

  let door_locked = json!({
    "@type": "LockedProperty",
    "title": "Door Lock Status",
    "type": "boolean",
    "description": "Whether or not the door is currently open.",
    "readOnly": true,
  });

  door_thing.add_property(Box::new(BaseProperty::new(
    "lock".into(),
    door_state(Some(door.is_closed())),
    None,
    Some(door_locked.as_object().unwrap().to_owned()),
  )));

  let door_unlock = json!({
    "title": "Unlock",
    "description": "Unlock the door.",
  });

  door_thing.add_available_action(
    "unlock".into(),
    door_unlock.as_object().unwrap().to_owned(),
  );

  if supports_locking {
    let door_lock = json!({
      "title": "Lock",
      "description": "Lock the door.",
    });

    door_thing.add_available_action(
      "lock".into(),
      door_lock.as_object().unwrap().to_owned(),
    );
  }

  let thing: Arc<RwLock<Box<dyn Thing + 'static>>>  = Arc::new(RwLock::new(Box::new(door_thing)));

  let thing_clone = thing.clone();
  door.on_change(move |closed| {
    let thing = thing_clone.write().unwrap();
    let value = door_state(Some(closed));
    set_property(thing, "lock", value)
  });

  thing
}

#[actix_rt::main]
async fn main() {
  env_logger::init();

  let mut gpio = Gpio::new().unwrap();

  let relay = WaveshareRelay::new(&mut gpio);

  let mut things = Vec::new();
  let mut doors = HashMap::new();

  let main_door_input = gpio.get(17).unwrap().into_input_pullup();
  let mut main_door = Door::new(relay.ch1, main_door_input);
  let main_door_thing = make_door_thing(&mut main_door, "main-door-1", "Main Door", false);
  let main_door: Arc<RwLock<Box<dyn Any + Send + Sync>>> = Arc::new(RwLock::new(Box::new(main_door)));

  doors.insert(main_door_thing.read().unwrap().get_id(), main_door.clone());
  things.push(main_door_thing.clone());

  let cellar_door_input = gpio.get(27).unwrap().into_input_pullup();
  let mut cellar_door = Door::new(relay.ch2, cellar_door_input);
  let cellar_door_thing = make_door_thing(&mut cellar_door, "cellar-door-1", "Cellar Door", false);
  let cellar_door: Arc<RwLock<Box<dyn Any + Send + Sync>>> = Arc::new(RwLock::new(Box::new(cellar_door)));

  doors.insert(cellar_door_thing.read().unwrap().get_id(), cellar_door.clone());
  things.push(cellar_door_thing.clone());

  let garage_door_input = gpio.get(22).unwrap().into_input_pullup();
  let stop_output = relay.ch4;
  let open_output = relay.ch3;
  let close_output = relay.ch5;
  let mut garage_door = GarageDoor::new(
    stop_output,
    open_output,
    close_output,
    garage_door_input,
  );
  let garage_door_thing = make_door_thing(&mut garage_door, "garage-door-1", "Garage Door", true);
  let garage_door: Arc<RwLock<Box<dyn Any + Send + Sync>>> = Arc::new(RwLock::new(Box::new(garage_door)));

  doors.insert(garage_door_thing.read().unwrap().get_id(), garage_door.clone());
  things.push(garage_door_thing.clone());

  let generator = Generator {
    doors,
  };

  let mut server = WebThingServer::new(
    ThingsType::Multiple(things, "DoorServer".to_owned()),
    Some(8888),
    None,
    None,
    Box::new(generator),
    None,
    Some(true),
  );

  server.start(None).await.unwrap()
}
