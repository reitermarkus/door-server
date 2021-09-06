use embedded_hal_0::blocking::delay::DelayMs;
use rppal::spi::{Bus, Mode as SpiMode, Spi, SlaveSelect};
use rppal::hal::Delay;
use smart_leds::{SmartLedsWrite, RGB8};

const fn encode_byte(mut data: u8) -> [u8; 4] {
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

fn encode_colors(data: &[RGB8]) -> Vec<u8> {
  let mut buffer = vec![];

  buffer.push(0);

  for pixel in data {
    buffer.extend(&encode_byte(pixel.g));
    buffer.extend(&encode_byte(pixel.r));
    buffer.extend(&encode_byte(pixel.b));
  }

  for _ in 0..20 {
    buffer.push(0);
  }

  buffer
}

pub fn test() {
  log::info!("Starting LED test thread.");

  let mut delay = Delay;

  let mut data = [RGB8::default(); 12];

  let empty = [RGB8::default(); 12];

  // Raspberry Pi `/boot/config.txt` must be set to use a core frequency of 250 MHz.
  let spi_freq = 800_000 * 3;

  let mut spi = Spi::new(
    Bus::Spi0,
    SlaveSelect::Ss0,
    spi_freq,
    SpiMode::Mode0,
  ).unwrap();

  // let mut output_buffer = [0; 20 + (12 * 12)];
  // let mut ws = Ws2812::new(spi, &mut output_buffer);

  let mut index = 0;
  loop {

    for i in 0..(data.len() / 3) {
      data[i * 3 + 0] = RGB8 {
          r: 0,
          g: 0,
          b: 0x10,
      };
      data[i * 3 + 1] = RGB8 {
          r: 0,
          g: 0x10,
          b: 0,
      };
      data[i * 3 + 2] = RGB8 {
          r: 0x10,
          g: 0,
          b: 0,
      };
    }

    data[index].r = 0x10;
    data[index].g = 0x10;
    data[index].b = 0x00;
    index = (index + 1) % data.len();


    let pixel_buf = encode_colors(&data);

    spi.write(&pixel_buf).unwrap();
    // ws.write(data.iter().cloned()).unwrap();
    delay.delay_ms(250 as u16);

    let pixel_buf = encode_colors(&empty);

    spi.write(&pixel_buf).unwrap();
    // ws.write(empty.iter().cloned()).unwrap();

    delay.delay_ms(250 as u16);
  }
}
