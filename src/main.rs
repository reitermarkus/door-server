use std::{
  any::Any,
  collections::HashMap,
  env,
  future::Future,
  io,
  ops::DerefMut,
  process, str,
  sync::{Arc, RwLock, Weak},
};

use rppal::gpio::{Gpio, Trigger};
use serde_json::json;
use tokio::{net::UdpSocket, sync::Mutex};
use webthing::{
  server::ActionGenerator, Action, BaseEvent, BaseProperty, BaseThing, Thing, ThingsType, WebThingServer,
};

use door_server::{on_change_debounce, Door, GarageDoor, StatefulDoor};

mod action;
use action::{LockAction, UnlockAction};

use door_server::{led::closed_to_color, Board};

struct Generator {
  doors: HashMap<String, Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>>>,
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

async fn make_door_thing<OC, F>(
  mut door: impl StatefulDoor,
  id: &str,
  name: &str,
  supports_locking: bool,
  mut on_change: OC,
) -> Arc<RwLock<Box<dyn Thing + 'static>>>
where
  F: Future + Send + Sync + 'static,
  OC: (FnMut(bool) -> F) + Send + Sync + 'static,
{
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
  on_change(door.is_closed()).await;

  let thing_clone = thing.clone();
  door.on_change(move |closed| {
    let thing = thing_clone.clone();
    let on_change = on_change(closed);

    async move {
      let thing = thing.write().unwrap();
      let value = door_state(Some(closed));
      set_property(thing, "lock", value);

      on_change.await
    }
  });

  thing
}

#[actix_rt::main]
async fn main() {
  env_logger::init();

  let port = env::var("PORT").map(|s| s.parse::<u16>().expect("Port is invalid")).unwrap_or(8888);

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
    let ring = ring_clone.clone();

    async move {
      let mut ring = ring.lock().await;
      ring.set_top_left(closed_to_color(closed));
      ring.render();
    }
  })
  .await;
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

  let main_door: Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>> =
    Arc::new(tokio::sync::RwLock::new(Box::new(main_door)));

  doors.insert(main_door_thing.read().unwrap().get_id(), main_door.clone());
  things.push(main_door_thing.clone());

  let main_door_thing_clone = main_door_thing.clone();
  door_bell_button
    .set_async_interrupt(
      Trigger::Both,
      on_change_debounce(move |closed| {
        let main_door_thing = main_door_thing_clone.clone();

        async move {
          if closed {
            log::info!("Door bell button pressed.");

            let event = Box::new(BaseEvent::new("bell".to_owned(), Some(json!(true))));
            main_door_thing.write().unwrap().add_event(event);
          } else {
            log::info!("Door bell button released.");
          }
        }
      }),
    )
    .unwrap();

  let mut cellar_door = Door::new(board.cellar_door_open, board.cellar_door_contact);
  let ring_clone = ring.clone();
  let cellar_door_thing = make_door_thing(&mut cellar_door, "cellar-door-1", "Cellar Door", false, move |closed| {
    let ring = ring_clone.clone();

    async move {
      let mut ring = ring.lock().await;
      ring.set_bottom_right(closed_to_color(closed));
      ring.render();
    }
  })
  .await;
  let cellar_door: Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>> =
    Arc::new(tokio::sync::RwLock::new(Box::new(cellar_door)));

  doors.insert(cellar_door_thing.read().unwrap().get_id(), cellar_door.clone());
  things.push(cellar_door_thing.clone());

  let mut garage_door = GarageDoor::new(
    board.garage_door_2_open,
    board.garage_door_2_stop,
    board.garage_door_2_close,
    board.garage_door_2_contact,
  );
  let led_clone = led.clone();
  let ring_clone = ring.clone();
  let garage_door_thing = make_door_thing(&mut garage_door, "garage-door-1", "Garage Door", true, move |closed| {
    let led = led_clone.clone();
    let ring = ring_clone.clone();

    async move {
      {
        let mut ring = ring.lock().await;
        ring.set_top_right(closed_to_color(closed));
        ring.render();
      }

      let mut led = led.lock().await;

      if closed {
        led.0.set_low();
        led.1.set_high();
      } else {
        led.0.set_high();
        led.1.set_low();
      }
      led.2.set_low();
    }
  })
  .await;
  let garage_door: Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>> =
    Arc::new(tokio::sync::RwLock::new(Box::new(garage_door)));

  let garage_door_clone = garage_door.clone();
  let led_clone = led.clone();
  garage_door_button
    .set_async_interrupt(
      Trigger::Both,
      on_change_debounce(move |closed| {
        let led = led_clone.clone();
        let garage_door = garage_door_clone.clone();

        async move {
          let mut led = led.lock().await;

          if closed {
            log::info!("Garage door button pressed.");

            led.0.set_high();
            led.1.set_high();
            led.2.set_high();

            let mut garage_door = garage_door.write().await;
            let garage_door = garage_door.downcast_mut::<GarageDoor>().unwrap();

            if garage_door.is_open() {
              log::info!("Garage is open, closing.");
              garage_door.close().await
            } else {
              log::info!("Garage is closed, opening.");
              garage_door.open().await
            }
          } else {
            log::info!("Garage door button released.");

            led.0.set_high();
            led.1.set_high();
            led.2.set_low();
          }
        }
      }),
    )
    .unwrap();

  doors.insert(garage_door_thing.read().unwrap().get_id(), garage_door.clone());
  things.push(garage_door_thing.clone());

  let generator = Generator { doors };

  let ekey_receiver = async {
    let socket: UdpSocket = UdpSocket::bind("0.0.0.0:56000").await?;
    let mut buf: [u8; 64] = [0; 64];
    loop {
      let (size, _) = socket.recv_from(&mut buf).await?;
      let message = str::from_utf8(&buf[..size]);
      match message {
        Ok(s) => match s.parse::<ekey::multi::Multi>() {
          Ok(packet) => {
            log::info!(
              "Received finger scanner {} action{} at {}.",
              packet.action(),
              packet.user_name().map(|name| format!(" by {name}")).unwrap_or_default(),
              packet.finger_scanner_name()
            );

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
              finger_scanner_name => log::warn!("Unknown finger scanner: {finger_scanner_name}"),
            }
          },
          Err(err) => log::error!("Invalid EKEY message format: {err:?}"),
        },
        Err(err) => log::error!("Invalid EKEY request: {err:?}"),
      }
    }

    #[allow(unused)]
    Ok::<_, io::Error>(())
  };

  let webthing_server = {
    let mut server = WebThingServer::new(
      ThingsType::Multiple(things, "DoorServer".to_owned()),
      Some(port),
      None,
      None,
      Box::new(generator),
      None,
      Some(true),
    );

    log::info!("Starting WebThing server on port {port}…");
    server.start(None)
  };

  tokio::try_join!(ekey_receiver, webthing_server).unwrap_or_else(|err: io::Error| {
    log::error!("{}", err);
    process::exit(1);
  });
}
