use std::{sync::Arc, time::Duration};

use ekey::{Action, multi::DigitalInput};
use rppal::gpio::Gpio;
use tokio::{
  signal::unix::{SignalKind, signal},
  sync::{Mutex, RwLock, broadcast},
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

  let event_tx_clone = event_tx.clone();
  let main_door = Door::new(board.main_door_open, board.main_door_contact, move |closed| {
    let event_tx = event_tx_clone.clone();

    async move {
      log::info!("Main door callback: closed={closed}");
      let _ = event_tx.send(Event::DoorContact(DoorId::Main, closed));
    }
  })
  .await;
  let main_door = Arc::new(RwLock::new(main_door));

  async fn set_on_change<OC, F>(mut door: impl StatefulDoor, mut on_change: OC)
  where
    F: Future + Send + Sync + 'static,
    OC: (FnMut(bool) -> F) + Send + Sync + 'static,
  {
    // Initialize at start.
    on_change(door.is_closed()).await;

    door.on_change(on_change);
  }

  let event_tx_clone = event_tx.clone();
  door_bell.on_change(move |pressed| {
    let event_tx = event_tx_clone.clone();

    async move {
      if pressed {
        let _ = event_tx.send(Event::ButtonPress(ButtonId::DoorBell));
      }
    }
  });

  let event_tx_clone = event_tx.clone();
  let cellar_door = Door::new(board.cellar_door_open, board.cellar_door_contact, move |closed| {
    let event_tx = event_tx_clone.clone();

    async move {
      let _ = event_tx.send(Event::DoorContact(DoorId::Cellar, closed));
    }
  })
  .await;
  let cellar_door = Arc::new(RwLock::new(cellar_door));

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
  let garage_door = Arc::new(RwLock::new(garage_door));

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
          main_door.force_update().await;

          let cellar_door = &mut *cellar_door.write().await;
          cellar_door.force_update().await;

          let garage_door = &mut *garage_door.write().await;
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
          main_door.open().await;
        },
        Event::DoorCommand(DoorId::Cellar, DoorCommand::Unlock) => {
          let cellar_door = &mut *cellar_door.write().await;
          cellar_door.open().await;
        },
        Event::DoorCommand(DoorId::Garage, DoorCommand::Unlock) => {
          unreachable!();
        },
        Event::GarageDoorCommand(command) => {
          log::info!("Garage door command received: {:?}", command);

          let garage_door = &mut *garage_door.write().await;

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
            "HT" => {
              if packet.action() == Action::Open {
                let main_door = &mut *main_door.write().await;
                main_door.handle_external_open().await;
                event_tx.send(Event::Refresh).unwrap();
              }
            },
            "KT" => {
              if packet.action() == Action::Open {
                let cellar_door = &mut *cellar_door.write().await;
                cellar_door.handle_external_open().await;
                event_tx.send(Event::Refresh).unwrap();
              }
            },
            "GT" => {
              if packet.action() == Action::Open {
                let garage_door = &mut *garage_door.write().await;
                garage_door.handle_external_open(Duration::from_secs(0)); // TODO: Delay.
                event_tx.send(Event::Refresh).unwrap();
              }
            },
            "****" => {
              if packet.action() == Action::DigitalInput {
                match packet.input().unwrap() {
                  DigitalInput::Input1 => {
                    let main_door = &mut *main_door.write().await;
                    main_door.handle_external_open().await;
                    event_tx.send(Event::Refresh).unwrap();
                  },
                  DigitalInput::Input2 => {
                    let cellar_door = &mut *cellar_door.write().await;
                    cellar_door.handle_external_open().await;
                    event_tx.send(Event::Refresh).unwrap();
                  },
                  DigitalInput::Input3 => {
                    let garage_door = &mut *garage_door.write().await;
                    garage_door.handle_external_open(Duration::from_secs(0)); // TODO: Delay.
                    event_tx.send(Event::Refresh).unwrap();
                  },
                  DigitalInput::Input4 => {
                    let garage_door = &mut *garage_door.write().await;
                    garage_door.handle_external_stop(Duration::from_secs(0)); // TODO: Delay.
                    event_tx.send(Event::Refresh).unwrap();
                  },
                }
              }
            },
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
