use crate::circuit::hal::Dht11;
use crate::message::{EventMessage, SensorEvent, SensorMessage};
use crate::Receiver;
use crate::{error, info, Sender};

#[embassy_executor::task]
pub async fn sensor_task(
	mut humid_temp: Dht11,
	sensor_receiver: Receiver<SensorMessage, 1>,
	event_sender: Sender<EventMessage, 1>,
) {
	loop {
		match sensor_receiver.receive().await {
			SensorMessage::Update => {
				info!("Updating sensors");
				match humid_temp.read().await {
					Ok(reading) => {
						event_sender
							.send(EventMessage::Sensor(SensorEvent::Dht(reading)))
							.await
					}
					Err(e) => error!("Error reading from sensor: {:?}", e),
				}
			}
		}
	}
}
