use core::future::Future;
use core::pin::{pin, Pin};
use core::task::{Context, Poll};

use crate::{error, info, warn, MIDI_NOTE_CAPACITY};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant};
use futures::{Stream, StreamExt};
use heapless::{binary_heap::Min, BinaryHeap, Vec};

use crate::message::{EventMessage, PlayerMessage, SongEvent, SynthMessage};

use embassy_sync::channel::{Receiver, Sender};
use embassy_time::Timer;

use midly::Timing;
use midly::TrackEventKind;
use midly::{Format, MetaMessage, TrackEvent};

const TRACK_CAPCITY: usize = 64;

#[embassy_executor::task]
pub async fn midi_player(
	player_receiver: Receiver<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	note_sender: Sender<'static, CriticalSectionRawMutex, SynthMessage, MIDI_NOTE_CAPACITY>,
	event_sender: Sender<'static, CriticalSectionRawMutex, EventMessage, 1>,
) {
	let mut playing = None;
	let mut looping = false;

	loop {
		// stopped loop
		while playing.is_none() {
			match player_receiver.receive().await {
				PlayerMessage::Loop(midi) => {
					playing = Some(midi);
					looping = true;
				}
				PlayerMessage::Play(midi) => {
					playing = Some(midi);
					looping = false;
				}
				PlayerMessage::Stop => (),
			}
		}

		// playing
		if let Some(ref now_playing) = playing {
			// load file and set initial variables
			let (header, tracks) =
				midly::parse(&now_playing.data).expect("Unable to parse midi file");

			let tempo = 500_000; // microseconds per beat
			let ticks_per_beat = if let Timing::Metrical(tpb) = header.timing {
				tpb.as_int()
			} else {
				panic!("Currently only supports metrical time")
			};

			let mut events = Vec::<_, TRACK_CAPCITY>::new();
			let received_message = pin!(async {
				match player_receiver.receive().await {
					PlayerMessage::Loop(midi) => (Some(midi), true),
					PlayerMessage::Play(midi) => (Some(midi), false),
					PlayerMessage::Stop => (None, false),
				}
			});

			match header.format {
				Format::SingleTrack | Format::Parallel => {
					for track in tracks {
						match track {
							Err(e) => {
								error!("Error while reading tracks: {:?}", defmt::Debug2Format(&e))
							}
							Ok(event_iter) => {
								if let Err(_) = events.push(event_iter) {
									warn!("Midi tracks buffer full, skipping");
									break;
								}
							}
						}
					}

					info!("Loaded midi file with {} tracks", events.len());

					let send_stream =
						SendTimedEventStream::new(events, tempo, ticks_per_beat, note_sender);

					let mut send_until = send_stream.take_until(received_message);

					event_sender
						.send(SongEvent::Start(now_playing.name).into())
						.await;

					send_until.by_ref().collect::<()>().await;

					event_sender
						.send(SongEvent::End(now_playing.name).into())
						.await;

					if let Some(res) = send_until.take_result() {
						(playing, looping) = res;
					} else {
						if !looping {
							playing = None;
						}
					}
				}
				Format::Sequential => {
					error!("Honestly, you're better off converting this to Single Track or splitting it into multiple files");
				}
			};
		}

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
		Some(self.cmp(&other))
	}
}

impl Ord for OrderedEvent<'_> {
	fn cmp(&self, other: &Self) -> core::cmp::Ordering {
		self.ticks.cmp(&other.ticks)
	}
}

pub struct SendTimedEventStream<'a, I, const N: usize>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{
	events: Vec<I, N>,
	tempo: u32,
	ticks_per_beat: u16,
	last_instant: Instant,
	next_events: BinaryHeap<OrderedEvent<'a>, Min, N>,
	note_sender: Sender<'a, CriticalSectionRawMutex, SynthMessage, MIDI_NOTE_CAPACITY>,
}

impl<'a, I, const N: usize> SendTimedEventStream<'a, I, N>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{
	fn new(
		mut events: Vec<I, N>,
		tempo: u32,
		ticks_per_beat: u16,
		note_sender: Sender<'a, CriticalSectionRawMutex, SynthMessage, MIDI_NOTE_CAPACITY>,
	) -> Self {
		let last_instant = Instant::now();

		let mut next_events: BinaryHeap<_, _, N> = Default::default();

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

		Self {
			events,
			tempo,
			ticks_per_beat,
			last_instant,
			next_events,
			note_sender,
		}
	}
}

impl<'a, I, const N: usize> Unpin for SendTimedEventStream<'a, I, N> where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>
{
}

impl<'a, I, const N: usize> Stream for SendTimedEventStream<'a, I, N>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{
	type Item = ();

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		pin!(async {
			loop {
				let Some(next_event) = self.next_events.peek() else {
					break;
				};

				// see if the next event should happen
				let until_next =
					ticks_to_duration(self.ticks_per_beat, self.tempo, next_event.ticks);

				// next note is in the future
				if until_next > Instant::now() - self.last_instant {
					break;
				}

				let event = self.next_events.pop().unwrap();

				// in case the next event involves timing, we must adjust all expectations
				// based on the current tempo before it changes.
				self.last_instant += until_next;

				// this doesn't mess up the heap because everything is decremented by the same
				// amount
				self.next_events
					.iter_mut()
					.for_each(|nev| nev.ticks -= event.ticks);

				// now that everything has been adjusted to lose the ticks of the current
				// event, add the next event in the track
				match self.events[event.track].next() {
					Some(Ok(next)) => {
						self.next_events
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
						self.note_sender
							.send(SynthMessage::Midi { channel, message })
							.await;
					}
					TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
						self.tempo = tempo.as_int();
					}
					_ => (),
				};
			}

			match self.next_events.peek() {
				Some(next_event) => {
					Timer::after(ticks_to_duration(
						self.ticks_per_beat,
						self.tempo,
						next_event.ticks,
					))
					.await;

					Some(())
				}
				None => None,
			}
		})
		.poll(cx)
	}
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
