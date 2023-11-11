use rppal::gpio::{Gpio, OutputPin};

pub struct WaveshareRelay {
  pub ch1: OutputPin,
  pub ch2: OutputPin,
  pub ch3: OutputPin,
  pub ch4: OutputPin,
  pub ch5: OutputPin,
  pub ch6: OutputPin,
  pub ch7: OutputPin,
  pub ch8: OutputPin,
}

impl WaveshareRelay {
  pub fn new(gpio: &mut Gpio) -> Self {
    let mut ch1 = gpio.get(5).unwrap().into_output();
    ch1.set_high();
    let mut ch2 = gpio.get(6).unwrap().into_output();
    ch2.set_high();
    let mut ch3 = gpio.get(13).unwrap().into_output();
    ch3.set_high();
    let mut ch4 = gpio.get(16).unwrap().into_output();
    ch4.set_high();
    let mut ch5 = gpio.get(19).unwrap().into_output();
    ch5.set_high();
    let mut ch6 = gpio.get(20).unwrap().into_output();
    ch6.set_high();
    let mut ch7 = gpio.get(21).unwrap().into_output();
    ch7.set_high();
    let mut ch8 = gpio.get(26).unwrap().into_output();
    ch8.set_high();

    Self { ch1, ch2, ch3, ch4, ch5, ch6, ch7, ch8 }
  }
}
