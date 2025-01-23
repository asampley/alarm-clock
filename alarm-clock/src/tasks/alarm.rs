use futures_lite::FutureExt;

use crate::{info, Sender};
use crate::message::EventMessage;
use crate::util::Either;
use crate::ALARM_TIME;

#[embassy_executor::task]
pub async fn alarm_task(event_channel: Sender<EventMessage, 1>) {
	let mut alarm_time = ALARM_TIME.receiver().unwrap();

	loop {
		match alarm_time.try_get() {
			None => {
				alarm_time.changed().await;
			}
			Some(time) => {
				info!("Next alarm at {}", time.as_chars());
				match async { Either::First(time.wait_until().await) }
					.or(async { Either::Second(alarm_time.changed().await) }).await
				{
					Either::First(()) => {
						info!("Alarm time!");
						event_channel.send(EventMessage::Alarm).await;
					}
					Either::Second(_) => (),
				}
			}
		}
	}
}
