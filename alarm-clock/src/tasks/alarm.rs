use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use futures_lite::FutureExt;

use embassy_sync::watch;

use crate::message::EventMessage;
use crate::time::ClockTime;
use crate::util::Either;
use crate::{info, Sender, Watch};

static ALARM_TIME: Watch<ClockTime, 1> = Watch::new();

pub fn alarm_setter() -> watch::Sender<'static, CriticalSectionRawMutex, ClockTime, 1> {
	ALARM_TIME.sender()
}

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
					.or(async { Either::Second(alarm_time.changed().await) })
					.await
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
