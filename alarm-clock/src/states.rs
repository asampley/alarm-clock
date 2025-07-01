use embassy_time::{Duration, Instant, Timer};

use embassy_sync::lazy_lock::LazyLock;

use enum_dispatch::enum_dispatch;
use futures_lite::FutureExt;
use heapless::{String, Vec};

use crate::circuit::alphanum::BlinkRate;
use crate::circuit::dht::Dht11Reading;
use crate::midi_dir::Midi;
use crate::tasks::alarm::alarm_setter;
use crate::tasks::timer::TIMERS;
use crate::time::{set_time, time_since, time_until, TimeRem};
use crate::util::Calf;
use crate::{ClockTime, Sender, ALARM_SONG, MIDI_DIR};

use crate::selector::{BinarySelector, ExponentialSelector, LinearSelector, Selector};

use crate::message::{
	AlphanumMessage, ButtonDirection, ButtonEvent, ButtonFunction, EventMessage, PlayerMessage,
	SensorEvent, SensorMessage, SongEvent, TimerEvent, TimerMessage,
};

static TIMES: LazyLock<Vec<ClockTime, { 24 * 60 }>> = LazyLock::new(|| {
	(0..24 * 60)
		.map(ClockTime::new)
		.collect::<Vec<_, { 24 * 60 }>>()
});

async fn send_time<const SIZE: usize>(
	alphanum_sender: Sender<AlphanumMessage, SIZE>,
	time: ClockTime,
) {
	alphanum_sender
		.send(AlphanumMessage::Static(Calf::Owned(time.as_chars())))
		.await
}

fn format_timer(duration: Duration, hours: bool) -> String<4> {
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

fn format_temperature(temperature: u8) -> String<4> {
	use core::fmt::Write;

	let mut string = String::new();

	write!(string, "{:>3}C", temperature).unwrap();

	string
}

fn format_humidity(humidity: u8) -> String<4> {
	use core::fmt::Write;

	let mut string = String::new();

	write!(string, "{:>3}%", humidity).unwrap();

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
			EventMessage::Sensor(sensor) => self.sensor(sensor).await,
		}
	}

	async fn timer(&mut self, event: TimerEvent) -> Option<StateTransition> {
		match event {
			TimerEvent::Start(time) => Some(StateTransition::TimerRunning(time)),
			TimerEvent::End => Some(StateTransition::Timer),
		}
	}

	async fn alarm(&mut self) -> Option<StateTransition> {
		Some(StateTransition::Alarm)
	}

	async fn button(&mut self, _event: ButtonEvent) -> Option<StateTransition> {
		None
	}

	async fn sensor(&mut self, _event: SensorEvent) -> Option<StateTransition> {
		None
	}

	async fn song(&mut self, _event: SongEvent) -> Option<StateTransition> {
		None
	}
}

#[enum_dispatch(State)]
pub enum ConcreteState {
	Clock(StateClock),
	MainMenu(StateMainMenu),
	ClockSet(StateClockSet),
	Alarm(StateAlarm),
	AlarmTime(StateAlarmTimeSet),
	AlarmSong(StateAlarmSongSet),
	Timer(StateTimer),
	TimerSet(StateTimerSet),
	TimerRunning(StateTimerRunning),
	TimerMenu(StateTimerMenu),
	Sensors(StateSensors),
	Play(StatePlay),
}

#[derive(Clone, Copy, Debug)]
pub enum StateTransition {
	Clock,
	MainMenu,
	ClockSet,
	Alarm,
	AlarmTime,
	AlarmSong,
	Timer,
	TimerSet,
	TimerRunning(Instant),
	TimerMenu(Instant),
	Sensors,
	Play,
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
			send_time(self.alphanum_sender, ClockTime::now()).await;

			Timer::after(ClockTime::duration_to_next_minute()).await;
		}
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => {
				self.player_sender.send(PlayerMessage::Stop).await;

				match function {
					ButtonFunction::Select => Some(StateTransition::MainMenu),
					ButtonFunction::Direction(dir) => {
						let lock = TIMERS.lock().await;

						let instant = match dir {
							ButtonDirection::Prev => lock.last_timer(),
							ButtonDirection::Next => lock.first_timer(),
						};

						instant.map(StateTransition::TimerRunning)
					}
				}
			}
			ButtonEvent::Release(_) => None,
		}
	}
}

#[derive(Debug)]
struct MenuItem<T: 'static> {
	name: &'static str,
	items: MenuContents<T>,
}

impl<T> MenuItem<T> {
	const fn leaf(name: &'static str, leaf: T) -> Self {
		Self {
			name,
			items: MenuContents::Leaf(leaf),
		}
	}

