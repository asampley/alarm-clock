pub mod event;
pub mod message;

type CSRM = embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

use message::*;

const ALPHANUM_CAP: usize = 1;
const EVENT_CAP: usize = 16;
const PLAYER_CAP: usize = 1;
const SENSOR_CAP: usize = 1;
const SENSOR_SUBS: usize = 2;
const SENSOR_PUBS: usize = 2;
const TIMER_CAP: usize = 1;

pub type Channel<T, const CAP: usize> = embassy_sync::channel::Channel<CSRM, T, CAP>;
pub type Sender<T, const CAP: usize> = embassy_sync::channel::Sender<'static, CSRM, T, CAP>;
pub type Receiver<T, const CAP: usize> = embassy_sync::channel::Receiver<'static, CSRM, T, CAP>;
pub type PubSubChannel<T, const CAP: usize, const SUBS: usize, const PUBS: usize> =
	embassy_sync::pubsub::PubSubChannel<CSRM, T, CAP, SUBS, PUBS>;
pub type Subscriber<'a, T, const CAP: usize, const SUBS: usize, const PUBS: usize> =
	embassy_sync::pubsub::Subscriber<'a, CSRM, T, CAP, SUBS, PUBS>;
pub type Publisher<'a, T, const CAP: usize, const SUBS: usize, const PUBS: usize> =
	embassy_sync::pubsub::Publisher<'a, CSRM, T, CAP, SUBS, PUBS>;
pub type Mutex<T> = embassy_sync::mutex::Mutex<CSRM, T>;
pub type Watch<T, const CAP: usize> = embassy_sync::watch::Watch<CSRM, T, CAP>;
pub type RwLock<T> = embassy_sync::rwlock::RwLock<CSRM, T>;
pub type RwLockReadGuard<'a, T> = embassy_sync::rwlock::RwLockReadGuard<'a, CSRM, T>;
pub type RwLockWriteGuard<'a, T> = embassy_sync::rwlock::RwLockReadGuard<'a, CSRM, T>;

pub type AlphanumChannel = Channel<AlphanumMessage, ALPHANUM_CAP>;
pub type AlphanumReceiver = Receiver<AlphanumMessage, ALPHANUM_CAP>;
pub type AlphanumSender = Sender<AlphanumMessage, ALPHANUM_CAP>;

pub type EventChannel = Channel<EventMessage, EVENT_CAP>;
pub type EventReceiver = Receiver<EventMessage, EVENT_CAP>;
pub type EventSender = Sender<EventMessage, EVENT_CAP>;

pub type MidiNoteChannel = Channel<SynthMessage, { crate::MIDI_NOTE_CAPACITY }>;
pub type MidiNoteReceiver = Receiver<SynthMessage, { crate::MIDI_NOTE_CAPACITY }>;
pub type MidiNoteSender = Sender<SynthMessage, { crate::MIDI_NOTE_CAPACITY }>;

pub type PlayerChannel = Channel<PlayerMessage, PLAYER_CAP>;
pub type PlayerReceiver = Receiver<PlayerMessage, PLAYER_CAP>;
pub type PlayerSender = Sender<PlayerMessage, PLAYER_CAP>;

pub type SensorPubSub = PubSubChannel<SensorMessage, SENSOR_CAP, SENSOR_SUBS, SENSOR_PUBS>;
pub type SensorSubscriber<'a> = Subscriber<'a, SensorMessage, SENSOR_CAP, SENSOR_SUBS, SENSOR_PUBS>;
pub type SensorPublisher<'a> = Publisher<'a, SensorMessage, SENSOR_CAP, SENSOR_SUBS, SENSOR_PUBS>;

pub type TimerChannel = Channel<TimerMessage, TIMER_CAP>;
pub type TimerReceiver = Receiver<TimerMessage, TIMER_CAP>;
pub type TimerSender = Sender<TimerMessage, TIMER_CAP>;
