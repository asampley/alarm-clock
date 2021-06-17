use rppal::gpio::{ Gpio, InputPin, Trigger };

pub struct Button {
	pin: InputPin,
}

impl Button {
	pub fn new(pin_num: u8) -> rppal::gpio::Result<Self> {
		let mut pin = Gpio::new()?.get(pin_num)?.into_input_pullup();

		pin.set_interrupt(Trigger::Both)?;

		Ok(Button { pin: pin })
	}

	pub fn input_pin(&self) -> &InputPin {
		&self.pin
	}
}
