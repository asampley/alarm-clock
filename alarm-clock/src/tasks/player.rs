use core::pin::pin;

use crate::midi_dir::Midi;
use crate::{MIDI_NOTE_CAPACITY, Receiver, Sender, error, info, warn};
use embassy_time::{Duration, Instant};
use futures_lite::{FutureExt, Stream, StreamExt, stream};
use heapless::{BinaryHeap, Vec, binary_heap::Min};

use crate::message::{EventMessage, PlayerMessage, SongEvent, SynthMessage};
use crate::util::Either;

use embassy_time::Timer;

use midly::Timing;
use midly::TrackEventKind;
use midly::{Format, MetaMessage, TrackEvent};

const TRACK_CAPCITY: usize = 64;

enum State {
	Playing(Midi),
	Looping(Midi),
	Stopped,
}

impl State {
	fn next(self) -> Self {
		match self {
			Self::Playing(_) => Self::Stopped,
			_ => self,
		}
	}
}

#[embassy_executor::task]
pub async fn midi_player(
	player_receiver: Receiver<PlayerMessage, 1>,
	note_sender: Sender<SynthMessage, MIDI_NOTE_CAPACITY>,
	event_sender: Sender<EventMessage, 16>,
) {
	let mut state = State::Stopped;

	loop {
		state = match state {
			State::Stopped => match player_receiver.receive().await {
				PlayerMessage::Loop(midi) => State::Looping(midi),
				PlayerMessage::Play(midi) => State::Playing(midi),
				PlayerMessage::Stop => State::Stopped,
			},
			State::Playing(ref now_playing) | State::Looping(ref now_playing) => {
				// load file and set initial variables
				let (header, tracks) =
					midly::parse(now_playing.data).expect("Unable to parse midi file");

				let ticks_per_beat = if let Timing::Metrical(tpb) = header.timing {
					tpb.as_int()
				} else {
					panic!("Currently only supports metrical time")
				};

				let mut events = Vec::<_, TRACK_CAPCITY>::new();

				match header.format {
					Format::SingleTrack | Format::Parallel => {
						for track in tracks {
							match track {
								Err(e) => {
									error!(
										"Error while reading tracks: {:?}",
										defmt::Debug2Format(&e)
									)
								}
								Ok(event_iter) => {
									if events.push(event_iter).is_err() {
										warn!("Midi tracks buffer full, skipping");
										break;
									}
								}
							}
						}

						info!("Loaded midi file with {} tracks", events.len());

						let mut event_stream =
							pin!(stream_events_timed(&mut events, ticks_per_beat));

						event_sender
							.send(SongEvent::Start(now_playing.name).into())
							.await;

						let res = loop {
							let event = event_stream.next();

							match async { Either::First(player_receiver.receive().await) }
								.or(async { Either::Second(event.await) })
								.await
							{
								Either::First(msg) => match msg {
									PlayerMessage::Loop(midi) => break Some(State::Looping(midi)),
									PlayerMessage::Play(midi) => break Some(State::Playing(midi)),
									PlayerMessage::Stop => break Some(State::Stopped),
								},
								Either::Second(Some(msg)) => note_sender.send(msg).await,
								Either::Second(None) => break None,
							}
						};

						event_sender
							.send(SongEvent::End(now_playing.name).into())
							.await;

						res.unwrap_or_else(|| state.next())
					}
					Format::Sequential => {
						error!(
							"Honestly, you're better off converting this to Single Track or splitting it into multiple files"
						);

						State::Stopped
					}
				}
			}
		};

		note_sender.send(SynthMessage::Clear).await;
	}
}

#[derive(Debug)]
pub struct OrderedEvent<'a> {
	ticks: u32,
	track: usize,
	kind: TrackEventKind<'a>,
}

impl PartialEq for OrderedEvent<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.ticks.eq(&other.ticks)
	}
}

impl Eq for OrderedEvent<'_> {}

impl PartialOrd for OrderedEvent<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for OrderedEvent<'_> {
	fn cmp(&self, other: &Self) -> core::cmp::Ordering {
		self.ticks.cmp(&other.ticks)
	}
}

fn stream_events_timed<
	'a,
	'event: 'a,
	I: Iterator<Item = Result<TrackEvent<'event>, midly::Error>>,
	const N: usize,
>(
	events: &'a mut Vec<I, N>,
	ticks_per_beat: u16,
) -> impl Stream<Item = SynthMessage> + use<'a, I, N> {
	let tempo = 500_000; // microseconds per beat

	let mut next_events: BinaryHeap<_, Min, N> = Default::default();

	for (t_i, track_events) in events.iter_mut().enumerate() {
		match track_events.next() {
			Some(Ok(ev)) => next_events
				.push(OrderedEvent {
					ticks: ev.delta.into(),
					kind: ev.kind,
					track: t_i,
				})
				.unwrap(),
			Some(Err(e)) => warn_midly(&e),
			None => (),
		}
	}

	let last_instant = Instant::now();

	stream::unfold(
		(events, next_events, tempo, ticks_per_beat, last_instant),
		|mut state| async {
			loop {
				let (events, next_events, tempo, ticks_per_beat, last_instant) = &mut state;

				let next_event = next_events.peek()?;

				// see if the next event should happen
				let until_next = ticks_to_duration(*ticks_per_beat, *tempo, next_event.ticks);

				Timer::at(*last_instant + until_next).await;

				let event = next_events.pop().unwrap();

				// in case the next event involves timing, we must adjust all expectations
				// based on the current tempo before it changes.
				*last_instant += until_next;

				// this doesn't mess up the heap because everything is decremented by the same
				// amount
				next_events
					.iter_mut()
					.for_each(|nev| nev.ticks -= event.ticks);

				// now that everything has been adjusted to lose the ticks of the current
				// event, add the next event in the track
				match events[event.track].next() {
					Some(Ok(next)) => {
						next_events
							.push(OrderedEvent {
								ticks: next.delta.into(),
								track: event.track,
								kind: next.kind,
							})
							.unwrap();
					}
					Some(Err(e)) => warn_midly(&e),
					None => (),
				};

				match event.kind {
					TrackEventKind::Midi { channel, message } => {
						return Some((SynthMessage::Midi { channel, message }, state));
					}
					TrackEventKind::Meta(MetaMessage::Tempo(new)) => {
						*tempo = new.as_int();
					}
					_ => (),
				};
			}
		},
	)
}

/// Turn midi ticks into a time span. This should be done as late as possible to account for tempo
/// changes.
///
/// `tempo` is microseconds per beat
fn ticks_to_duration(ticks_per_beat: u16, tempo: u32, ticks: u32) -> Duration {
	Duration::from_micros(tempo as u64 * ticks as u64 / ticks_per_beat as u64)
}

fn warn_midly(e: &midly::Error) {
	warn!(
		"Error while reading midi file: {:?}",
		defmt::Debug2Format(e)
	)
}
