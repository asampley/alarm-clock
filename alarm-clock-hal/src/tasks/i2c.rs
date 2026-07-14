use embassy_executor::{SpawnError, SpawnToken};
use embassy_time::{Duration, Instant, Timer};

use embedded_hal::i2c::{ErrorType, I2c as SyncI2c};
use embedded_hal_async::i2c::I2c as AsyncI2c;

use futures_lite::FutureExt;

use heapless::{String, Vec};

use crate::channel::event::SensorEvent;
use crate::channel::message::{AlphanumMessage, EventMessage, SensorMessage};
use crate::channel::{AlphanumReceiver, EventSender, SensorSubscriber};
use crate::circuit::alphanum::{Alphanum, Char, char_to_alphanum};
use crate::circuit::bmp::Bmp180;
use crate::{CONFIG, error, info};

const BLANKS: &str = "    ";
static BLANKS_RENDERED: [Char; 4] = [char_to_alphanum(' '); 4];

enum TextMode<I: Iterator<Item = char>, J: Iterator<Item = Char>> {
	Static,
	Iter(I),
	IterRendered(J),
}

enum Event {
	AlphanumMessage(AlphanumMessage),
	AlphanumScroll,
	SensorMessage(SensorMessage),
}

pub type I2cTask<S, I> = fn(
	I,
	Alphanum,
	Bmp180,
	EventSender,
	AlphanumReceiver,
	SensorSubscriber<'static>,
) -> Result<SpawnToken<S>, SpawnError>;

pub async fn i2c_task<I: SyncI2c + AsyncI2c + ErrorType<Error: defmt_or_log::FormatOrDebug>>(
	mut i2c: I,
	alphanum: Alphanum,
	bmp: Bmp180,
	event_sender: EventSender,
	alphanum_receiver: AlphanumReceiver,
	mut sensor_subscriber: SensorSubscriber<'static>,
) -> ! {
	let mut text_str; // must keep text in scope to create an iterator
	let mut text_rendered; // must keep text in scope to create an iterator
	let mut text_mode = TextMode::Static;

	let mut scroll_time = None;

	loop {
		let sensor_receiver_future =
			async { Event::SensorMessage(sensor_subscriber.next_message_pure().await) };
		let alphanum_receiver_future =
			async { Event::AlphanumMessage(alphanum_receiver.receive().await) };
		let alphanum_scroll_future = async {
			match scroll_time {
				None => core::future::pending().await,
				Some(ref mut time) => {
					let delay = Duration::from_millis(CONFIG.scroll_delay_ms);
					Timer::at(*time).await;
					*time += delay;
					Event::AlphanumScroll
				}
			}
		};

		match alphanum_receiver_future
			.or(alphanum_scroll_future)
			.or(sensor_receiver_future)
			.await
		{
			Event::AlphanumMessage(msg) => {
				match msg {
					AlphanumMessage::Static(chars) => {
						let _ = alphanum
							.display_str(&mut i2c, &chars)
							.await
							.inspect_err(|e| error!("{:?}", e));
						text_mode = TextMode::Static;
					}
					AlphanumMessage::StaticRendered(chars) => {
						let _ = alphanum
							.display(&mut i2c, &chars)
							.await
							.inspect_err(|e| error!("{:?}", e));
						text_mode = TextMode::Static;
					}
					AlphanumMessage::Loop(t) => {
						text_str = t;
						text_mode =
							TextMode::Iter(BLANKS.chars().chain(text_str.chars()).cycle().skip(2));
					}
					AlphanumMessage::LoopRendered(t) => {
						text_rendered = t;
						text_mode = TextMode::IterRendered(
							BLANKS_RENDERED
								.iter()
								.chain(text_rendered.iter())
								.cycle()
								.skip(2)
								.cloned(),
						);
					}
					AlphanumMessage::Empty => {
						let _ = alphanum
							.display_str(&mut i2c, BLANKS)
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
				}

				scroll_time = match text_mode {
					TextMode::Static => None,
					TextMode::Iter(_) | TextMode::IterRendered(_) => Some(Instant::now()),
				};
			}
			Event::AlphanumScroll => match text_mode {
				TextMode::Static => (),
				TextMode::Iter(ref mut iter) => {
					let chars = iter.clone().take(4).collect::<String<4>>();
					let _ = alphanum
						.display_str(&mut i2c, &chars)
						.await
						.inspect_err(|e| error!("{:?}", e));
					iter.next();
				}
				TextMode::IterRendered(ref mut iter) => {
					let chars = iter.clone().take(4).collect::<Vec<Char, 4>>()[..]
						.try_into()
						.unwrap();
					let _ = alphanum
						.display(&mut i2c, &chars)
						.await
						.inspect_err(|e| error!("{:?}", e));
					iter.next();
				}
			},
			Event::SensorMessage(msg) => match msg {
				SensorMessage::Update => {
					info!("Updating bmp sensor");

					match bmp.read(&mut i2c, 0).await {
						Ok(reading) => {
							event_sender
								.send(EventMessage::Sensor(SensorEvent::Bmp(reading)))
								.await
						}
						Err(e) => {
							error!("Error reading from bmp sensor: {:?}", e);
							event_sender
								.send(EventMessage::Sensor(SensorEvent::BmpError))
								.await;
						}
					}
				}
			},
		}
	}
}
