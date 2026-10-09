use derive_more::{Deref, DerefMut};

#[derive(Deref, DerefMut)]
pub struct Buzzer<Pin> {
	pin: Pin,
}

impl<Pin> Buzzer<Pin> {
	pub fn new(pin: Pin) -> Self {
		Self { pin }
	}
}
