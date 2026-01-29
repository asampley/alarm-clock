use embassy_executor::SpawnToken;
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};

use crate::channel::event::SensorEvent;
use crate::channel::message::{EventMessage, SensorMessage};
use crate::channel::{EventSender, SensorSubscriber};
use crate::circuit::dht::Dht11;
use crate::{error, info};

pub type DhtTask<S, P>
	= fn(dht: Dht11<P>, sensor_subscriber: SensorSubscriber<'static>, event_sender: EventSender) -> SpawnToken<S>;

pub async fn dht_task<Pin: InputPin + OutputPin + ErrorType<Error: defmt::Format>>(
	mut humid_temp: Dht11<Pin>,
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
