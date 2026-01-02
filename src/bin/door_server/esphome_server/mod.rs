use std::{env, net::SocketAddr, sync::Arc};

use esphome_native_api::{
  esphomeapi::EspHomeApi,
  parser::ProtoMessage,
  proto::version_2025_12_1::{
    BinarySensorStateResponse, CoverCommandRequest, CoverStateResponse, EventResponse, ListEntitiesDoneResponse,
    ListEntitiesRequest, LockCommand, LockCommandRequest, LockState, LockStateResponse, SubscribeStatesRequest,
  },
};
use mac_address::get_mac_address;
use tokio::{net::TcpSocket, sync::broadcast};

use crate::event::{ButtonId, DoorCommand, DoorId, Event, GarageDoorCommand};

mod entities;

pub async fn start(event_tx: broadcast::Sender<Event>, event_rx: broadcast::Receiver<Event>) {
  let addr = SocketAddr::from(([0, 0, 0, 0], 6053));
  let socket = TcpSocket::new_v4().unwrap();
  socket.set_reuseaddr(true).unwrap();
  socket.bind(addr).unwrap();

  let listener = socket.listen(128).unwrap();
  log::debug!("Listening on: {addr}");

  let mac_address = get_mac_address().unwrap().unwrap_or_default();
  let encryption_key = env::var("ESPHOME_ENCRYPTION_KEY").unwrap_or_default();

  let entities = Arc::new(entities::entities());

  loop {
    log::info!("Waiting for connection.");
    let stream = match listener.accept().await {
      Ok((stream, _)) => stream,
      Err(err) => {
        log::error!("Failed to accept connection: {err}");
        break;
      },
    };

    let peer_addr = stream.peer_addr().unwrap();
    log::info!("Accepted request from {peer_addr}.");

    let entities = entities.clone();

    let event_tx = event_tx.clone();
    let event_rx = event_rx.resubscribe();

    let encryption_key = encryption_key.clone();
    tokio::task::spawn(async move {
      log::info!("Starting ESPHome API server to {peer_addr}.");

      let mut server = EspHomeApi::builder()
        .api_version_major(1)
        .api_version_minor(42)
        .server_info("ESPHome Rust".into())
        .esphome_version("2025.12.1".into()) // FIXME: Should be set by `esphome-native-api` automatically.
        .name("door_server".into())
        .friendly_name("Door Server".into())
        .mac(mac_address.to_string())
        // .bluetooth_mac_address(bluetooth_mac_address.to_string())
        // .bluetooth_proxy_feature_flags(0b1111111)
        .manufacturer("Markus Reiter".to_string())
        .model("Door Server".to_string())
        // .suggested_area("".to_string())
        .encryption_key(encryption_key.clone())
        .build();

      let (tx, mut rx) = server.start(stream).await.expect("Failed to start server");

      loop {
        let message = match rx.recv().await {
          Ok(message) => message,
          Err(broadcast::error::RecvError::Closed) => {
            log::info!("Connection to {peer_addr} closed.");
            break;
          },
          Err(broadcast::error::RecvError::Lagged(n)) => {
            log::warn!("Receiver lagged, {n} messages lost.");
            continue;
          },
        };

        match message {
          ProtoMessage::ListEntitiesRequest(ListEntitiesRequest {}) => {
            log::info!("ListEntitiesRequest");

            for entity in entities.values() {
              tx.send(entity.clone()).await.unwrap();
            }

            tx.send(ProtoMessage::ListEntitiesDoneResponse(ListEntitiesDoneResponse {})).await.unwrap();
          },
          ProtoMessage::SubscribeStatesRequest(SubscribeStatesRequest {}) => {
            let tx = tx.clone();
            let mut event_rx = event_rx.resubscribe();

            let event_tx = event_tx.clone();
            tokio::spawn(async move {
              event_tx.send(Event::Refresh).unwrap();

              while let Ok(event) = event_rx.recv().await {
                match event {
                  Event::ButtonPress(ButtonId::DoorBell) => {
                    tx.send(ProtoMessage::EventResponse(EventResponse {
                      device_id: 0,
                      key: 0, // TOOD: Get from map.
                      event_type: "ring".into(),
                    }))
                    .await
                    .unwrap();
                  },
                  Event::DoorContact(DoorId::Main, closed) => {
                    tx.send(ProtoMessage::BinarySensorStateResponse(BinarySensorStateResponse {
                      device_id: 0,
                      key: 1,
                      state: !closed,
                      missing_state: false,
                    }))
                    .await
                    .unwrap();
                    tx.send(ProtoMessage::LockStateResponse(LockStateResponse {
                      device_id: 0,
                      key: 2,
                      state: if closed { LockState::Locked } else { LockState::Unlocked } as i32,
                    }))
                    .await
                    .unwrap();
                  },
                  Event::DoorContact(DoorId::Cellar, closed) => {
                    tx.send(ProtoMessage::BinarySensorStateResponse(BinarySensorStateResponse {
                      device_id: 0,
                      key: 3,
                      state: !closed,
                      missing_state: false,
                    }))
                    .await
                    .unwrap();
                    tx.send(ProtoMessage::LockStateResponse(LockStateResponse {
                      device_id: 0,
                      key: 4,
                      state: if closed { LockState::Locked } else { LockState::Unlocked } as i32,
                    }))
                    .await
                    .unwrap();
                  },
                  Event::DoorContact(DoorId::Garage, closed) => {
                    tx.send(ProtoMessage::BinarySensorStateResponse(BinarySensorStateResponse {
                      device_id: 0,
                      key: 5,
                      state: !closed,
                      missing_state: false,
                    }))
                    .await
                    .unwrap();
                  },
                  Event::GarageDoorPosition(state) => {
                    tx.send(ProtoMessage::CoverStateResponse(CoverStateResponse {
                      device_id: 0,
                      key: 6,
                      #[allow(deprecated)]
                      legacy_state: 0,
                      position: state.position(),
                      tilt: 0.0,
                      current_operation: state.cover_operation() as i32,
                    }))
                    .await
                    .unwrap();
                  },
                  _ => (),
                }
              }
            });
          },
          ProtoMessage::LockCommandRequest(LockCommandRequest { device_id: 0, key, command, .. }) => {
            match LockCommand::try_from(command) {
              Ok(LockCommand::LockUnlock | LockCommand::LockOpen) => {
                if key == 2 {
                  event_tx.send(Event::DoorCommand(DoorId::Main, DoorCommand::Unlock)).unwrap();
                }

                if key == 4 {
                  event_tx.send(Event::DoorCommand(DoorId::Cellar, DoorCommand::Unlock)).unwrap();
                }

                log::info!("Opening door {key}.");
              },
              _ => continue,
            }
          },
          ProtoMessage::CoverCommandRequest(CoverCommandRequest { device_id: 0, key, position, stop, .. }) => {
            log::info!("CoverCommandRequest: key={key}, stop={stop}, position={position}");

            if stop {
              event_tx.send(Event::GarageDoorCommand(GarageDoorCommand::Stop)).unwrap();
            } else if position == 1.0 {
              event_tx.send(Event::GarageDoorCommand(GarageDoorCommand::Open)).unwrap();
            } else if position == 0.0 {
              event_tx.send(Event::GarageDoorCommand(GarageDoorCommand::Close)).unwrap();
            }
          },
          message => {
            log::warn!("Unhandled message: {:?}", message);
          },
        }
      }
    });
  }
}
