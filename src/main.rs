use std::{
  any::Any,
  collections::HashMap,
  net::UdpSocket,
  ops::DerefMut,
  str,
  sync::{Arc, Mutex, RwLock, Weak},
  thread,
};

use rppal::gpio::{Gpio, Trigger};
use serde_json::json;
use webthing::{
  server::ActionGenerator, Action, BaseEvent, BaseProperty, BaseThing, Thing, ThingsType, WebThingServer,
};

use door_server::{on_change_debounce, Door, GarageDoor, StatefulDoor};

mod action;
use action::{LockAction, UnlockAction};

use door_server::{led::closed_to_color, Board};

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

fn set_property<T: DerefMut<Target = Box<dyn Thing + 'static>>>(
  mut thing: T,
  property: &str,
  value: serde_json::Value,
) {
  let locked_property = thing.find_property(property).unwrap();

  let previous_value = locked_property.get_value();
  if previous_value != value {
    locked_property.set_cached_value(value.clone()).unwrap();
    thing.property_notify(property.into(), value);
  }
}

fn make_door_thing(
  mut door: impl StatefulDoor,
  id: &str,
  name: &str,
  supports_locking: bool,
  mut on_change: impl FnMut(bool) + Send + Sync + 'static,
) -> Arc<RwLock<Box<dyn Thing + 'static>>> {
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

  door_thing.add_available_action("unlock".into(), door_unlock.as_object().unwrap().to_owned());

  door_thing.add_available_event(
    "finger_scan".to_owned(),
    json!({
      "description": "A finger has been scanned.",
      "type": "object",
      "unit": "",
    })
    .as_object()
    .unwrap()
    .to_owned(),
  );

  if supports_locking {
    let door_lock = json!({
      "title": "Lock",
      "description": "Lock the door.",
    });

    door_thing.add_available_action("lock".into(), door_lock.as_object().unwrap().to_owned());
  }

  let thing: Arc<RwLock<Box<dyn Thing + 'static>>> = Arc::new(RwLock::new(Box::new(door_thing)));

  // Initialize at start.
  on_change(door.is_closed());

  let thing_clone = thing.clone();
  door.on_change(move |closed| {
    let thing = thing_clone.write().unwrap();
    let value = door_state(Some(closed));
    set_property(thing, "lock", value);

    on_change(closed)
  });

  thing
}

