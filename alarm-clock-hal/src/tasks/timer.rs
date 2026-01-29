use embassy_time::{Duration, Instant, Timer};

use futures_lite::FutureExt;

use heapless::Vec;

use crate::channel::event::TimerEvent;
use crate::channel::message::TimerMessage;
use crate::channel::{EventSender, Mutex, TimerReceiver};
use crate::util::Either;
use crate::{error, info};

pub static TIMERS: Mutex<Timers<64>> = Mutex::new(Timers { timers: Vec::new() });

pub struct Timers<const SIZE: usize> {
	timers: Vec<Instant, SIZE>,
}

impl<const SIZE: usize> Timers<SIZE> {
	fn insert_timer(&mut self, time: Instant) -> Result<usize, Instant> {
		match self.timers.binary_search(&time) {
			Ok(position) | Err(position) => self.timers.insert(position, time).map(|()| position),
		}
	}

	pub fn first_timer(&self) -> Option<Instant> {
		self.timers.first().copied()
	}

	pub fn last_timer(&self) -> Option<Instant> {
		self.timers.last().copied()
	}

	fn remove_timer(&mut self, time: Instant) -> Option<Instant> {
		match self.timers.binary_search(&time) {
			Ok(position) => Some(self.timers.remove(position)),
			Err(_) => None,
		}
	}

	pub fn prev_timer(&self, from: Instant) -> Option<Instant> {
		match self.timers.binary_search(&from) {
			Ok(position) | Err(position) => self.timers.get(position.checked_sub(1)?).copied(),
		}
	}

	pub fn next_timer(&self, from: Instant) -> Option<Instant> {
		match self.timers.binary_search(&from) {
			Ok(position) => self.timers.get(position + 1).copied(),
			Err(position) => self.timers.get(position).copied(),
		}
	}

	pub fn len(&self) -> usize {
		self.timers.len()
	}
}

#[embassy_executor::task]
pub async fn timer_task(
	timer_receiver: TimerReceiver,
	event_sender: EventSender,
) {
	let mut last_timer = Instant::from_ticks(0);

	loop {
		let next_timer = TIMERS.lock().await.next_timer(last_timer);

		match next_timer {
			None => process_message(timer_receiver.receive().await, &event_sender).await,
			Some(timer_time) => match timer_time.checked_duration_since(Instant::now()) {
				None => process_message(timer_receiver.receive().await, &event_sender).await,
				Some(_) => {
					match async { Either::First(timer_receiver.receive().await) }
						.or(async { Either::Second(Timer::at(timer_time).await) })
						.await
					{
						Either::First(message) => process_message(message, &event_sender).await,
						Either::Second(()) => {
							info!("Timer done!");

							last_timer = timer_time;

							event_sender.send(TimerEvent::End.into()).await;
						}
					}
				}
			},
		}
	}
}

async fn process_message(message: TimerMessage, event_sender: &EventSender) {
	match message {
		TimerMessage::Seconds(seconds) => {
			info!("Started timer for {} seconds", seconds);

			let time = Instant::now() + Duration::from_secs(seconds);
			match TIMERS.lock().await.insert_timer(time) {
				Err(_) => error!("Maximum timers reached!"),
				Ok(_) => {
					event_sender.send(TimerEvent::Start(time).into()).await;
				}
			}
		}
		TimerMessage::Remove(index) => {
			info!("Timer {} cancelled", index);
			TIMERS.lock().await.remove_timer(index);
		}
	}
}
