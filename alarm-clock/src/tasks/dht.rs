use embassy_sync::pubsub::DynSubscriber;

use crate::circuit::dht::Dht11;
use crate::message::{EventMessage, SensorEvent, SensorMessage};
use crate::{Sender, error, info};

#[embassy_executor::task]
pub async fn dht_task(
	mut humid_temp: Dht11,
	mut sensor_subscriber: DynSubscriber<'static, SensorMessage>,
	event_sender: Sender<EventMessage, 16>,
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