#[actix_rt::main]
async fn main() {
  env_logger::init();

  let gpio = Gpio::new().unwrap();
  let board = Board::new(gpio);

  let mut things = Vec::new();
  let mut doors = HashMap::new();

  let led = Arc::new(Mutex::new(board.led));
  let ring = Arc::new(Mutex::new(board.ring));

  let mut door_bell_button = board.main_door_bell;
  let mut garage_door_button = board.garage_door_button;

  let mut main_door = Door::new(board.main_door_open, board.main_door_contact);
  let ring_clone = ring.clone();
  let main_door_thing = make_door_thing(&mut main_door, "main-door-1", "Main Door", false, move |closed| {
    let mut ring = ring_clone.lock().unwrap();
    ring.set_top_left(closed_to_color(closed));
    ring.render();
  });
  main_door_thing.write().unwrap().add_available_event(
    "bell".to_owned(),
    json!({
      "description": "The door bell has been rung.",
      "type": "object",
      "unit": "",
    })
    .as_object()
    .unwrap()
    .to_owned(),
  );

  let main_door: Arc<RwLock<Box<dyn Any + Send + Sync>>> = Arc::new(RwLock::new(Box::new(main_door)));

  doors.insert(main_door_thing.read().unwrap().get_id(), main_door.clone());
  things.push(main_door_thing.clone());

  let main_door_thing_clone = main_door_thing.clone();
  door_bell_button
    .set_async_interrupt(
      Trigger::FallingEdge,
      on_change_debounce(move |_| {
        let main_door_thing = &main_door_thing_clone;

        log::info!("Door bell button pressed.");

        let event = Box::new(BaseEvent::new("bell".to_owned(), Some(json!(true))));
        main_door_thing.write().unwrap().add_event(event);
      }),
    )
    .unwrap();

  let mut cellar_door = Door::new(board.cellar_door_open, board.cellar_door_contact);
  let ring_clone = ring.clone();
  let cellar_door_thing = make_door_thing(&mut cellar_door, "cellar-door-1", "Cellar Door", false, move |closed| {
    let mut ring = ring_clone.lock().unwrap();
    ring.set_bottom_right(closed_to_color(closed));
    ring.render();
  });
  let cellar_door: Arc<RwLock<Box<dyn Any + Send + Sync>>> = Arc::new(RwLock::new(Box::new(cellar_door)));

  doors.insert(cellar_door_thing.read().unwrap().get_id(), cellar_door.clone());
  things.push(cellar_door_thing.clone());

  let garage_door_input = board.garage_door_2_contact;
  let stop_output = board.garage_door_2_stop;
  let open_output = board.garage_door_2_open;
  let close_output = board.garage_door_2_close;
  let mut garage_door = GarageDoor::new(stop_output, open_output, close_output, garage_door_input);
  let led_clone = led.clone();
  let ring_clone = ring.clone();
  let garage_door_thing = make_door_thing(&mut garage_door, "garage-door-1", "Garage Door", true, move |closed| {
    let mut ring = ring_clone.lock().unwrap();
    ring.set_top_right(closed_to_color(closed));
    ring.render();

    let mut led = led_clone.lock().unwrap();

    if closed {
      led.0.set_low();
      led.1.set_high();
    } else {
      led.0.set_high();
      led.1.set_low();
    }
    led.2.set_low();
  });
  let garage_door: Arc<RwLock<Box<dyn Any + Send + Sync>>> = Arc::new(RwLock::new(Box::new(garage_door)));

  let garage_door_clone = garage_door.clone();
  let led_clone = led.clone();
  garage_door_button
    .set_async_interrupt(
      Trigger::FallingEdge,
      on_change_debounce(move |_| {
        log::info!("Garage door button pressed.");

        let mut led = led_clone.lock().unwrap();
        led.0.set_high();
        led.1.set_high();
        led.2.set_low();

        let mut garage_door = garage_door_clone.write().unwrap();
        let garage_door = garage_door.downcast_mut::<GarageDoor>().unwrap();

        if garage_door.is_open() {
          garage_door.close()
        } else {
          garage_door.open()
        }
      }),
    )
    .unwrap();

  doors.insert(garage_door_thing.read().unwrap().get_id(), garage_door.clone());
  things.push(garage_door_thing.clone());

  let generator = Generator { doors };

  let socket = UdpSocket::bind("0.0.0.0:56000").unwrap();
  thread::spawn(move || {
    let mut buf = [0; 64];

    loop {
      match socket.recv_from(&mut buf) {
        Ok((size, _)) => match str::from_utf8(&buf[..size]) {
          Ok(s) => match s.parse::<ekey::multi::Multi>() {
            Ok(packet) => {
              dbg!(&packet);

              let value = serde_json::value::to_value(&packet).unwrap();
              let event = Box::new(BaseEvent::new("finger_scan".to_owned(), Some(value)));

              match packet.finger_scanner_name() {
                "HT" => {
                  let main_door = &mut *main_door_thing.write().unwrap();
                  main_door.add_event(event);
                },
                "KT" => {
                  let cellar_door = &mut *cellar_door_thing.write().unwrap();
                  cellar_door.add_event(event);
                },
                "GT" => {
                  let garage_door = &mut *garage_door_thing.write().unwrap();
                  garage_door.add_event(event);
                },
                _ => (),
              }
            },
            Err(_) => log::error!("Invalid EKEY message format."),
          },
          Err(_) => log::error!("Invalid EKEY request."),
        },
        Err(err) => log::error!("{}", err),
      }
    }
  });

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
