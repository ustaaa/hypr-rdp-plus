//! Remote S16LE PCM exposed as a PipeWire input device, never as speaker playback.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const BUFFER_MS: usize = 100;

pub struct RemoteMicrophone {
    samples: Arc<Mutex<PcmBuffer>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl RemoteMicrophone {
    pub fn start(rate: u32, channels: u16) -> anyhow::Result<Self> {
        anyhow::ensure!(
            matches!(rate, 44_100 | 48_000),
            "Unsupported microphone rate: {rate}"
        );
        anyhow::ensure!(
            matches!(channels, 1 | 2),
            "Unsupported microphone channels: {channels}"
        );
        let frame = usize::from(channels) * 2;
        let samples = Arc::new(Mutex::new(PcmBuffer::new(
            rate as usize * BUFFER_MS / 1000,
            frame,
        )));
        let stop = Arc::new(AtomicBool::new(false));
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        let worker_samples = Arc::clone(&samples);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("rdp-microphone".into())
            .spawn(move || {
                let result = run_source(
                    rate,
                    channels,
                    worker_samples,
                    Arc::clone(&worker_stop),
                    &startup_tx,
                );
                if let Err(error) = result {
                    let _ = startup_tx.try_send(Err(format!("{error:#}")));
                    tracing::warn!(%error, "Remote microphone stopped");
                }
                worker_stop.store(true, Ordering::Release);
            })?;
        let mut microphone = Self {
            samples,
            stop,
            worker: Some(worker),
        };
        match startup_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(microphone),
            Ok(Err(error)) => {
                microphone.stop();
                anyhow::bail!("{error}")
            }
            Err(error) => {
                microphone.stop();
                anyhow::bail!("Microphone startup failed: {error}")
            }
        }
    }

    pub fn push(&self, pcm: &[u8]) {
        if !self.stop.load(Ordering::Acquire) {
            if let Ok(mut samples) = self.samples.lock() {
                samples.push(pcm);
            }
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
    }
}

impl Drop for RemoteMicrophone {
    fn drop(&mut self) {
        self.stop();
    }
}

struct PcmBuffer {
    bytes: VecDeque<u8>,
    capacity: usize,
    frame: usize,
    partial: Vec<u8>,
    last_push: Option<Instant>,
}

impl PcmBuffer {
    fn new(frames: usize, frame: usize) -> Self {
        let capacity = frames * frame;
        assert!(capacity > 0 && frame > 0);
        Self {
            bytes: VecDeque::with_capacity(capacity),
            capacity,
            frame,
            partial: Vec::with_capacity(frame),
            last_push: None,
        }
    }

    fn clear(&mut self) {
        self.bytes.clear();
        self.partial.clear();
        self.last_push = None;
    }

    fn push(&mut self, mut input: &[u8]) {
        if input.is_empty() {
            return;
        }
        if self
            .last_push
            .is_some_and(|time| time.elapsed() > Duration::from_millis(200))
        {
            self.clear();
        }
        self.last_push = Some(Instant::now());
        if !self.partial.is_empty() {
            let count = (self.frame - self.partial.len()).min(input.len());
            self.partial.extend_from_slice(&input[..count]);
            input = &input[count..];
            if self.partial.len() == self.frame {
                self.make_room(self.frame);
                self.bytes.extend(self.partial.drain(..));
            }
        }
        let aligned = input.len() / self.frame * self.frame;
        let complete = &input[..aligned];
        // Drop oldest complete frames on overflow: bounded latency, channel alignment intact.
        let keep = complete.len().min(self.capacity);
        self.make_room(keep);
        self.bytes.extend(&complete[complete.len() - keep..]);
        self.partial.extend_from_slice(&input[aligned..]);
    }

    fn make_room(&mut self, count: usize) {
        let excess = (self.bytes.len() + count).saturating_sub(self.capacity);
        self.bytes.drain(..excess);
    }

