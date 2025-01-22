use embassy_time::{Duration, Instant, Timer};

use futures_lite::{ FutureExt };

use crate::{info, Receiver, Sender};
use crate::message::{EventMessage, TimerEvent, TimerMessage};
use crate::timer::{get_timer, set_timer};
use crate::util::Either;

#[embassy_executor::task]
pub async fn timer_task(
	timer_receiver: Receiver<TimerMessage, 1>,
	event_sender: Sender<EventMessage, 1>,
) {
	loop {
		match get_timer() {
			None => process_message(timer_receiver.receive().await),
			Some(timer_time) => {
				info!("Started timer for {} seconds", (timer_time - Instant::now()).as_secs());

				event_sender.send(TimerEvent::Start.into()).await;

				match async { Either::First(timer_receiver.receive().await) }
					.or(async { Either::Second(Timer::at(timer_time).await) }).await
				{
					Either::First(message) => process_message(message),
					Either::Second(()) => {
						info!("Timer done!");

						set_timer(None);

						event_sender.send(TimerEvent::End.into()).await;
					}
				}
			}
		}
	}
}

fn process_message(message: TimerMessage) {
	match message {
		TimerMessage::Seconds(seconds) =>
			set_timer(Some(Instant::now() + Duration::from_secs(seconds))),
		TimerMessage::Cancel => set_timer(None),
	}
}
