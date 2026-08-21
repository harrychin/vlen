//! A tiny write-ahead log, the workload vlen is built for: every
//! record is length-prefix framed (`usize`, platform-independent),
//! timestamps are delta-encoded (small signed varints), replay is
//! decode-heavy, and a torn tail from a crash mid-write is detected
//! by the validating decoder instead of corrupting the replay.
//!
//! Run with: cargo run --release --example wal --features alloc

use std::time::Instant;

use vlen::{Error, Reader, Writer, bulk_decode_i64, bulk_encode_i64};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Event {
	device: u32,
	timestamp_ms: i64,
	reading: f32,
}

/// Appends one length-prefixed record. Timestamps are stored as the
/// delta from the previous record, so a steady sample rate encodes in
/// one or two bytes instead of nine.
fn append(log: &mut Vec<u8>, prev_ts: &mut i64, event: &Event) {
	let mut frame = [0u8; 22];
	let mut writer = Writer::new(&mut frame);
	writer.write(event.device).unwrap();
	writer.write(event.timestamp_ms - *prev_ts).unwrap();
	writer.write(event.reading).unwrap();
	let len = writer.finish();

	vlen::encode_append(log, len);
	log.extend_from_slice(&frame[..len]);
	*prev_ts = event.timestamp_ms;
}

/// Replays every complete record. A truncated or corrupt final
/// record — the signature of a crash mid-write — ends the replay
/// cleanly at the last intact frame instead of yielding garbage or
/// panicking.
fn replay(log: &[u8]) -> (Vec<Event>, usize) {
	let mut reader = Reader::new(log);
	let mut events = Vec::new();
	let mut prev_ts = 0i64;

	loop {
		let frame_start = reader.position();
		let frame_len = match reader.read::<usize>() {
			Ok(len) => len,
			// End of log, or a length prefix torn mid-byte.
			Err(_) => return (events, frame_start),
		};
		if reader.remaining() < frame_len {
			// The frame itself is torn.
			return (events, frame_start);
		}

		let payload_start = reader.position();
		let parsed: Result<Event, Error> = (|| {
			Ok(Event {
				device: reader.read::<u32>()?,
				timestamp_ms: prev_ts + reader.read::<i64>()?,
				reading: reader.read::<f32>()?,
			})
		})();
		match parsed {
			// The fields must consume exactly the framed length.
			Ok(event) if reader.position() - payload_start == frame_len => {
				prev_ts = event.timestamp_ms;
				events.push(event);
			},
			_ => return (events, frame_start),
		}
	}
}

fn main() {
	// A day of sensor data: steady 100 ms sampling with jitter, a few
	// devices, occasional gaps.
	let events: Vec<Event> = (0..100_000)
		.map(|i| {
			let jitter = (i * 7) % 5;
			let gap = if i % 1000 == 999 { 30_000 } else { 0 };
			Event {
				device: (i % 12) as u32,
				timestamp_ms: 1_700_000_000_000
					+ (i as i64) * 100
					+ jitter as i64 + gap,
				reading: (i % 100) as f32 / 4.0,
			}
		})
		.collect();

	// Append everything.
	let mut log = Vec::new();
	let mut prev_ts = 0i64;
	for event in &events {
		append(&mut log, &mut prev_ts, event);
	}

	let fixed_size = events.len() * (4 + 8 + 4 + 1);
	println!("records:        {}", events.len());
	println!(
		"log size:       {} bytes ({:.2} bytes/record)",
		log.len(),
		log.len() as f64 / events.len() as f64
	);
	println!(
		"fixed layout:   {} bytes — vlen is {:.1}x smaller",
		fixed_size,
		fixed_size as f64 / log.len() as f64
	);

	// Replay (the hot path after a restart).
	let start = Instant::now();
	let (replayed, valid) = replay(&log);
	let elapsed = start.elapsed();
	assert_eq!(replayed, events);
	assert_eq!(valid, log.len());
	println!(
		"replay:         {} records in {:.2?} ({:.1} ns/record)",
		replayed.len(),
		elapsed,
		elapsed.as_nanos() as f64 / replayed.len() as f64
	);

	// Crash simulation: tear the final record mid-frame.
	let torn = &log[..log.len() - 3];
	let (recovered, valid) = replay(torn);
	assert_eq!(recovered.len(), events.len() - 1);
	assert_eq!(&replayed[..recovered.len()], &recovered[..]);
	println!(
		"torn tail:      recovered {} of {} records, {} valid bytes",
		recovered.len(),
		events.len(),
		valid
	);

	// Columnar variant: the timestamp deltas alone, through the
	// run-detecting bulk codec (smooth deltas are its sweet spot).
	let mut prev = 0i64;
	let deltas: Vec<i64> = events
		.iter()
		.map(|e| {
			let d = e.timestamp_ms - prev;
			prev = e.timestamp_ms;
			d
		})
		.collect();
	let mut column = vec![0u8; deltas.len() * 9];
	let len = bulk_encode_i64(&mut column, &deltas).unwrap();
	let mut decoded = vec![0i64; deltas.len()];
	let start = Instant::now();
	bulk_decode_i64(&column[..len], &mut decoded).unwrap();
	let elapsed = start.elapsed();
	assert_eq!(decoded, deltas);
	println!(
		"delta column:   {} timestamps in {} bytes, bulk decode {:.2?} ({:.2} ns/value)",
		deltas.len(),
		len,
		elapsed,
		elapsed.as_nanos() as f64 / deltas.len() as f64
	);
}