	const fn menu(name: &'static str, items: &'static [Self]) -> Self {
		Self {
			name,
			items: MenuContents::Menu(items),
		}
	}
}

#[derive(Debug)]
enum MenuContents<T: 'static> {
	Leaf(T),
	Menu(&'static [MenuItem<T>]),
}

pub struct StateMainMenu {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	mode_selector: LinearSelector<'static, MenuItem<StateTransition>>,
}

impl StateMainMenu {
	pub fn new(alphanum_sender: Sender<AlphanumMessage, 1>) -> Self {
		const MAIN_MENU: &[MenuItem<StateTransition>] = &[
			MenuItem::leaf("Back", StateTransition::Clock),
			MenuItem::leaf("Clock Set", StateTransition::ClockSet),
			MenuItem::leaf("Timer Set", StateTransition::TimerSet),
			MenuItem::menu(
				"Alarm",
				&[
					MenuItem::leaf("Time", StateTransition::AlarmTime),
					MenuItem::leaf("Song", StateTransition::AlarmSong),
					MenuItem::leaf("Exit", StateTransition::Clock),
				],
			),
			MenuItem::leaf("Sensors", StateTransition::Sensors),
			MenuItem::leaf("Play", StateTransition::Play),
		];

		Self {
			alphanum_sender,
			mode_selector: LinearSelector::new(MAIN_MENU),
		}
	}
}

impl State for StateMainMenu {
	async fn init(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Loop(self.mode_selector.curr().name))
			.await
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => match self.mode_selector.curr().items {
					MenuContents::Leaf(state) => Some(state),
					MenuContents::Menu(items) => {
						self.mode_selector = LinearSelector::new(items);

						self.alphanum_sender
							.send(AlphanumMessage::Loop(self.mode_selector.curr().name))
							.await;

						None
					}
				},
				ButtonFunction::Direction(dir) => {
					let menu_item = match dir {
						ButtonDirection::Prev => self.mode_selector.decr(),
						ButtonDirection::Next => self.mode_selector.incr(),
					};

					self.alphanum_sender
						.send(AlphanumMessage::Loop(menu_item.name))
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
			time_selector: BinarySelector::new(TIMES.get()),
		}
	}
}

impl State for StateClockSet {
	async fn init(&mut self) {
		send_time(self.alphanum_sender, *self.time_selector.curr()).await;
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

					send_time(self.alphanum_sender, *time).await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

enum AlarmDisplay {
	Time,
	Sensor(SensorDisplay),
}

pub struct StateAlarm {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
	sensor_sender: Sender<SensorMessage, 1>,
	dht: Option<Dht11Reading>,
	display: AlarmDisplay,
	display_rotate: Instant,
}

impl StateAlarm {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
		sensor_sender: Sender<SensorMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			sensor_sender,
			dht: None,
			display: AlarmDisplay::Time,
			display_rotate: Instant::now() + Duration::from_secs(3),
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

		self.sensor_sender.send(SensorMessage::Update).await;
	}

	async fn finish(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Blink(BlinkRate::Off))
			.await;

		self.player_sender.send(PlayerMessage::Stop).await;
	}

	async fn between_events(&mut self) -> ! {
		loop {
			if self.display_rotate < Instant::now() {
				self.display = match self.display {
					AlarmDisplay::Time => AlarmDisplay::Sensor(Default::default()),
					AlarmDisplay::Sensor(s) => s.next().map_or(AlarmDisplay::Time, AlarmDisplay::Sensor),
				};
				self.display_rotate += Duration::from_secs(3);
			}

			match self.display {
				AlarmDisplay::Time => send_time(self.alphanum_sender, ClockTime::now()).await,
				AlarmDisplay::Sensor(s) => {
					let message = match s {
						SensorDisplay::Temperature => self.dht.map(|d| format_temperature(d.temperature)),
						SensorDisplay::Humidity => self.dht.map(|d| format_humidity(d.humidity)),
					};

					self.alphanum_sender.send(AlphanumMessage::Static(Calf::Owned(message.unwrap_or(String::new())))).await;
				},
			}

			Timer::at(self.display_rotate)
				.or(Timer::after(ClockTime::duration_to_next_minute()))
				.await
		}
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(_) => Some(StateTransition::Clock),
			ButtonEvent::Release(_) => None,
		}
	}

	async fn sensor(&mut self, event: SensorEvent) -> Option<StateTransition> {
		match event {
			SensorEvent::Dht(reading) => self.dht = Some(reading),
			SensorEvent::DhtError => { self.sensor_sender.send(SensorMessage::Update).await; }
		}

		None
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
			time_selector: BinarySelector::new(TIMES.get()),
		}
	}
}

