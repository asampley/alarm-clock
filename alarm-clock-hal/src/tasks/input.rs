use embassy_executor::{SpawnError, SpawnToken};
use embedded_hal::digital::{ErrorType, InputPin};
use embedded_hal_async::digital::Wait;

use crate::channel::EventSender;
use crate::channel::event::{ButtonEvent, ButtonFunction};
use crate::circuit::button::Button;
use crate::error;

/// Note: needs pool size 3
pub type PollInputTask<S, P> =
	fn(EventSender, Button<P>, ButtonFunction) -> Result<SpawnToken<S>, SpawnError>;

pub async fn poll_input<Pin: InputPin + Wait + ErrorType<Error: defmt_or_log::FormatOrDebug>>(
	event_sender: EventSender,
	mut button: Button<Pin>,
	function: ButtonFunction,
) -> ! {
	loop {
		let _ = button
			.wait_press_release()
			.await
			.inspect_err(|e| error!("{:?}", e));

		match button.is_high() {
			Err(e) => error!("{:?}", e),
			Ok(high) => {
				if high {
					event_sender
						.send(ButtonEvent::Release(function).into())
						.await;
				} else {
					event_sender.send(ButtonEvent::Press(function).into()).await;
				}
			}
		}
	}
}
