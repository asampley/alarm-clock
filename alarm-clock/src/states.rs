use core::fmt;

use embassy_time::{Duration, Timer};

use embassy_sync::lazy_lock::LazyLock;

use enum_dispatch::enum_dispatch;
use heapless::{String, Vec};

use crate::circuit::alphanum::BlinkRate;
use crate::midi_dir::Midi;
use crate::time::{set_time, TimeRem};
use crate::timer::timer_remaining;
use crate::util::Calf;
use crate::{ClockTime, Sender, ALARM_SONG, ALARM_TIME, MIDI_DIR};

use crate::selector::{BinarySelector, ExponentialSelector, LinearSelector, Selector};

use crate::message::{
	AlphanumMessage, ButtonDirection, ButtonEvent, ButtonFunction, EventMessage, PlayerMessage,
	SongEvent, TimerEvent, TimerMessage,
};

static TIMES: LazyLock<Vec<ClockTime, { 24 * 60 }>> = LazyLock::new(|| {
	(0..24 * 60)
		.map(|i| ClockTime::new(i))
		.collect::<Vec<_, { 24 * 60 }>>()
});

async fn send_time<const SIZE: usize>(
	alphanum_sender: &Sender<AlphanumMessage, SIZE>,
	time: ClockTime,
) {
	alphanum_sender
		.send(AlphanumMessage::Static(Calf::Owned(time.as_chars())))
		.await
}

async fn format_timer(duration: Duration, hours: bool) -> String<4> {
	use core::fmt::Write;

	let mut string = String::new();

	let unit = duration.as_secs();

	let unit = if hours { unit / 60 } else { unit };

	let first = unit / 60;
	let second = unit % 60;

	if first > 0 {
		write!(&mut string, "{:>2}{:02}", first, second).unwrap();
	} else {
		write!(&mut string, "{:>4}", second).unwrap();
	}

	string
}

#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait State {
	async fn init(&mut self) {}

	/// should be a never returning future, but cancelled and run anew every event.
	async fn between_events(&mut self) -> ! {
		core::future::pending().await
	}

	async fn finish(&mut self) {}

	async fn event(&mut self, event: EventMessage) -> Option<StateTransition> {
		match event {
			EventMessage::Button(button) => self.button(button).await,
			EventMessage::Song(song) => self.song(song).await,
			EventMessage::Alarm => self.alarm().await,
			EventMessage::Timer(timer) => self.timer(timer).await,
		}
	}

	async fn timer(&mut self, event: TimerEvent) -> Option<StateTransition> {
		match event {
			TimerEvent::Start => None,
			TimerEvent::End => Some(StateTransition::Timer),
		}
	}

	async fn alarm(&mut self) -> Option<StateTransition> {
		Some(StateTransition::Alarm)
	}

	async fn button(&mut self, _event: ButtonEvent) -> Option<StateTransition> {
		None
	}

	async fn song(&mut self, _event: SongEvent) -> Option<StateTransition> {
		None
	}
}

#[enum_dispatch(State)]
pub enum ConcreteState {
	StateClock(StateClock),
	StateModeSelect(StateModeSelect),
	StateClockSet(StateClockSet),
	StateAlarm(StateAlarm),
	StateAlarmTime(StateAlarmTimeSet),
	StateAlarmSong(StateAlarmSongSet),
	StateTimer(StateTimer),
	StateTimerSet(StateTimerSet),
	StateTimerRunning(StateTimerRunning),
	StatePlay(StatePlay),
}

#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum StateTransition {
	Clock,
	ModeSelect,
	ClockSet,
	Alarm,
	AlarmTime,
	AlarmSong,
	Timer,
	TimerSet,
	TimerRunning,
	Play,
}

impl StateTransition {
	fn as_str(&self) -> &'static str {
		match self {
			Self::Clock => "Clock",
			Self::ModeSelect => "Mode Select",
			Self::ClockSet => "Clock Set",
			Self::Alarm => "Alarm",
			Self::AlarmTime => "Alarm Time",
			Self::AlarmSong => "Alarm Song",
			Self::Timer => "Timer",
			Self::TimerSet => "Timer Set",
			Self::TimerRunning => "Timer Running",
			Self::Play => "Play",
		}
	}
}

impl fmt::Display for StateTransition {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

pub struct StateClock {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
}

impl StateClock {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
		}
	}
}

