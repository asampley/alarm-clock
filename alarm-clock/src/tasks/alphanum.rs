use crate::error;
use embassy_time::{with_timeout, Duration, TimeoutError};

use heapless::String;

use crate::{Receiver, CONFIG};

use crate::circuit::hal::Alphanum;
use crate::message::AlphanumMessage;

const BLANKS: &str = "    ";

enum TextMode<I: Iterator<Item = char>> {
	Static,
	Iter(I),
}

#[embassy_executor::task]
pub async fn alphanum_task(mut alphanum: Alphanum, receiver: Receiver<AlphanumMessage, 1>) -> ! {
	let mut text; // must keep text in scope to create an iterator
	let mut text_mode = TextMode::Static;

	loop {
		let msg = match text_mode {
			TextMode::Static => Ok(receiver.receive().await),
			TextMode::Iter(_) => {
				with_timeout(
					Duration::from_millis(CONFIG.get().scroll_delay_ms),
					receiver.receive(),
				)
				.await
			}
		};

		match msg {
			Err(TimeoutError) => (),
			Ok(msg) => match msg {
				AlphanumMessage::Static(chars) => {
					let _ = alphanum
						.display(&chars)
						.await
						.inspect_err(|e| error!("{:?}", e));
					text_mode = TextMode::Static;
				}
				AlphanumMessage::Loop(t) => {
					text = t;
					text_mode = TextMode::Iter(BLANKS.chars().chain(text.chars()).cycle().skip(2));
				}
				AlphanumMessage::Empty => {
					let _ = alphanum
						.display(BLANKS)
						.await
						.inspect_err(|e| error!("{:?}", e));
					text_mode = TextMode::Static;
				}
				AlphanumMessage::Blink(blink_rate) => {
					let _ = alphanum
						.blink_rate(blink_rate)
						.await
						.inspect_err(|e| error!("{:?}", e));
				}
			},
		}

		match text_mode {
			TextMode::Static => (),
			TextMode::Iter(ref mut iter) => {
				let chars = iter.clone().take(4).collect::<String<4>>();
				let _ = alphanum
					.display(&chars)
					.await
					.inspect_err(|e| error!("{:?}", e));
				iter.next();
			}
		}
	}
}
