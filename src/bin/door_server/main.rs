use std::{any::Any, sync::Arc};

use rppal::gpio::Gpio;
use tokio::{
  signal::unix::{SignalKind, signal},
  sync::{Mutex, broadcast},
};

use door_server::{Button, Door, GarageDoor, StatefulDoor};

mod ekey_receiver;
mod esphome_server;
mod event;

use door_server::{Board, led::closed_to_color};

use crate::event::{ButtonId, DoorCommand, DoorId, Event, GarageDoorCommand};

#[tokio::main]
async fn main() {
  env_logger::init();

  let gpio = Gpio::new().unwrap();
  let board = Board::new(gpio);

  let led = Arc::new(Mutex::new(board.led));
  let ring = Arc::new(Mutex::new(board.ring));

  let (event_tx, event_rx) = broadcast::channel(8);

  let mut door_bell = Button::new(board.main_door_bell);
  let mut garage_door_button = Button::new(board.garage_door_button);

  let mut main_door = Door::new(board.main_door_open, board.main_door_contact);

  async fn set_on_change<OC, F>(mut door: impl StatefulDoor, mut on_change: OC)
  where
    F: Future + Send + Sync + 'static,
    OC: (FnMut(bool) -> F) + Send + Sync + 'static,
  {
    // Initialize at start.
    on_change(door.is_closed()).await;

    door.on_change(move |closed| on_change(closed));
  }

  let event_tx_clone = event_tx.clone();
  set_on_change(&mut main_door, move |closed| {
    let event_tx = event_tx_clone.clone();

    async move {
      let _ = event_tx.send(Event::DoorContact(DoorId::Main, closed));
    }
  })
  .await;

  let main_door: Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>> =
    Arc::new(tokio::sync::RwLock::new(Box::new(main_door)));

  let event_tx_clone = event_tx.clone();
  door_bell.on_change(move |pressed| {
    let event_tx = event_tx_clone.clone();

    async move {
      if pressed {
        let _ = event_tx.send(Event::ButtonPress(ButtonId::DoorBell));
      }
    }
  });

  let mut cellar_door = Door::new(board.cellar_door_open, board.cellar_door_contact);
  let event_tx_clone = event_tx.clone();
  set_on_change(&mut cellar_door, move |closed| {
    let event_tx = event_tx_clone.clone();

    async move {
      let _ = event_tx.send(Event::DoorContact(DoorId::Cellar, closed));
    }
  })
  .await;
  let cellar_door: Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>> =
    Arc::new(tokio::sync::RwLock::new(Box::new(cellar_door)));

  let mut garage_door = GarageDoor::new(
    board.garage_door_2_open,
    board.garage_door_2_stop,
    board.garage_door_2_close,
    board.garage_door_2_contact,
  );
  let event_tx_clone = event_tx.clone();
  set_on_change(&mut garage_door, move |closed| {
    let event_tx = event_tx_clone.clone();

    async move {
      let _ = event_tx.send(Event::DoorContact(DoorId::Garage, closed));
      let _ = event_tx.send(Event::Refresh);
    }
  })
  .await;
  let garage_door: Arc<tokio::sync::RwLock<Box<dyn Any + Send + Sync>>> =
    Arc::new(tokio::sync::RwLock::new(Box::new(garage_door)));

  let event_tx_clone = event_tx.clone();
  let garage_door_clone = garage_door.clone();
  let led_clone = led.clone();
  garage_door_button.on_change(move |pressed| {
    let event_tx = event_tx_clone.clone();

    let led = led_clone.clone();
    let garage_door = garage_door_clone.clone();

    async move {
      let mut led = led.lock().await;

      if pressed {
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

        let _ = event_tx.send(Event::ButtonPress(ButtonId::GarageDoor));
      } else {
        log::info!("Garage door button released.");

        led.0.set_high();
        led.1.set_high();
        led.2.set_low();
      }
    }
  });

  let ekey_receiver = ekey_receiver::start(event_tx.clone());

  let event_tx_clone = event_tx.clone();
  let event_rx_clone = event_rx.resubscribe();
  let event_handler = async move {
    let event_tx = event_tx_clone;
    let mut event_rx = event_rx_clone;
    while let Ok(event) = event_rx.recv().await {
      match event {
        Event::Refresh => {
          log::info!("Refresh door states.");

          let main_door = &mut *main_door.write().await;
          let main_door = main_door.downcast_mut::<Door>().unwrap();

          let cellar_door = &mut *cellar_door.write().await;
          let cellar_door = cellar_door.downcast_mut::<Door>().unwrap();

          let garage_door = &mut *garage_door.write().await;
          let garage_door = garage_door.downcast_mut::<GarageDoor>().unwrap();

          event_tx.send(Event::DoorContact(DoorId::Main, main_door.is_closed())).unwrap();
          event_tx.send(Event::DoorContact(DoorId::Cellar, cellar_door.is_closed())).unwrap();
          event_tx.send(Event::DoorContact(DoorId::Garage, garage_door.is_closed())).unwrap();

          let garage_door_state = garage_door.state();
          log::info!("Garage door state: {:?}", garage_door_state);
          event_tx.send(Event::GarageDoorPosition(garage_door_state)).unwrap();

          if !garage_door_state.is_stopped() {
            event_tx.send(Event::Refresh).unwrap();
          }
        },
        Event::DoorCommand(DoorId::Main, DoorCommand::Unlock) => {
          let main_door = &mut *main_door.write().await;
          let main_door = main_door.downcast_mut::<Door>().unwrap();
          main_door.open().await;
        },
        Event::DoorCommand(DoorId::Cellar, DoorCommand::Unlock) => {
          let cellar_door = &mut *cellar_door.write().await;
          let cellar_door = cellar_door.downcast_mut::<Door>().unwrap();
          cellar_door.open().await;
        },
        Event::DoorCommand(DoorId::Garage, DoorCommand::Unlock) => {
          unreachable!();
        },
        Event::GarageDoorCommand(command) => {
          log::info!("Garage door command received: {:?}", command);

          let garage_door = &mut *garage_door.write().await;
          let garage_door = garage_door.downcast_mut::<GarageDoor>().unwrap();

          event_tx.send(Event::DoorContact(DoorId::Garage, garage_door.is_closed())).unwrap();

          match command {
            GarageDoorCommand::Open => garage_door.open().await,
            GarageDoorCommand::Stop => garage_door.stop().await,
            GarageDoorCommand::Close => garage_door.close().await,
          }

          event_tx.send(Event::Refresh).unwrap();
        },
        Event::ButtonPress(ButtonId::DoorBell) => {},
        Event::ButtonPress(ButtonId::GarageDoor) => {},
        Event::DoorContact(DoorId::Main, closed) => {
          let mut ring = ring.lock().await;
          ring.set_top_left(closed_to_color(closed));
          ring.render();
        },
        Event::DoorContact(DoorId::Cellar, closed) => {
          let mut ring = ring.lock().await;
          ring.set_bottom_right(closed_to_color(closed));
          ring.render();
        },
        Event::DoorContact(DoorId::Garage, closed) => {
          let mut ring = ring.lock().await;
          ring.set_top_right(closed_to_color(closed));
          ring.render();

          let mut led = led.lock().await;
          if closed {
            led.0.set_low();
            led.1.set_high();
            led.2.set_low();
          } else {
            led.0.set_high();
            led.1.set_low();
            led.2.set_low();
          }
        },
        Event::GarageDoorPosition(_) => {},
        Event::FingerScan(packet) => {
          let value = serde_json::value::to_value(&packet).unwrap();

          match packet.finger_scanner_name() {
            "HT" => {},
            "KT" => {},
            "GT" => {},
            finger_scanner_name => log::warn!("Unknown finger scanner: {finger_scanner_name}"),
          }
        },
      }
    }
  };

  let esphome_server = esphome_server::start(event_tx.clone(), event_rx.resubscribe());

  let sigint = async { signal(SignalKind::interrupt()).unwrap().recv().await };
  let sigterm = async { signal(SignalKind::terminate()).unwrap().recv().await };

  tokio::select! {
    _ = sigint => {
      log::info!("Received SIGINT, stopping server.");
    },
    _ = sigterm => {
      log::info!("Received SIGTERM, stopping server.");
    },
    _ = ekey_receiver => (),
    _ = event_handler => (),
    _ = esphome_server => (),
  }
}
