use core::iter::{Peekable, Flatten};
use core::pin::{Pin, pin};
use core::task::{Poll, Context};
use core::future::Future;

use crate::{error, info, warn, MIDI_NOTE_CAPACITY};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant};
use futures::{Stream, StreamExt};
use heapless::Vec;
use sized_dst::Dst;

use crate::message::{BuzzerMessage, EventMessage, PlayerMessage, SongEvent};

use crate::note::MidiNote;

use embassy_sync::channel::{Receiver, Sender};
use embassy_time::Timer;

use midly::{MetaMessage, Format, TrackEvent, TrackIter};
use midly::MidiMessage;
use midly::Timing;
use midly::TrackEventKind;

const TRACK_CAPCITY: usize = 64;

#[embassy_executor::task]
pub async fn midi_player(
	player_receiver: Receiver<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	note_sender: Sender<'static, CriticalSectionRawMutex, BuzzerMessage, MIDI_NOTE_CAPACITY>,
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
			event_sender
				.send(SongEvent::Start(now_playing.name).into())
				.await;

			// load file and set initial variables
			let (header, tracks) =
				midly::parse(&now_playing.data).expect("Unable to parse midi file");

			let tempo = 500_000; // microseconds per beat
			let ticks_per_beat = if let Timing::Metrical(tpb) = header.timing {
				tpb.as_int()
			} else {
				panic!("Currently only supports metrical time")
			};

			type DynEventIter<'a> = dyn Iterator<Item = Result<TrackEvent<'a>, midly::Error>>;
			const DYN_EVENT_ITER_MAX_BYTES: usize = core::mem::size_of::<Flatten<TrackIter>>();

			let mut events = Vec::<Peekable<Dst<DynEventIter, DYN_EVENT_ITER_MAX_BYTES>>, TRACK_CAPCITY>::new();
			let mut next_times = Vec::<_, TRACK_CAPCITY>::new();
			let now = Instant::now();

			let received_message = pin!(async {
				match player_receiver.receive().await {
					PlayerMessage::Loop(midi) => {
						(Some(midi), true)
					}
					PlayerMessage::Play(midi) => {
						(Some(midi), false)
					}
					PlayerMessage::Stop => {
						(None, false)
					}
				}
			});

			let send_stream: SendTimedEventStream<_, TRACK_CAPCITY> = match header.format {
				Format::Parallel => {
					for track in tracks {
						match track {
							Err(e) => error!("Error while reading tracks: {:?}", defmt::Debug2Format(&e)),
							Ok(event_iter) => {
								let mut events_i = Dst::<DynEventIter, DYN_EVENT_ITER_MAX_BYTES>::new(event_iter).peekable();

								let next_time_us = match events_i.peek() {
									Some(Ok(ev)) => {
										delta_to_micros(ticks_per_beat, tempo, ev.delta.as_int())
									}
									_ => 0,
								};

								if let Err(_) = events.push(events_i) {
									warn!("Midi events buffer full, skipping");
									break;
								}

								// fine to unwrap because next_times is the same length as events
								next_times.push(now + Duration::from_micros(next_time_us)).unwrap()
							}
						}
					}

					info!("Loaded midi file with {} tracks", events.len());

					SendTimedEventStream::new(
						events,
						tempo,
						ticks_per_beat,
						note_sender
					)
				}
				Format::SingleTrack | Format::Sequential => {
					let events_0 = Dst::<DynEventIter, DYN_EVENT_ITER_MAX_BYTES>::new(
						tracks
							.filter_map(|f| {
								f.inspect_err(|e| {
									error!("Error while reading tracks: {:?}", defmt::Debug2Format(e))
								})
								.ok()
							})
							.flatten()
					).peekable();

					if let Err(_) = events.push(events_0) {
						warn!("Midi events buffer full, skipping");
						break;
					}

					SendTimedEventStream::new(
						events,
						tempo,
						ticks_per_beat,
						note_sender
					)
				}
			};

			let mut send_until = send_stream.take_until(received_message);

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

		note_sender.send(BuzzerMessage::Clear).await;
	}
}

pub struct SendTimedEventStream<'a, I, const N: usize>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{
	events: Vec<Peekable<I>, N>,
	tempo: u32,
	ticks_per_beat: u16,
	next_times: Vec<Instant, N>,
	note_sender: Sender<'a, CriticalSectionRawMutex, BuzzerMessage, MIDI_NOTE_CAPACITY>,
}

impl<'a, I, const N: usize> SendTimedEventStream<'a, I, N>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{
	fn new(
		mut events: Vec<Peekable<I>, N>,
		tempo: u32,
		ticks_per_beat: u16,
		note_sender: Sender<'a, CriticalSectionRawMutex, BuzzerMessage, MIDI_NOTE_CAPACITY>,
	) -> Self {
		let now = Instant::now();

		let next_times = events.iter_mut().map(|e_i| match e_i.peek() {
			Some(Ok(ev)) => {
				now + Duration::from_micros(delta_to_micros(ticks_per_beat, tempo, ev.delta.as_int()))
			}
			_ => now,
		})
		.collect();

		Self { events, tempo, ticks_per_beat, next_times, note_sender }
	}
}

impl<'a, I, const N: usize> Unpin for SendTimedEventStream<'a, I, N>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{}

impl<'a, I, const N: usize> Stream for SendTimedEventStream<'a, I, N>
where
	I: Iterator<Item = Result<TrackEvent<'a>, midly::Error>>,
{
	type Item = ();

	fn poll_next(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Self::Item>> {
		pin!(async {
			// break out of loop when there are no more notes
			if !self.as_mut().events.iter_mut().any(|ev| ev.peek().is_some()) {
				return None;
			}

			for ti in 0..self.events.len() {
				if self.events[ti].peek().is_none() {
					continue;
				}

				while self.next_times[ti] <= Instant::now() {
					let event = match self.events[ti].next() {
						Some(Ok(event)) => event,
						Some(Err(_)) => {
							warn!("Error while reading midi note from track {}", ti);
							break;
						}
						None => break,
					};

					match event.kind {
						TrackEventKind::Midi {
							channel: _,
							message,
						} => {
							if let Some(note) = midi_to_buzzer(message) {
								self.note_sender.send(BuzzerMessage::Note(note)).await;
							}
						}
						TrackEventKind::Meta(MetaMessage::Tempo(new_tempo)) => {
							self.tempo = new_tempo.as_int()
						}
						_ => (),
					};

					let ticks_per_beat = self.ticks_per_beat;
					let tempo = self.tempo;

					if let Some(Ok(next_event)) = self.events[ti].peek() {
						let increment = Duration::from_micros(delta_to_micros(
							ticks_per_beat,
							tempo,
							next_event.delta.as_int(),
						));

						self.next_times[ti] += increment;
					};
				}
			}

			Timer::at(*self.next_times.iter().min().unwrap()).await;

			Some(())
		}).poll(cx)
	}
}

fn delta_to_micros(ticks_per_beat: u16, tempo: u32, delta: u32) -> u64 {
	tempo as u64 * delta as u64 / ticks_per_beat as u64
}

fn midi_to_buzzer(msg: MidiMessage) -> Option<MidiNote> {
	match msg {
		MidiMessage::NoteOff { key, .. } => {
			Some(MidiNote { key, vel: 0.into() })
		}
		MidiMessage::NoteOn { key, vel } => {
			Some(MidiNote { key, vel })
		}
		MidiMessage::Aftertouch { key, vel } => {
			Some(MidiNote { key, vel })
		}
		_ => None,
	}
}