impl State for StateClock {
	async fn between_events(&mut self) -> ! {
		loop {
			send_time(&mut self.alphanum_sender, ClockTime::now()).await;

			Timer::after(ClockTime::duration_to_next_minute()).await;
		}
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => {
				self.player_sender.send(PlayerMessage::Stop).await;

				match function {
					ButtonFunction::Select => Some(StateTransition::ModeSelect),
					ButtonFunction::Direction(_) => match timer_remaining() {
						Some(_) => Some(StateTransition::TimerRunning),
						None => None,
					},
				}
			}
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateModeSelect {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	mode_selector: LinearSelector<'static, StateTransition>,
}

impl StateModeSelect {
	pub fn new(alphanum_sender: Sender<AlphanumMessage, 1>) -> Self {
		Self {
			alphanum_sender,
			mode_selector: LinearSelector::new(&[
				StateTransition::Clock,
				StateTransition::ClockSet,
				StateTransition::TimerSet,
				StateTransition::AlarmTime,
				StateTransition::AlarmSong,
				StateTransition::Play,
			]),
		}
	}
}

impl State for StateModeSelect {
	async fn init(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Loop(self.mode_selector.curr().as_str()))
			.await
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => Some(*self.mode_selector.curr()),
				ButtonFunction::Direction(dir) => {
					let state = match dir {
						ButtonDirection::Prev => self.mode_selector.decr(),
						ButtonDirection::Next => self.mode_selector.incr(),
					};

					self.alphanum_sender
						.send(AlphanumMessage::Loop(state.as_str()))
						.await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateClockSet {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	time_selector: BinarySelector<'static, ClockTime>,
}

impl StateClockSet {
	pub fn new(alphanum_sender: Sender<AlphanumMessage, 1>) -> Self {
		Self {
			alphanum_sender,
			time_selector: BinarySelector::new(&TIMES.get()),
		}
	}
}

impl State for StateClockSet {
	async fn init(&mut self) {
		send_time(&mut self.alphanum_sender, *self.time_selector.curr()).await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					set_time(*self.time_selector.curr()).await;

					Some(StateTransition::Clock)
				}
				ButtonFunction::Direction(dir) => {
					let time = match dir {
						ButtonDirection::Prev => self.time_selector.decr(),
						ButtonDirection::Next => self.time_selector.incr(),
					};

					send_time(&mut self.alphanum_sender, *time).await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateAlarm {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
}

impl StateAlarm {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
		}
	}
}

impl State for StateAlarm {
	async fn init(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Blink(BlinkRate::OneHz))
			.await;

		self.player_sender
			.send(PlayerMessage::Loop(*ALARM_SONG.lock().await))
			.await;
	}

	async fn finish(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Blink(BlinkRate::Off))
			.await;

		self.player_sender.send(PlayerMessage::Stop).await;
	}

	async fn between_events(&mut self) -> ! {
		loop {
			send_time(&mut self.alphanum_sender, ClockTime::now()).await;

			Timer::after(ClockTime::duration_to_next_minute()).await;
		}
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(_) => Some(StateTransition::Clock),
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateAlarmTimeSet {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	time_selector: BinarySelector<'static, ClockTime>,
}

impl StateAlarmTimeSet {
	pub fn new(alphanum_sender: Sender<AlphanumMessage, 1>) -> Self {
		Self {
			alphanum_sender,
			time_selector: BinarySelector::new(&TIMES.get()),
		}
	}
}

impl State for StateAlarmTimeSet {
	async fn init(&mut self) {
		send_time(&mut self.alphanum_sender, *self.time_selector.curr()).await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					ALARM_TIME.sender().send(*self.time_selector.curr());

					Some(StateTransition::Clock)
				}
				ButtonFunction::Direction(dir) => {
					let time = match dir {
						ButtonDirection::Prev => self.time_selector.decr(),
						ButtonDirection::Next => self.time_selector.incr(),
					};

					send_time(&mut self.alphanum_sender, *time).await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateAlarmSongSet {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
	midi_selector: LinearSelector<'static, Midi>,
}

impl StateAlarmSongSet {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			midi_selector: LinearSelector::new(&MIDI_DIR),
		}
	}
}

impl State for StateAlarmSongSet {
	async fn init(&mut self) {
		let midi_file = self.midi_selector.curr();

		self.player_sender
			.send(PlayerMessage::Play(*midi_file))
			.await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					*ALARM_SONG.lock().await = *self.midi_selector.curr();

					self.player_sender.send(PlayerMessage::Stop).await;

					Some(StateTransition::Clock)
				}
				ButtonFunction::Direction(dir) => {
					let midi_file = match dir {
						ButtonDirection::Prev => self.midi_selector.decr(),
						ButtonDirection::Next => self.midi_selector.incr(),
					};

					self.player_sender
						.send(PlayerMessage::Play(*midi_file))
						.await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}

	async fn song(&mut self, event: SongEvent) -> Option<StateTransition> {
		match event {
			SongEvent::Start(name) => {
				self.alphanum_sender.send(AlphanumMessage::Loop(name)).await;
			}
			SongEvent::End(_) => {
				self.alphanum_sender
					.send(AlphanumMessage::Static(Calf::Borrowed("Play")))
					.await;
			}
		}

		None
	}
}

pub struct StatePlay {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
	midi_selector: LinearSelector<'static, Midi>,
}

impl StatePlay {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			midi_selector: LinearSelector::new(&MIDI_DIR),
		}
	}
}

impl State for StatePlay {
	async fn init(&mut self) {
		let midi_file = self.midi_selector.curr();

		self.player_sender
			.send(PlayerMessage::Play(*midi_file))
			.await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					self.player_sender.send(PlayerMessage::Stop).await;

					Some(StateTransition::Clock)
				}
				ButtonFunction::Direction(dir) => {
					let midi_file = match dir {
						ButtonDirection::Prev => self.midi_selector.decr(),
						ButtonDirection::Next => self.midi_selector.incr(),
					};

					self.player_sender
						.send(PlayerMessage::Play(*midi_file))
						.await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}

	async fn song(&mut self, event: SongEvent) -> Option<StateTransition> {
		match event {
			SongEvent::Start(name) => {
				self.alphanum_sender.send(AlphanumMessage::Loop(name)).await;
			}
			SongEvent::End(_) => {
				self.alphanum_sender
					.send(AlphanumMessage::Loop("Play"))
					.await;
			}
		}

		None
	}
}

pub struct StateTimerRunning {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	timer_sender: Sender<TimerMessage, 1>,
}

impl StateTimerRunning {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		timer_sender: Sender<TimerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			timer_sender,
		}
	}
}

