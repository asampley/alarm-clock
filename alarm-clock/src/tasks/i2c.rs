use embassy_time::{Duration, Instant, Timer};

use embedded_hal::i2c::I2c as SyncI2c;
use embedded_hal_async::i2c::{ErrorType, I2c as AsyncI2c};

use futures_lite::FutureExt;

use heapless::String;

use crate::{error, Receiver, CONFIG};

use crate::circuit::alphanum::Alphanum;
use crate::message::AlphanumMessage;

/// Required to be concrete for embassy tasks
pub type I2c = impl SyncI2c + AsyncI2c + ErrorType<Error: defmt::Format>;
pub type Error = <I2c as ErrorType>::Error;

const BLANKS: &str = "    ";

enum TextMode<I: Iterator<Item = char>> {
	Static,
	Iter(I),
}

enum FutureOptions {
	AlphanumReceiver(AlphanumMessage),
	AlphanumScroll,
}

#[embassy_executor::task]
pub async fn alphanum_task(
	mut i2c: I2c,
	mut alphanum: Alphanum,
	receiver: Receiver<AlphanumMessage, 1>
) -> ! {
	let mut text; // must keep text in scope to create an iterator
	let mut text_mode = TextMode::Static;

	let mut scroll_time = None;

	loop {
		let alphanum_receiver_future = async {
			FutureOptions::AlphanumReceiver(receiver.receive().await)
		};
		let alphanum_scroll_future = async {
			match text_mode {
				TextMode::Static => core::future::pending().await,
				TextMode::Iter(_) => {
					let delay = Duration::from_millis(CONFIG.get().scroll_delay_ms);
					let time = scroll_time.get_or_insert(Instant::now() + delay);

					Timer::at(*time).await;

					*time += delay;

					FutureOptions::AlphanumScroll
				}
			}
		};

		match alphanum_receiver_future
			.or(alphanum_scroll_future)
			.await
		{
			FutureOptions::AlphanumReceiver(msg) => match msg {
				AlphanumMessage::Static(chars) => {
					let _ = alphanum
						.display(&mut i2c, &chars)
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
						.display(&mut i2c, BLANKS)
						.await
						.inspect_err(|e| error!("{:?}", e));
					text_mode = TextMode::Static;
				}
				AlphanumMessage::Blink(blink_rate) => {
					let _ = alphanum
						.blink_rate(&mut i2c, blink_rate)
						.await
						.inspect_err(|e| error!("{:?}", e));
				}
			},
			FutureOptions::AlphanumScroll => match text_mode {
				TextMode::Static => (),
				TextMode::Iter(ref mut iter) => {
					let chars = iter.clone().take(4).collect::<String<4>>();
					let _ = alphanum
						.display(&mut i2c, &chars)
						.await
						.inspect_err(|e| error!("{:?}", e));
					iter.next();
				}
			},
		}
	}
}
