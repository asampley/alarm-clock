use core::ops::{Deref, DerefMut};

use embedded_hal::digital::OutputPin;

pub struct Buzzer<Pin: OutputPin> {
	pin: Pin,
}

impl<Pin: OutputPin> Buzzer<Pin> {
	pub fn new(pin: Pin) -> Self {
		Self { pin }
	}
}

impl<Pin: OutputPin> Deref for Buzzer<Pin> {
	type Target = Pin;

	fn deref(&self) -> &Self::Target {
		&self.pin
	}
}

impl<Pin: OutputPin> DerefMut for Buzzer<Pin> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pin
	}
}