impl State for StateAlarmTimeSet {
	async fn init(&mut self) {
		send_time(self.alphanum_sender, *self.time_selector.curr()).await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => {
					alarm_setter().send(*self.time_selector.curr());

					Some(StateTransition::Clock)
				}
				ButtonFunction::Direction(dir) => {
					let time = match dir {
						ButtonDirection::Prev => self.time_selector.decr(),
						ButtonDirection::Next => self.time_selector.incr(),
					};

					send_time(self.alphanum_sender, *time).await;

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
	timer_instant: Instant,
	alphanum_sender: Sender<AlphanumMessage, 1>,
}

impl StateTimerRunning {
	pub fn new(timer_instant: Instant, alphanum_sender: Sender<AlphanumMessage, 1>) -> Self {
		Self {
			timer_instant,
			alphanum_sender,
		}
	}
}

impl State for StateTimerRunning {
	async fn between_events(&mut self) -> ! {
		loop {
			let remaining = time_until(self.timer_instant).unwrap_or(Duration::from_ticks(0));

			let secs = remaining.as_secs();

			let hours = secs > 60 * 60;
			let timer = format_timer(remaining, hours);
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
				ButtonFunction::Select => Some(StateTransition::TimerMenu(self.timer_instant)),
				ButtonFunction::Direction(dir) => match dir {
					ButtonDirection::Prev => {
						match TIMERS.lock().await.prev_timer(self.timer_instant) {
							Some(instant) => {
								self.timer_instant = instant;
								None
							}
							None => Some(StateTransition::Clock),
						}
					}
					ButtonDirection::Next => {
						match TIMERS.lock().await.next_timer(self.timer_instant) {
							Some(instant) => {
								self.timer_instant = instant;
								None
							}
							None => Some(StateTransition::Clock),
						}
					}
				},
			},
			ButtonEvent::Release(_) => None,
		}
	}
}

pub struct StateTimer {
	alphanum_sender: Sender<AlphanumMessage, 1>,
	player_sender: Sender<PlayerMessage, 1>,
	timer_sender: Sender<TimerMessage, 1>,
}

impl StateTimer {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		player_sender: Sender<PlayerMessage, 1>,
		timer_sender: Sender<TimerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			timer_sender,
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

	async fn timer(&mut self, _event: TimerEvent) -> Option<StateTransition> {
		None
	}

	async fn between_events(&mut self) -> ! {
		loop {
			let since = TIMERS
				.lock()
				.await
				.first_timer()
				.and_then(time_since)
				.unwrap_or(Duration::from_ticks(0));

			let secs = since.as_secs();

			let hours = secs > 60 * 60;
			let timer = format_timer(since, hours);
			self.alphanum_sender
				.send(AlphanumMessage::Static(Calf::Owned(timer)))
				.await;

			if hours {
				Timer::after(since.rem_min()).await
			} else {
				Timer::after(since.rem_sec()).await
			}
		}
	}

	async fn finish(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Blink(BlinkRate::Off))
			.await;

		self.player_sender.send(PlayerMessage::Stop).await;
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select | ButtonFunction::Direction(_) => {
					let lock = TIMERS.lock().await;

					let transition = match lock.len() {
						1 => Some(StateTransition::Clock),
						_ => {
							if let Some(t) = lock.first_timer().and_then(|t| lock.next_timer(t)) {
								if t > Instant::now() {
									Some(StateTransition::TimerRunning(t))
								} else {
									None
								}
							} else {
								Some(StateTransition::Clock)
							}
						}
					};

					if let Some(t) = lock.first_timer() {
						self.timer_sender.send(TimerMessage::Remove(t)).await;
					}

					transition
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
		);
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

