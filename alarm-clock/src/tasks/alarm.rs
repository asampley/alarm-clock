use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::DynPublisher;
use embassy_sync::watch;

use futures_lite::FutureExt;

use crate::message::{EventMessage, SensorMessage};
use crate::time::ClockTime;
use crate::{Sender, Watch, info};

static ALARM_TIME: Watch<ClockTime, 1> = Watch::new();

pub fn alarm_setter() -> watch::Sender<'static, CriticalSectionRawMutex, ClockTime, 1> {
	ALARM_TIME.sender()
}

enum Action {
	Alarm,
	Sensor,
	Change,
}

#[embassy_executor::task]
pub async fn alarm_task(
	event_channel: Sender<EventMessage, 16>,
	sensor_channel: DynPublisher<'static, SensorMessage>,
) {
	let mut alarm_time = ALARM_TIME.receiver().unwrap();

	loop {
		match alarm_time.try_get() {
			None => {
				alarm_time.changed().await;
			}
			Some(time) => {
				info!("Next alarm at {}", time.as_chars());

				// Warm up sensor for actual alarm (this one in case we miss the window of 1 minute before
				sensor_channel.publish(SensorMessage::Update).await;

				match async {
					time.wait_until().await;
					Action::Alarm
				}
				.or(async {
					(ClockTime::new(time.minutes - 1)).wait_until().await;
					Action::Sensor
				})
				.or(async {
					alarm_time.changed().await;
					Action::Change
				})
				.await
				{
					Action::Alarm => {
						info!("Alarm time!");
						event_channel.send(EventMessage::Alarm).await;
					}
					// Warm up sensor for actual alarm
					Action::Sensor => sensor_channel.publish(SensorMessage::Update).await,
					Action::Change => (),
				}
			}
		}
	}
}
