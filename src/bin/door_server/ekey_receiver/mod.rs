use tokio::{net::UdpSocket, sync::broadcast};

use crate::event::Event;

pub async fn start(tx: broadcast::Sender<Event>) -> Result<(), std::io::Error> {
  let socket = UdpSocket::bind("0.0.0.0:56000").await?;
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

          match tx.send(Event::FingerScan(packet)) {
            Ok(_) => (),
            Err(err) => {
              log::error!("Failed to send finger scan event: {err:?}");
              return Ok(());
            },
          }
        },
        Err(err) => log::error!("Invalid EKEY message format: {err:?}"),
      },
      Err(err) => log::error!("Invalid EKEY request: {err:?}"),
    }
  }
}
