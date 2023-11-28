use rppal::{gpio::{Gpio, Trigger}, hal::Delay};
use embedded_hal::delay::DelayUs;

#[derive(PartialEq)]
enum Dir {
  In,
  Out,
}

fn test_gpio(gpio: &mut Gpio, delay: &mut impl DelayUs, description: &str, n: u8, dir: Dir) {
  println!("Testing GPIO{n} ({description}):");
  let mut gpio = gpio.get(n).unwrap();

  match dir {
    Dir::In => {
      let mut gpio = gpio.into_input_pullup();

      println!("Waiting for GPIO{n} ({description}) to be pulled low.");

      while gpio.is_high()  {
        delay.delay_ms(10).unwrap();
      }

      println!("GPIO{n} was pulled low.");

      while gpio.is_low()  {
        delay.delay_ms(10).unwrap();
      }

      delay.delay_ms(1000).unwrap();
    },
    Dir::Out => {
      let mut gpio = gpio.into_output_high();

      println!("Toggling GPIO{n} ({description}).");

      gpio.set_low();
      delay.delay_ms(1000).unwrap();
      gpio.set_high();
      delay.delay_ms(1000).unwrap();
    }
  }
}

fn main() {
  let mut gpio = Gpio::new().unwrap();
  let mut delay = Delay::new();

  test_gpio(&mut gpio, &mut delay, "Main Door Open", 19, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Main Door Bell", 0, Dir::In);
  test_gpio(&mut gpio, &mut delay, "Main Door Contact", 17, Dir::In);

  test_gpio(&mut gpio, &mut delay, "Cellar Door Open", 26, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Cellar Door Contact", 1, Dir::In);

  test_gpio(&mut gpio, &mut delay, "Garage Door 1 Open", 13, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Garage Door 1 Stop", 6, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Garage Door 1 Close", 5, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Garage Door 1 Contact", 2, Dir::In);

  test_gpio(&mut gpio, &mut delay, "Garage Door 2 Open", 21, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Garage Door 2 Stop", 20, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Garage Door 2 Close", 16, Dir::Out);
  test_gpio(&mut gpio, &mut delay, "Garage Door 2 Contact", 25, Dir::In);
}
