use door_server::GarageDoorState;

#[derive(Debug, Clone, Copy)]
pub enum ButtonId {
  DoorBell,
  GarageDoor,
}

#[derive(Debug, Clone, Copy)]
pub enum DoorId {
  Main,
  Cellar,
  Garage,
}

#[derive(Debug, Clone, Copy)]
pub enum DoorCommand {
  Unlock,
}

#[derive(Debug, Clone, Copy)]
pub enum GarageDoorCommand {
  Open,
  Stop,
  Close,
}

#[derive(Debug, Clone)]
pub enum Event {
  Refresh,
  DoorCommand(DoorId, DoorCommand),
  GarageDoorCommand(GarageDoorCommand),
  ButtonPress(ButtonId),
  DoorContact(DoorId, bool),
  GarageDoorPosition(GarageDoorState),
  FingerScan(ekey::multi::Multi),
}
