use embassy_time::{Duration, Instant, TICK_HZ, TimeoutError, WithTimeout};
use heapless::String;

use crate::Watch;

// not necessarily in past due to signed-ness of `embassy_time::Instant`
static TIME_ZERO: Watch<Instant, 64> = Watch::new_with(Instant::from_ticks(0));

pub async fn set_time(time: ClockTime) {
	TIME_ZERO
		.sender()
		.send(Instant::now().clock_sub(Duration::from_secs(time.minutes as u64 * 60)));
}

pub fn time_until(time: Instant) -> Option<Duration> {
	time.checked_duration_since(Instant::now())
}

pub fn time_since(time: Instant) -> Option<Duration> {
	Instant::now().checked_duration_since(time)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockTime {
	pub minutes: u16,
}

impl ClockTime {
	pub const fn new(minutes: u16) -> Self {
		Self {
			minutes: minutes % (24 * 60),
		}
	}

	pub fn now() -> Self {
		let delta = Instant::now().clock_sub(TIME_ZERO.try_get().unwrap());
		Self::new((delta.as_secs() / 60 % (24 * 60)) as u16)
	}

	pub async fn wait_until(self) {
		let mut time_zero = TIME_ZERO.receiver().unwrap();

		loop {
			let wait_time = Duration::from_secs(
				(self - ClockTime::now() - ClockTime::new(1)).minutes as u64 * 60,
			) + Self::duration_to_next_minute();

			if let Err(TimeoutError) = time_zero.changed().with_timeout(wait_time).await {
				break;
			}
		}
	}

	pub fn duration_to_next_minute() -> Duration {
		let delta = Instant::now().clock_sub(TIME_ZERO.try_get().unwrap());
		Duration::from_secs(60) - delta.rem_min()
	}

	pub fn hours(&self) -> u8 {
		(self.minutes / 60) as u8
	}

	pub fn minutes(&self) -> u8 {
		(self.minutes % 60) as u8
	}

	pub fn as_chars(&self) -> String<4> {
		let mut string = String::new();

		use core::fmt::Write;
		write!(&mut string, "{:02}{:02}", self.hours(), self.minutes()).unwrap();

		string
	}
}

impl core::ops::Sub<ClockTime> for ClockTime {
	type Output = ClockTime;

	fn sub(self, rhs: ClockTime) -> Self::Output {
		ClockTime::new((self.minutes.wrapping_sub(rhs.minutes) as i16).rem_euclid(24 * 60) as u16)
	}
}

pub trait TimeRem {
	// The remainder when divided into seconds
	fn rem_sec(self) -> Self;
	// The remainder when divided into minutes
	fn rem_min(self) -> Self;
}

impl TimeRem for Duration {
	fn rem_sec(self) -> Self {
		Duration::from_ticks(self.as_ticks().rem_euclid(TICK_HZ))
	}

	fn rem_min(self) -> Self {
		Duration::from_ticks(self.as_ticks().rem_euclid(TICK_HZ * 60))
	}
}

trait ClockSub<B> {
	type Output;

	fn clock_sub(self, b: B) -> Self::Output;
}

impl ClockSub<Instant> for Instant {
	type Output = Duration;

	fn clock_sub(self, b: Instant) -> Self::Output {
		Duration::from_ticks(clock_tick_sub(self.as_ticks(), b.as_ticks()))
	}
}

impl ClockSub<Duration> for Instant {
	type Output = Instant;

	fn clock_sub(self, b: Duration) -> Self::Output {
		Instant::from_ticks(clock_tick_sub(self.as_ticks(), b.as_ticks()))
	}
}

fn clock_tick_sub(ticks_a: u64, ticks_b: u64) -> u64 {
	(ticks_a.wrapping_sub(ticks_b) as i64).rem_euclid(TICK_HZ as i64 * 60 * 60 * 24) as u64
}
