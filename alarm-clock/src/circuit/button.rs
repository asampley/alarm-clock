use core::ops::{Deref, DerefMut};

use embassy_time::{Duration, Instant};
use embedded_hal::digital::{ErrorType, InputPin};
use embedded_hal_async::digital::Wait;

/// Required to be concrete for embassy tasks
pub type Pin = impl InputPin + Wait + ErrorType<Error: defmt::Format>;
pub type Error = <Pin as ErrorType>::Error;

pub struct Button {
	pin: Pin,
	bounce_time: Duration,
	last_event: Option<Instant>,
}

impl Button {
	// expects pull up pin
	pub fn new(pin: Pin, bounce_time: Duration) -> Self {
		Self {
			pin,
			bounce_time,
			last_event: None,
		}
	}

	pub async fn wait_press_release(&mut self) -> Result<(), Error> {
		loop {
			self.wait_for_any_edge().await?;

			if self
				.last_event
				.map_or(true, |e| e + self.bounce_time <= Instant::now())
			{
				self.last_event = Some(Instant::now());

				return Ok(());
			}
		}
	}
}

impl Deref for Button {
	type Target = Pin;

	fn deref(&self) -> &Self::Target {
		&self.pin
	}
}

impl DerefMut for Button {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pin
	}
}
