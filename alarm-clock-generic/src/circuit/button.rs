use derive_more::{Deref, DerefMut};
use embassy_time::{Duration, Instant};
use embedded_hal::digital::InputPin;
use embedded_hal_async::digital::Wait;

#[derive(Deref, DerefMut)]
pub struct Button<Pin> {
	#[deref]
	#[deref_mut]
	pin: Pin,
	bounce_time: Duration,
	last_event: Option<Instant>,
}

impl<Pin: InputPin + Wait> Button<Pin> {
	// expects pull up pin
	pub fn new(pin: Pin, bounce_time: Duration) -> Self {
		Self {
			pin,
			bounce_time,
			last_event: None,
		}
	}

	pub async fn wait_press_release(&mut self) -> Result<(), Pin::Error> {
		loop {
			self.wait_for_any_edge().await?;

			if self
				.last_event
				.is_none_or(|e| e + self.bounce_time <= Instant::now())
			{
				self.last_event = Some(Instant::now());

				return Ok(());
			}
		}
	}
}
