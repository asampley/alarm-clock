use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;

use crate::info;
use crate::message::PlayerMessage;
use crate::{ALARM_SONG, ALARM_TIME};

#[embassy_executor::task]
pub async fn alarm_task(player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>) {
	let mut alarm_time = ALARM_TIME.receiver().unwrap();

	loop {
		match alarm_time.try_get() {
			None => {
				alarm_time.changed().await;
			}
			Some(time) => {
				info!("Next alarm at {}", time.as_chars());
				match select(time.wait_until(), alarm_time.changed()).await {
					Either::First(_) => {
						info!("Alarm time!");

						player_sender
							.send(PlayerMessage::Loop(*ALARM_SONG.lock().await))
							.await;
					}
					Either::Second(_) => (),
				}
			}
		}
	}
}