impl State for StateTimerRunning {
	async fn between_events(&mut self) -> ! {
		loop {
			let remaining = timer_remaining().unwrap_or(Duration::from_ticks(0));

			let secs = remaining.as_secs();

			let hours = secs > 60 * 60;
			let timer = format_timer(remaining, hours).await;
			self.alphanum_sender
				.send(AlphanumMessage::Static(Calf::Owned(timer)))
				.await;

			if hours {
				Timer::after(remaining.rem_min()).await
			} else {
				Timer::after(remaining.rem_sec()).await
			}
		}
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					self.timer_sender.send(TimerMessage::Cancel).await;

					Some(StateTransition::Clock)
				}
				ButtonFunction::Direction(_) => Some(StateTransition::Clock),
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateTimer {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
}

impl StateTimer {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
		}
	}
}

impl State for StateTimer {
	async fn init(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Static(Calf::Borrowed("   0")))
			.await;
		self.alphanum_sender
			.send(AlphanumMessage::Blink(BlinkRate::OneHz))
			.await;

		self.player_sender
			.send(PlayerMessage::Loop(*ALARM_SONG.lock().await))
			.await;
	}

	async fn finish(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Blink(BlinkRate::Off))
			.await;

		self.player_sender.send(PlayerMessage::Stop).await;
	}

	async fn timer(&mut self, _event: TimerEvent) -> Option<StateTransition> {
		None
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select | ButtonFunction::Direction(_) => {
					Some(StateTransition::Clock)
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateTimerSet {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	timer_sender: Sender<TimerMessage, 1>,
	exponential_selector: ExponentialSelector,
}

impl StateTimerSet {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		timer_sender: Sender<TimerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			timer_sender,
			exponential_selector: Default::default(),
		}
	}
}

impl State for StateTimerSet {
	async fn init(&mut self) {
		let timer = format_timer(
			Duration::from_secs(*self.exponential_selector.curr() as u64) * 60,
			true,
		)
		.await;
		self.alphanum_sender
			.send(AlphanumMessage::Static(Calf::Owned(timer)))
			.await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					self.timer_sender
						.send(TimerMessage::Seconds(
							*self.exponential_selector.curr() as u64 * 60,
						))
						.await;

					Some(StateTransition::TimerRunning)
				}
				ButtonFunction::Direction(direction) => {
					let minutes = *match direction {
						ButtonDirection::Prev => self.exponential_selector.decr(),
						ButtonDirection::Next => self.exponential_selector.incr(),
					};

					let timer = format_timer(Duration::from_secs(minutes as u64) * 60, true).await;
					self.alphanum_sender
						.send(AlphanumMessage::Static(Calf::Owned(timer)))
						.await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}