    fn read(&mut self, output: &mut [u8]) {
        // A newly opened recorder must not receive old speech from a long-idle source.
        if self
            .last_push
            .is_some_and(|time| time.elapsed() > Duration::from_millis(200))
        {
            self.clear();
        }
        output.fill(0);
        let count = output.len().min(self.bytes.len()) / self.frame * self.frame;
        for (dest, byte) in output[..count].iter_mut().zip(self.bytes.drain(..count)) {
            *dest = byte;
        }
    }
}

fn run_source(
    rate: u32,
    channels: u16,
    samples: Arc<Mutex<PcmBuffer>>,
    stop: Arc<AtomicBool>,
    startup: &mpsc::SyncSender<Result<(), String>>,
) -> anyhow::Result<()> {
    use pipewire::{self as pw, spa};
    pw::init();
    let mainloop = pw::main_loop::MainLoopBox::new(None)?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)?;
    let core = context.connect(None)?;
    let stream = pw::stream::StreamBox::new(
        &core,
        "Hypr-RDP Remote Microphone",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_ROLE => "Communication",
            *pw::keys::MEDIA_CLASS => "Audio/Source",
            *pw::keys::NODE_NAME => "hypr-rdp-microphone",
            *pw::keys::NODE_DESCRIPTION => "Hypr-RDP Remote Microphone",
            *pw::keys::APP_NAME => "hypr-rdp",
            "node.virtual" => "true",
        },
    )?;
    let state_stop = Arc::clone(&stop);
    let state_startup = startup.clone();
    let frame = usize::from(channels) * 2;
    let _listener = stream
        .add_local_listener_with_user_data(samples)
        .state_changed(move |_, _, _, state| match state {
            pw::stream::StreamState::Paused | pw::stream::StreamState::Streaming => {
                let _ = state_startup.try_send(Ok(()));
            }
            pw::stream::StreamState::Error(error) => {
                let _ = state_startup.try_send(Err(error.to_string()));
                state_stop.store(true, Ordering::Release);
            }
            _ => {}
        })
        .process(move |stream, samples| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let requested = buffer.requested();
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            let size = if let Some(output) = data.data() {
                let wanted = if requested == 0 {
                    output.len()
                } else {
                    usize::try_from(requested)
                        .unwrap_or(usize::MAX)
                        .saturating_mul(frame)
                        .min(output.len())
                };
                let wanted = wanted / frame * frame;
                // No RT_PROCESS flag: this callback runs on the worker loop, not a realtime thread.
                if let Ok(mut samples) = samples.lock() {
                    samples.read(&mut output[..wanted]);
                } else {
                    output[..wanted].fill(0);
                }
                wanted
            } else {
                0
            };
            let chunk = data.chunk_mut();
            *chunk.offset_mut() = 0;
            *chunk.stride_mut() = frame as i32;
            *chunk.size_mut() = size as u32;
        })
        .register()?;
    let mut format = spa::param::audio::AudioInfoRaw::new();
    format.set_format(spa::param::audio::AudioFormat::S16LE);
    format.set_rate(rate);
    format.set_channels(u32::from(channels));
    let bytes = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: format.into(),
        }),
    )
    .map_err(|error| anyhow::anyhow!("Microphone audio format: {error:?}"))?
    .0
    .into_inner();
    let pod = spa::pod::Pod::from_bytes(&bytes)
        .ok_or_else(|| anyhow::anyhow!("Invalid microphone format pod"))?;
    // No AUTOCONNECT: this source waits for an application to select it as input.
    stream.connect(
        spa::utils::Direction::Output,
        None,
        pw::stream::StreamFlags::MAP_BUFFERS,
        &mut [pod],
    )?;
    while !stop.load(Ordering::Acquire) {
        mainloop.loop_().iterate(Duration::from_millis(25));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_preserves_newest_complete_stereo_frames() {
        let mut buffer = PcmBuffer::new(2, 4);
        buffer.push(&[1, 2, 3, 4, 5, 6, 7, 8]);
        buffer.push(&[9, 10, 11, 12]);
        let mut output = [0; 12];
        buffer.read(&mut output);
        assert_eq!(output, [5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 0, 0]);
    }

    #[test]
    fn split_frame_packets_preserve_channels_and_silence_underrun() {
        let mut buffer = PcmBuffer::new(2, 4);
        buffer.push(&[1, 2, 3]);
        let mut output = [99; 4];
        buffer.read(&mut output);
        assert_eq!(output, [0; 4]);
        buffer.push(&[4, 5]);
        buffer.push(&[6, 7, 8]);
        let mut output = [0; 8];
        buffer.read(&mut output);
        assert_eq!(output, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn oversized_packet_does_not_grow_buffer() {
        let mut buffer = PcmBuffer::new(2, 2);
        buffer.push(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(buffer.bytes.len(), 4);
        let mut output = [0; 4];
        buffer.read(&mut output);
        assert_eq!(output, [5, 6, 7, 8]);
        buffer.push(&[10]);
        buffer.read(&mut output);
        assert_eq!(output, [9, 10, 0, 0]);
    }

    #[test]
    fn stale_speech_is_discarded() {
        let mut buffer = PcmBuffer::new(2, 2);
        buffer.push(&[1, 2]);
        buffer.last_push = Some(Instant::now() - Duration::from_secs(1));
        let mut output = [99; 2];
        buffer.read(&mut output);
        assert_eq!(output, [0; 2]);
    }
}
