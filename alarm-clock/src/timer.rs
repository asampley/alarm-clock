use embassy_time::{Duration, Instant};

use crate::Watch;

static TIMER_TIME: Watch<Option<Instant>, 1> = Watch::new();

pub fn set_timer(time: Option<Instant>) {
	TIMER_TIME.sender().send(time);
}

pub fn get_timer() -> Option<Instant> {
	TIMER_TIME.try_get().flatten()
}

pub fn timer_remaining() -> Option<Duration> {
	get_timer()
		.map(|t| t.saturating_duration_since(Instant::now()))
		.filter(|t| *t > Duration::from_ticks(0))
}
