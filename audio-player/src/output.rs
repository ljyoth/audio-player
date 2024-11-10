use std::sync::mpsc::{self, SyncSender, TryRecvError};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Sample, SizedSample, Stream, StreamError, SupportedStreamConfig,
};
use symphonia::core::{
    audio::{AudioBufferRef, SampleBuffer},
    conv::ConvertibleSample,
};
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub(super) enum AudioOutputError {
    #[error("OutputDeviceUnavailable")]
    OutputDeviceUnavailable,
    #[error("DefaultStreamConfigError {0}")]
    DefaultStreamConfig(#[from] cpal::DefaultStreamConfigError),
    #[error("SupportedStreamConfigsError {0}")]
    SupportedOutputConfigs(#[from] cpal::SupportedStreamConfigsError),
    #[error("SupportedOutputConfigs")]
    NoSupportedOutputConfigs,
    #[error("UnsupportedSampleFormat {0}")]
    UnsupportedSampleFormat(cpal::SampleFormat),
    #[error("BuildStreamError {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("PlayStreamError {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
    #[error("PauseStreamError {0}")]
    PauseStream(#[from] cpal::PauseStreamError),
}

pub(super) trait AudioOutputWrite {
    // TODO: create shared AudioBufferRef
    fn write(&mut self, data: AudioBufferRef);
    fn play(&mut self) -> Result<(), AudioOutputError>;
    fn pause(&mut self) -> Result<(), AudioOutputError>;
    fn sample_rate(&self) -> u32;
}

pub(super) enum AudioOutputWriter {
    Symphonia(SymphoniaAudioOutputWriter),
}

enum SymphoniaAudioOutputWriter {
    I8(SymphoniaAudioOutput<i8>),
    I16(SymphoniaAudioOutput<i16>),
    I32(SymphoniaAudioOutput<i32>),
    // I64(SymphoniaAudioOutput<i64>),
    U8(SymphoniaAudioOutput<u8>),
    U16(SymphoniaAudioOutput<u16>),
    U32(SymphoniaAudioOutput<u32>),
    // U64(SymphoniaAudioOutput<u64>),
    F32(SymphoniaAudioOutput<f32>),
    F64(SymphoniaAudioOutput<f64>),
}

impl AudioOutputWriter {
    pub(super) fn new() -> Result<AudioOutputWriter, AudioOutputError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputError::OutputDeviceUnavailable)?;
        let config = device.default_output_config()?;
        info!("default: {:?}", config);
        let supported = device
            .supported_output_configs()?
            .next()
            .ok_or(AudioOutputError::NoSupportedOutputConfigs)?
            .with_max_sample_rate();
        info!("supported: {:?}", supported);

        let writer = match config.sample_format() {
            cpal::SampleFormat::I8 => {
                SymphoniaAudioOutputWriter::I8(SymphoniaAudioOutput::<i8>::new(&device, &config)?)
            }
            cpal::SampleFormat::I16 => {
                SymphoniaAudioOutputWriter::I16(SymphoniaAudioOutput::<i16>::new(&device, &config)?)
            }
            cpal::SampleFormat::I32 => {
                SymphoniaAudioOutputWriter::I32(SymphoniaAudioOutput::<i32>::new(&device, &config)?)
            }
            // cpal::SampleFormat::I64 => SymphoniaAudioOutputWriter::I32(
            //     SymphoniaAudioOutputter::<i64>::new(&device, &config)?,
            // ),
            cpal::SampleFormat::U8 => {
                SymphoniaAudioOutputWriter::U8(SymphoniaAudioOutput::<u8>::new(&device, &config)?)
            }
            cpal::SampleFormat::U16 => {
                SymphoniaAudioOutputWriter::U16(SymphoniaAudioOutput::<u16>::new(&device, &config)?)
            }
            cpal::SampleFormat::U32 => {
                SymphoniaAudioOutputWriter::U32(SymphoniaAudioOutput::<u32>::new(&device, &config)?)
            }
            // cpal::SampleFormat::U64 => SymphoniaAudioOutputWriter::U64(
            //     SymphoniaAudioOutputter::<u64>::new(&device, &config)?,
            // ),
            cpal::SampleFormat::F32 => {
                SymphoniaAudioOutputWriter::F32(SymphoniaAudioOutput::<f32>::new(&device, &config)?)
            }
            cpal::SampleFormat::F64 => {
                SymphoniaAudioOutputWriter::F64(SymphoniaAudioOutput::<f64>::new(&device, &config)?)
            }
            sample_format => return Err(AudioOutputError::UnsupportedSampleFormat(sample_format)),
        };
        Ok(AudioOutputWriter::Symphonia(writer))
    }
}

macro_rules! match_symphonia_audio_output_writer {
    (|$writer:ident| $expression:expr) => {
        match $writer {
            SymphoniaAudioOutputWriter::I8($writer) => $expression,
            SymphoniaAudioOutputWriter::I16($writer) => $expression,
            SymphoniaAudioOutputWriter::I32($writer) => $expression,
            // SymphoniaAudioOutputWriter::I64($writer) => $expression,
            SymphoniaAudioOutputWriter::U8($writer) => $expression,
            SymphoniaAudioOutputWriter::U16($writer) => $expression,
            SymphoniaAudioOutputWriter::U32($writer) => $expression,
            // SymphoniaAudioOutputWriter::U64($writer) => $expression,
            SymphoniaAudioOutputWriter::F32($writer) => $expression,
            SymphoniaAudioOutputWriter::F64($writer) => $expression,
        }
    };
}

impl AudioOutputWrite for AudioOutputWriter {
    fn write(&mut self, data: AudioBufferRef) {
        match self {
            AudioOutputWriter::Symphonia(writer) => {
                match_symphonia_audio_output_writer!(|writer| writer.write(data))
            }
        }
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        match self {
            AudioOutputWriter::Symphonia(writer) => {
                match_symphonia_audio_output_writer!(|writer| writer.play())
            }
        }
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        match self {
            AudioOutputWriter::Symphonia(writer) => {
                match_symphonia_audio_output_writer!(|writer| writer.pause())
            }
        }
    }

    fn sample_rate(&self) -> u32 {
        match self {
            AudioOutputWriter::Symphonia(writer) => {
                match_symphonia_audio_output_writer!(|writer| writer.sample_rate())
            }
        }
    }
}

struct SymphoniaAudioOutput<T: Sample> {
    stream: Stream,
    tx: SyncSender<T>,
    sample_rate: u32,
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> SymphoniaAudioOutput<T> {
    fn new(
        device: &Device,
        config: &SupportedStreamConfig,
    ) -> Result<SymphoniaAudioOutput<T>, AudioOutputError> {
        fn handle_err(err: StreamError) {
            panic!("{}", err);
        }

        // May need to try rtrb/ringbuffer for performance
        let (tx, rx) = mpsc::sync_channel::<T>(config.sample_rate().0 as usize);
        let stream = device.build_output_stream(
            &config.config(),
            move |data, _| {
                data.iter_mut().for_each(|d| {
                    *d = match rx.try_recv() {
                        Ok(data) => data,
                        Err(TryRecvError::Empty) => T::MID,
                        Err(TryRecvError::Disconnected) => panic!("closed"),
                    }
                });
            },
            handle_err,
            None,
        )?;

        Ok(SymphoniaAudioOutput {
            stream,
            tx,
            sample_rate: config.sample_rate().0,
        })
    }
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> AudioOutputWrite
    for SymphoniaAudioOutput<T>
{
    fn write(&mut self, buffer: AudioBufferRef) {
        if buffer.frames() == 0 {
            return;
        }
        let spec = buffer.spec();

        let duration = buffer.capacity() as u64;
        let mut sample_buffer = SampleBuffer::<T>::new(duration.into(), *spec);
        sample_buffer.copy_interleaved_ref(buffer);
        sample_buffer.samples().iter().for_each(|&s| {
            self.tx.send(s).unwrap();
        })
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        self.stream.play()?;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        self.stream.pause()?;
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
