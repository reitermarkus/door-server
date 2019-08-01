pub const RELAY_IN1_PIN: u8 = 4;
pub const RELAY_IN2_PIN: u8 = 5;
pub const RELAY_IN3_PIN: u8 = 6;
pub const RELAY_IN4_PIN: u8 = 7;

pub const INPUT_PIN: u8 = 8;

mod door;
pub use crate::door::GarageDoor;
