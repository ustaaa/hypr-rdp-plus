//! Remote microphone negotiation; enabled independently of desktop audio output.
use super::microphone::RemoteMicrophone;
use ironrdp_rdpsnd::pdu::{AudioFormat, WaveFormat};
use ironrdp_server::{
    RdpeaiServerBackend, RdpeaiServerFactory, RdpeaiServerMessage, ServerEvent, ServerEventSender,
};
use tokio::sync::mpsc;

#[derive(Default)]
pub struct MicrophoneFactory {
    sender: Option<mpsc::UnboundedSender<ServerEvent>>,
}
impl ServerEventSender for MicrophoneFactory {
    fn set_sender(&mut self, sender: mpsc::UnboundedSender<ServerEvent>) {
        self.sender = Some(sender);
    }
}
impl RdpeaiServerFactory for MicrophoneFactory {
    fn build_backend(&self) -> Box<dyn RdpeaiServerBackend> {
        Box::new(MicrophoneHandler {
            sender: self.sender.clone(),
            formats: formats(),
            selected: None,
            source: None,
            packets: 0,
        })
    }
}
fn formats() -> Vec<AudioFormat> {
    [48000, 44100]
        .into_iter()
        .flat_map(|rate| {
            [1, 2].into_iter().map(move |channels| AudioFormat {
                format: WaveFormat::PCM,
                n_channels: channels,
                n_samples_per_sec: rate,
                n_avg_bytes_per_sec: rate * u32::from(channels) * 2,
                n_block_align: channels * 2,
                bits_per_sample: 16,
                data: None,
            })
        })
        .collect()
}
fn supported(f: &AudioFormat) -> bool {
    formats()
        .iter()
        .any(|offered| offered.matches_for_negotiation(f))
}
struct MicrophoneHandler {
    sender: Option<mpsc::UnboundedSender<ServerEvent>>,
    formats: Vec<AudioFormat>,
    selected: Option<AudioFormat>,
    source: Option<RemoteMicrophone>,
    packets: u64,
}
impl RdpeaiServerBackend for MicrophoneHandler {
    fn supported_formats(&self) -> &[AudioFormat] {
        &self.formats
    }
    fn on_formats_negotiated(&mut self, negotiated: &[AudioFormat]) {
        self.source = None;
        self.selected = None;
        let Some((index, format)) = negotiated.iter().enumerate().find(|(_, f)| supported(f))
        else {
            tracing::warn!("Remote microphone: no common PCM format");
            return;
        };
        self.selected = Some(format.clone());
        let mut capture_format = format.clone();
        capture_format.n_block_align = format.n_channels * 2;
        capture_format.n_avg_bytes_per_sec =
            format.n_samples_per_sec * u32::from(capture_format.n_block_align);
        if let Some(sender) = &self.sender {
            let _ = sender.send(ServerEvent::Rdpeai(RdpeaiServerMessage::Open {
                frames_per_packet: format.n_samples_per_sec / 100,
                initial_format: index as u32,
                capture_format,
            }));
        }
    }
    fn on_open_reply(&mut self, result: i32) {
        if result < 0 {
            tracing::warn!(result, "Remote microphone: client refused capture");
            return;
        }
        if let Some(f) = &self.selected {
            match RemoteMicrophone::start(f.n_samples_per_sec, f.n_channels) {
                Ok(source) => {
                    self.source = Some(source);
                    tracing::info!(
                        rate = f.n_samples_per_sec,
                        channels = f.n_channels,
                        "Remote microphone ready"
                    );
                }
                Err(error) => tracing::error!(%error, "Remote microphone PipeWire startup failed"),
            }
        }
    }
    fn on_audio_data(&mut self, format: &AudioFormat, data: &[u8]) {
        if self.selected.as_ref() != Some(format)
            || data.len() % (usize::from(format.n_channels) * 2).max(1) != 0
        {
            return;
        }
        if let Some(source) = &self.source {
            source.push(data);
            self.packets += 1;
            if self.packets == 1 {
                tracing::info!(bytes = data.len(), "Remote microphone receiving PCM");
            }
        }
    }
    fn on_close(&mut self) {
        self.source = None;
        self.selected = None;
        tracing::info!(packets = self.packets, "Remote microphone closed");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_allows_client_omitted_derived_fields() {
        let mut format = formats()[0].clone();
        format.n_avg_bytes_per_sec = 0;
        format.n_block_align = 0;
        assert!(supported(&format));
    }
    #[test]
    fn formats_are_bounded_pcm() {
        assert_eq!(formats().len(), 4);
        let mut bad = formats()[0].clone();
        bad.n_channels = 8;
        assert!(!supported(&bad));
    }
    #[test]
    fn open_uses_client_format_index() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut h = MicrophoneHandler {
            sender: Some(tx),
            formats: formats(),
            selected: None,
            source: None,
            packets: 0,
        };
        let mut bad = formats()[0].clone();
        bad.n_channels = 8;
        h.on_formats_negotiated(&[bad, formats()[2].clone()]);
        match rx.try_recv().unwrap() {
            ServerEvent::Rdpeai(RdpeaiServerMessage::Open {
                initial_format,
                frames_per_packet,
                ..
            }) => {
                assert_eq!(initial_format, 1);
                assert_eq!(frames_per_packet, 441);
            }
            _ => panic!("expected microphone open"),
        }
    }
}
