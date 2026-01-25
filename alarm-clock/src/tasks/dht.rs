use crate::channel::event::SensorEvent;
use crate::channel::message::{EventMessage, SensorMessage};
use crate::channel::{EventSender, SensorSubscriber};
use crate::circuit::dht::Dht11;
use crate::{error, info};

#[embassy_executor::task]
pub async fn dht_task(
	mut humid_temp: Dht11,
	mut sensor_subscriber: SensorSubscriber<'static>,
	event_sender: EventSender,
) {
	loop {
		match sensor_subscriber.next_message_pure().await {
			SensorMessage::Update => {
				info!("Updating dht sensor");
				match humid_temp.read() {
					Ok(reading) => {
						event_sender
							.send(EventMessage::Sensor(SensorEvent::Dht(reading)))
							.await
					}
					Err(e) => {
						error!("Error reading from dht sensor: {:?}", e);
						event_sender
							.send(EventMessage::Sensor(SensorEvent::DhtError))
							.await;
					}
				}
			}
		}
	}
}
