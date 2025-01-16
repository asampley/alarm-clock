use crate::error;
use crate::message::ButtonFunction;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;

use crate::circuit::hal::Button;
use crate::message::{ButtonEvent, EventMessage};

#[embassy_executor::task(pool_size = 3)]
pub async fn poll_input(
	event_sender: Sender<'static, CriticalSectionRawMutex, EventMessage, 1>,
	mut button: Button,
	function: ButtonFunction,
) -> ! {
	loop {
		let _ = button
			.wait_press_release()
			.await
			.inspect_err(|e| error!("{:?}", e));

		if button.is_high() {
			event_sender
				.send(ButtonEvent::Release(function).into())
				.await;
		} else {
			event_sender.send(ButtonEvent::Press(function).into()).await;
		}
	}
}
