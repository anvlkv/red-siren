use std::sync::Arc;
use std::time::Instant;

use cpal::OutputStreamTimestamp;
use thingbuf::mpsc::errors::TrySendError;
use thingbuf::ThingBuf;

use crate::{audio_runtime::stream::ProdData, dsp::DspNetworkBackend};

use super::quality::{Message, TelemetrySender};

pub struct PlaybackCallbackConfig {
    pub input_buffer: Option<Arc<ThingBuf<ProdData>>>,
    pub sample_rate: u32,
    pub telemetry: TelemetrySender,
    pub num_channels: usize,
}

pub fn playback_callback(
    mut net: Box<dyn DspNetworkBackend + Send + Sync>,
    PlaybackCallbackConfig {
        input_buffer,
        sample_rate,
        telemetry,
        num_channels,
    }: PlaybackCallbackConfig,
) -> Box<super::GenType> {
    let mut frame_buffer = vec![0_f32; num_channels];

    Box::new(
        move |ts: OutputStreamTimestamp, frames_per_channel: &mut [&mut [f32]]| {
            let cb_start = Instant::now();
            let buffer_size = frames_per_channel[0].len();

            let mut input_capture = None;

            #[allow(clippy::needless_range_loop)]
            for i in 0..buffer_size {
                let (input_frame, input_ts) = input_buffer
                    .as_ref()
                    .and_then(|buf| buf.pop())
                    .unwrap_or((0.0, None));

                net.process(&[input_frame], &mut frame_buffer);

                frames_per_channel
                    .iter_mut()
                    .enumerate()
                    .for_each(|(ch, channel)| {
                        channel[i] = frame_buffer[ch];
                    });
                input_capture = input_ts
                    .map(|input_ts| input_ts.capture)
                    .iter()
                    .chain(input_capture.iter())
                    .max()
                    .cloned();
            }

            let cb_duration = cb_start.elapsed();
            let estimated_duration = ts.playback.duration_since(ts.callback);

            let estimated_latency = if estimated_duration > cb_duration {
                Some(estimated_duration - cb_duration)
            } else {
                None
            };

            let input_latency = input_capture.map(|capture| ts.callback.duration_since(capture));

            let msg = Message {
                buffer_size,
                estimated_latency,
                sample_rate,
                input_latency,
            };

            if let Err(TrySendError::Full(_)) = telemetry.try_send(msg) {
                log::warn!("telemetry channel full, dropping message: {msg:?}");
            }
        },
    ) as Box<super::GenType>
}
