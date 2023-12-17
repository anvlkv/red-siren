extern crate coreaudio;

use std::sync::mpsc::{channel, TryRecvError};

use anyhow::Result;
use coreaudio::audio_unit::audio_format::LinearPcmFlags;
use coreaudio::audio_unit::render_callback::{self, data};
use coreaudio::audio_unit::{AudioUnit, Element, SampleFormat, Scope, StreamFormat};
use coreaudio::sys::{
    kAudioOutputUnitProperty_EnableIO, kAudioUnitProperty_StreamFormat, AudioStreamBasicDescription,
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use lazy_static::lazy_static;
use shared::play::{PlayOperation, PlayOperationOutput};

use crate::{Effect, ViewModel};

use super::{Core, CoreStreamer};

type S = f32;
const SAMPLE_FORMAT: SampleFormat = SampleFormat::F32;

lazy_static! {
    static ref CORE: Arc<Mutex<Core>> =
        Arc::new(Mutex::new(Core::new::<crate::RedSirenAUCapabilities>()));
    static ref OUT_STREAM: Arc<Mutex<Option<AudioUnit>>> = Arc::new(Mutex::new(None));
    static ref IN_STREAM: Arc<Mutex<Option<AudioUnit>>> = Arc::new(Mutex::new(None));
}

impl super::StreamerUnit for CoreStreamer {
    fn init(&self) -> Result<UnboundedReceiver<PlayOperationOutput>> {
        let mut input_audio_unit = AudioUnit::new(coreaudio::audio_unit::IOType::RemoteIO)?;
        let mut output_audio_unit = AudioUnit::new(coreaudio::audio_unit::IOType::RemoteIO)?;

        // Read device sample rate off the output stream
        let id = kAudioUnitProperty_StreamFormat;
        let asbd: AudioStreamBasicDescription =
            output_audio_unit.get_property(id, Scope::Output, Element::Output)?;
        let sample_rate = asbd.mSampleRate;

        // iOS doesn't let you reconfigure an "initialized" audio unit, so uninitialize them
        input_audio_unit.uninitialize()?;
        output_audio_unit.uninitialize()?;

        configure_for_recording(&mut input_audio_unit)?;

        let format_flag = match SAMPLE_FORMAT {
            SampleFormat::F32 => LinearPcmFlags::IS_FLOAT,
            SampleFormat::I32 | SampleFormat::I16 | SampleFormat::I8 => {
                LinearPcmFlags::IS_SIGNED_INTEGER
            }
            SampleFormat::I24 => {
                unimplemented!("Not implemented for I24")
            }
        };

        let in_stream_format = StreamFormat {
            sample_rate,
            sample_format: SAMPLE_FORMAT,
            flags: format_flag | LinearPcmFlags::IS_PACKED | LinearPcmFlags::IS_NON_INTERLEAVED,
            channels: 1,
        };

        let out_stream_format = StreamFormat {
            sample_rate,
            sample_format: SAMPLE_FORMAT,
            flags: format_flag | LinearPcmFlags::IS_PACKED | LinearPcmFlags::IS_NON_INTERLEAVED,
            channels: 2,
        };

        log::debug!("input={:#?}", &in_stream_format);
        log::debug!("output={:#?}", &out_stream_format);
        log::debug!("input_asbd={:#?}", &in_stream_format.to_asbd());
        log::debug!("output_asbd={:#?}", &out_stream_format.to_asbd());

        let id = kAudioUnitProperty_StreamFormat;
        input_audio_unit.set_property(
            id,
            Scope::Output,
            Element::Input,
            Some(&in_stream_format.to_asbd()),
        )?;
        output_audio_unit.set_property(
            id,
            Scope::Input,
            Element::Output,
            Some(&out_stream_format.to_asbd()),
        )?;

        let (render_sender, render_receiver) = channel::<ViewModel>();
        let (op_sender, op_receiver) = channel();
        let (resolve_sender, resolve_receiver) = futures::channel::mpsc::unbounded();

        type Args = render_callback::Args<data::NonInterleaved<S>>;

        let core = CORE.clone();
        let input_render_sender = render_sender.clone();
        log::debug!("set_input_callback");
        input_audio_unit.set_input_callback(move |args| {
            let Args { data, .. } = args;
            let core = core.lock().expect("input core lock");
            let input: Vec<Vec<f32>> =
                Vec::from_iter(data.channels().into_iter().map(|s| Vec::from(s)));

            let mut ops = vec![(PlayOperation::Input(input), resolve_sender.clone())];

            match op_receiver.try_recv() {
                Ok(op) => {
                    ops.push(op);
                    Ok(())
                }
                Err(TryRecvError::Empty) => Ok(()),
                Err(TryRecvError::Disconnected) => Err(()),
            }?;

            for (op, resolve) in ops {
                for effect in core.process_event(op) {
                    match effect {
                        Effect::Render(_) => {
                            let view = core.view();
                            input_render_sender.send(view).expect("send render");
                        }
                        Effect::Resolve(op) => {
                            resolve.unbounded_send(op.operation).expect("send resolve")
                        }
                    }
                }
            }

            Ok(())
        })?;
        input_audio_unit.initialize()?;

        log::debug!("set_render_callback");
        output_audio_unit.set_render_callback(move |args: Args| {
            let Args {
                num_frames,
                mut data,
                ..
            } = args;
            let buffer = &render_receiver.recv().expect("recv render data").0;

            for i in 0..num_frames {
                for (ch, channel) in data.channels_mut().enumerate() {
                    let sample: &S = buffer
                        .get(ch)
                        .or_else(|| buffer.first())
                        .and_then(|b| b.get(i))
                        .unwrap_or(&0_f32);
                    channel[i] = *sample;
                }
            }
            Ok(())
        })?;
        output_audio_unit.initialize()?;

        _ = IN_STREAM.lock().unwrap().insert(input_audio_unit);
        _ = OUT_STREAM.lock().unwrap().insert(output_audio_unit);
        _ = self
            .op_sender
            .lock()
            .expect("op sender lock")
            .insert(op_sender);
        _ = self
            .render_sender
            .lock()
            .expect("render sender lock")
            .insert(render_sender);

        Ok(resolve_receiver)
    }

    fn pause(&self) -> Result<()> {
        let mut input_audio_unit = IN_STREAM.lock().unwrap();
        let input_audio_unit = input_audio_unit.as_mut().unwrap();

        input_audio_unit.stop()?;

        let mut output_audio_unit = OUT_STREAM.lock().unwrap();
        let output_audio_unit = output_audio_unit.as_mut().unwrap();

        output_audio_unit.stop()?;

        log::info!("paused");

        Ok(())
    }

    fn start(&self) -> Result<()> {
        let mut input_audio_unit = IN_STREAM.lock().unwrap();
        let input_audio_unit = input_audio_unit.as_mut().unwrap();

        input_audio_unit.start()?;

        let mut output_audio_unit = OUT_STREAM.lock().unwrap();
        let output_audio_unit = output_audio_unit.as_mut().unwrap();

        output_audio_unit.start()?;

        log::info!("started");

        Ok(())
    }

    fn forward(&self, op: PlayOperation, resolve_id_sender: UnboundedSender<PlayOperationOutput>) {
        self.forward_op(CORE.clone(), op, resolve_id_sender);
    }
}

fn configure_for_recording(audio_unit: &mut AudioUnit) -> Result<(), coreaudio::Error> {
    println!("Configure audio unit for recording");

    // Enable mic recording
    let enable_input = 1u32;
    audio_unit.set_property(
        kAudioOutputUnitProperty_EnableIO,
        Scope::Input,
        Element::Input,
        Some(&enable_input),
    )?;

    // Disable output
    let disable_output = 0u32;
    audio_unit.set_property(
        kAudioOutputUnitProperty_EnableIO,
        Scope::Output,
        Element::Output,
        Some(&disable_output),
    )?;

    Ok(())
}
