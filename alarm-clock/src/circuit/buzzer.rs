use core::ops::{Deref, DerefMut};

use embedded_hal::digital::{ErrorType, OutputPin};

/// Required to be concrete for embassy tasks
pub type Pin = impl OutputPin + ErrorType<Error: defmt::Format>;
pub type Error = <Pin as ErrorType>::Error;

pub struct Buzzer {
	pin: Pin,
}

impl Buzzer {
	pub fn new(pin: Pin) -> Self {
		Self { pin }
	}
}

impl Deref for Buzzer {
	type Target = Pin;

	fn deref(&self) -> &Self::Target {
		&self.pin
	}
}

impl DerefMut for Buzzer {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pin
	}
}
