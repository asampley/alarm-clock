use core::ops::{Deref, DerefMut};

use embassy_time::{Duration, Instant};
use embedded_hal::digital::InputPin;
use embedded_hal_async::digital::Wait;

pub struct Button<Pin> {
	pin: Pin,
	bounce_time: Duration,
	last_event: Option<Instant>,
}

impl<Pin> Button<Pin> {
	// expects pull up pin
	pub fn new(pin: Pin, bounce_time: Duration) -> Self {
		Self {
			pin,
			bounce_time: bounce_time,
			last_event: None,
		}
	}
}

impl<Pin> Deref for Button<Pin> {
	type Target = Pin;

	fn deref(&self) -> &Self::Target {
		&self.pin
	}
}

impl<Pin> DerefMut for Button<Pin> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.pin
	}
}

impl<Pin: Wait + InputPin> Button<Pin> {
	pub async fn wait_press_release(&mut self) -> Result<(), Pin::Error> {
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
