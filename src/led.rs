use rppal::spi::{Bus, Mode as SpiMode, Spi, SlaveSelect};
use smart_leds::RGB8;

const fn encode_byte(data: u8) -> [u8; 4] {
  const fn encode_bit(bit: u8) -> u8 {
    0b1000 + bit * 0b0110
  }

  const fn encode_crumb(crumb: u8) -> u8 {
    (encode_bit(crumb >> 1) << 4) +
    encode_bit(crumb & 1)
  }

  [
    encode_crumb(data >> 6),
    encode_crumb((data >> 4) & 0b11),
    encode_crumb((data >> 2) & 0b11),
    encode_crumb(data & 0b11),
  ]
}

fn encode_colors(buffer: &mut Vec<u8>, data: &[RGB8]) {
  buffer.clear();
  buffer.push(0);

  for pixel in data {
    buffer.extend(&encode_byte(pixel.g));
    buffer.extend(&encode_byte(pixel.r));
    buffer.extend(&encode_byte(pixel.b));
  }

  for _ in 0..20 {
    buffer.push(0);
  }
}

pub struct RgbRing {
  spi: Spi,
  colors: [RGB8; 12],
  buffer: Vec<u8>,
}

impl RgbRing {
  pub fn new() -> Self {
    // Raspberry Pi `/boot/config.txt` must be set to use a core frequency of 250 MHz.
    let spi_freq = 800_000 * 3;

    Self {
      spi: Spi::new(
        Bus::Spi0,
        SlaveSelect::Ss0,
        spi_freq,
        SpiMode::Mode0,
      ).unwrap(),
      colors: Default::default(),
      buffer: Vec::new(),
    }
  }

  pub fn set_top_right(&mut self, color: RGB8) {
    self.colors[0] = color;
    self.colors[1] = color;
    self.colors[2] = color;
  }

  pub fn set_bottom_right(&mut self, color: RGB8) {
    self.colors[3] = color;
    self.colors[4] = color;
    self.colors[5] = color;
  }

  pub fn set_bottom_left(&mut self, color: RGB8) {
    self.colors[6] = color;
    self.colors[7] = color;
    self.colors[8] = color;
  }

  pub fn set_top_left(&mut self, color: RGB8) {
    self.colors[9] = color;
    self.colors[10] = color;
    self.colors[11] = color;
  }

  pub fn render(&mut self) {
    encode_colors(&mut self.buffer, &self.colors);
    self.spi.write(&self.buffer).unwrap();
  }
}

pub fn closed_to_color(closed: bool) -> RGB8 {
  if closed {
    RGB8 {
      r: 0x00,
      g: 0x0f,
      b: 0x01,
    }
  } else {
    RGB8 {
      r: 0x14,
      g: 0x00,
      b: 0x00,
    }
  }
}

