use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch;

use futures_lite::FutureExt;

use crate::channel::message::{EventMessage, SensorMessage};
use crate::channel::{EventSender, SensorPublisher, Watch};
use crate::info;
use crate::time::ClockTime;

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
pub async fn alarm_task(event_sender: EventSender, sensor_publisher: SensorPublisher<'static>) {
	let mut alarm_time = ALARM_TIME.receiver().unwrap();

	loop {
		match alarm_time.try_get() {
			None => {
				alarm_time.changed().await;
			}
			Some(time) => {
				info!("Next alarm at {}", time.as_chars());

				// Warm up sensor for actual alarm (this one in case we miss the window of 1 minute before
				sensor_publisher.publish(SensorMessage::Update).await;

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
						event_sender.send(EventMessage::Alarm).await;
					}
					// Warm up sensor for actual alarm
					Action::Sensor => sensor_publisher.publish(SensorMessage::Update).await,
					Action::Change => (),
				}
			}
		}
	}
}
