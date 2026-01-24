use embedded_hal::digital::InputPin;

use crate::message::ButtonFunction;
use crate::{error, Sender};

use crate::circuit::button::Button;
use crate::message::{ButtonEvent, EventMessage};

#[embassy_executor::task(pool_size = 3)]
pub async fn poll_input(
	event_sender: Sender<EventMessage, 16>,
	mut button: Button,
	function: ButtonFunction,
) -> ! {
	loop {
		let _ = button
			.wait_press_release()
			.await
			.inspect_err(|e| error!("{:?}", e));

		match button.is_high() {
			Err(e) => error!("{:?}", e),
			Ok(high) => if high {
				event_sender
					.send(ButtonEvent::Release(function).into())
					.await;
			} else {
				event_sender.send(ButtonEvent::Press(function).into()).await;
			}
		}
	}
}