					// let event transition handle switching to timer
					None
				}
				ButtonFunction::Direction(direction) => {
					let minutes = *match direction {
						ButtonDirection::Prev => self.exponential_selector.decr(),
						ButtonDirection::Next => self.exponential_selector.incr(),
					};

					let timer = format_timer(Duration::from_secs(minutes as u64) * 60, true);
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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
enum SensorDisplay {
	Temperature = 0,
	Humidity = 1,
}

impl Default for SensorDisplay {
	fn default() -> Self {
		Self::Temperature
	}
}

impl SensorDisplay {
	fn next(&self) -> Option<Self> {
		match self {
			Self::Temperature => Some(Self::Humidity),
			Self::Humidity => None,
		}
	}

	fn next_wrapping(&self) -> Self {
		match self {
			Self::Temperature => Self::Humidity,
			Self::Humidity => Self::Temperature,
		}
	}

	fn prev_wrapping(&self) -> Self {
		match self {
			Self::Temperature => Self::Humidity,
			Self::Humidity => Self::Temperature,
		}
	}
}

pub struct StateSensors {
	request_update: Instant,
	dht_last: Option<Dht11Reading>,
	display: SensorDisplay,
	alphanum_sender: Sender<AlphanumMessage, 1>,
	sensor_sender: Sender<SensorMessage, 1>,
}

impl StateSensors {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage, 1>,
		sensor_sender: Sender<SensorMessage, 1>,
	) -> Self {
		Self {
			request_update: Instant::now(),
			dht_last: None,
			display: SensorDisplay::Temperature,
			alphanum_sender,
			sensor_sender,
		}
	}
}

impl State for StateSensors {
	async fn between_events(&mut self) -> ! {
		loop {
			let now = Instant::now();

			if self.request_update < now {
				self.sensor_sender.send(SensorMessage::Update).await;

				self.request_update = now + Duration::from_secs(5);
			}

			let message = if let Some(reading) = self.dht_last {
				match self.display {
					SensorDisplay::Temperature => format_temperature(reading.temperature),
					SensorDisplay::Humidity => format_humidity(reading.humidity),
				}
			} else {
				String::new()
			};

			self.alphanum_sender
				.send(AlphanumMessage::Static(Calf::Owned(message)))
				.await;

			Timer::at(self.request_update).await;
		}
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => Some(StateTransition::Clock),
				ButtonFunction::Direction(direction) => {
					self.display = match direction {
						ButtonDirection::Prev => self.display.prev_wrapping(),
						ButtonDirection::Next => self.display.next_wrapping(),
					};

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}

	async fn sensor(&mut self, event: SensorEvent) -> Option<StateTransition> {
		match event {
			SensorEvent::Dht(reading) => self.dht_last = Some(reading),
			SensorEvent::DhtError => { self.sensor_sender.send(SensorMessage::Update).await; }
		}

		None
	}
}

enum TimerMenuItem {
	Delete,
	Back,
}

pub struct StateTimerMenu {
	timer_instant: Instant,
	alphanum_sender: Sender<AlphanumMessage, 1>,
	timer_sender: Sender<TimerMessage, 1>,
	menu_selector: LinearSelector<'static, MenuItem<TimerMenuItem>>,
}

impl StateTimerMenu {
	pub fn new(
		timer_instant: Instant,
		alphanum_sender: Sender<AlphanumMessage, 1>,
		timer_sender: Sender<TimerMessage, 1>,
	) -> Self {
		const TIMER_MENU: &[MenuItem<TimerMenuItem>] = &[
			MenuItem::leaf("Back", TimerMenuItem::Back),
			MenuItem::leaf("Delete", TimerMenuItem::Delete),
		];

		Self {
			timer_instant,
			alphanum_sender,
			timer_sender,
			menu_selector: LinearSelector::new(TIMER_MENU),
		}
	}
}

impl State for StateTimerMenu {
	async fn init(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Loop(self.menu_selector.curr().name))
			.await
	}

	async fn button(&mut self, event: ButtonEvent) -> Option<StateTransition> {
		match event {
			ButtonEvent::Press(function) => match function {
				ButtonFunction::Select => match &self.menu_selector.curr().items {
					MenuContents::Leaf(item) => match item {
						TimerMenuItem::Back => {
							Some(StateTransition::TimerRunning(self.timer_instant))
						}
						TimerMenuItem::Delete => {
							self.timer_sender
								.send(TimerMessage::Remove(self.timer_instant))
								.await;

							let lock = TIMERS.lock().await;

							Some(match lock.prev_timer(self.timer_instant) {
								Some(instant) => StateTransition::TimerRunning(instant),
								None => StateTransition::Clock,
							})
						}
					},
					MenuContents::Menu(items) => {
						self.menu_selector = LinearSelector::new(items);

						self.alphanum_sender
							.send(AlphanumMessage::Loop(self.menu_selector.curr().name))
							.await;

						None
					}
				},
				ButtonFunction::Direction(dir) => {
					let menu_item = match dir {
						ButtonDirection::Prev => self.menu_selector.decr(),
						ButtonDirection::Next => self.menu_selector.incr(),
					};

					self.alphanum_sender
						.send(AlphanumMessage::Loop(menu_item.name))
						.await;

					None
				}
			},
			ButtonEvent::Release(_) => None,
		}
	}
}
