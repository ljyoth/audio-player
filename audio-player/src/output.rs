use std::sync::mpsc::{self, SyncSender, TryRecvError};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SizedSample, Stream, StreamError, SupportedStreamConfig,
};
use symphonia::core::{
    audio::{AudioBufferRef, SampleBuffer as SymphoniaSampleBuffer},
    conv::ConvertibleSample,
};
use tracing::info;

use crate::buffer::{Sample, SampleBuffer, ToSample};

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

pub(super) trait AudioOutputWrite<T: Sample> {
    fn write<S: Sample + ToSample<T>>(&mut self, data: &SampleBuffer<f64>);
    fn write_symphonia(&mut self, data: AudioBufferRef);
    fn play(&mut self) -> Result<(), AudioOutputError>;
    fn pause(&mut self) -> Result<(), AudioOutputError>;
    fn sample_rate(&self) -> u32;
}

pub(super) enum AudioOutputWriter {
    Cpal(CpalAudioOutputWriter),
}

enum CpalAudioOutputWriter {
    I8(CpalAudioOutput<i8>),
    I16(CpalAudioOutput<i16>),
    I32(CpalAudioOutput<i32>),
    // I64(SymphoniaAudioOutput<i64>),
    U8(CpalAudioOutput<u8>),
    U16(CpalAudioOutput<u16>),
    U32(CpalAudioOutput<u32>),
    // U64(SymphoniaAudioOutput<u64>),
    F32(CpalAudioOutput<f32>),
    F64(CpalAudioOutput<f64>),
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
                CpalAudioOutputWriter::I8(CpalAudioOutput::<i8>::new(&device, &config)?)
            }
            cpal::SampleFormat::I16 => {
                CpalAudioOutputWriter::I16(CpalAudioOutput::<i16>::new(&device, &config)?)
            }
            cpal::SampleFormat::I32 => {
                CpalAudioOutputWriter::I32(CpalAudioOutput::<i32>::new(&device, &config)?)
            }
            // cpal::SampleFormat::I64 => SymphoniaAudioOutputWriter::I32(
            //     SymphoniaAudioOutputter::<i64>::new(&device, &config)?,
            // ),
            cpal::SampleFormat::U8 => {
                CpalAudioOutputWriter::U8(CpalAudioOutput::<u8>::new(&device, &config)?)
            }
            cpal::SampleFormat::U16 => {
                CpalAudioOutputWriter::U16(CpalAudioOutput::<u16>::new(&device, &config)?)
            }
            cpal::SampleFormat::U32 => {
                CpalAudioOutputWriter::U32(CpalAudioOutput::<u32>::new(&device, &config)?)
            }
            // cpal::SampleFormat::U64 => SymphoniaAudioOutputWriter::U64(
            //     SymphoniaAudioOutputter::<u64>::new(&device, &config)?,
            // ),
            cpal::SampleFormat::F32 => {
                CpalAudioOutputWriter::F32(CpalAudioOutput::<f32>::new(&device, &config)?)
            }
            cpal::SampleFormat::F64 => {
                CpalAudioOutputWriter::F64(CpalAudioOutput::<f64>::new(&device, &config)?)
            }
            sample_format => return Err(AudioOutputError::UnsupportedSampleFormat(sample_format)),
        };
        Ok(AudioOutputWriter::Cpal(writer))
    }
}

macro_rules! match_cpal_audio_output_writer {
    (|$writer:ident| $expression:expr) => {
        match $writer {
            CpalAudioOutputWriter::I8($writer) => $expression,
            CpalAudioOutputWriter::I16($writer) => $expression,
            CpalAudioOutputWriter::I32($writer) => $expression,
            // CpalAudioOutputWriter::I64($writer) => $expression,
            CpalAudioOutputWriter::U8($writer) => $expression,
            CpalAudioOutputWriter::U16($writer) => $expression,
            CpalAudioOutputWriter::U32($writer) => $expression,
            // CpalAudioOutputWriter::U64($writer) => $expression,
            CpalAudioOutputWriter::F32($writer) => $expression,
            CpalAudioOutputWriter::F64($writer) => $expression,
        }
    };
}

impl<T: Sample> AudioOutputWrite<T> for AudioOutputWriter {
    fn write<S: Sample + ToSample<T>>(&mut self, data: &SampleBuffer<f64>) {
        match self {
            AudioOutputWriter::Cpal(writer) => {
                match_cpal_audio_output_writer!(|writer| writer.write(data))
            }
        }
    }

    fn write_symphonia(&mut self, data: AudioBufferRef) {
        match self {
            AudioOutputWriter::Cpal(writer) => {
                match_cpal_audio_output_writer!(|writer| writer.write_symphonia(data))
            }
        }
    }

    fn play(&mut self) -> Result<(), AudioOutputError> {
        match self {
            AudioOutputWriter::Cpal(writer) => {
                match_cpal_audio_output_writer!(|writer| writer.play())
            }
        }
    }

    fn pause(&mut self) -> Result<(), AudioOutputError> {
        match self {
            AudioOutputWriter::Cpal(writer) => {
                match_cpal_audio_output_writer!(|writer| writer.pause())
            }
        }
    }

    fn sample_rate(&self) -> u32 {
        match self {
            AudioOutputWriter::Cpal(writer) => {
                match_cpal_audio_output_writer!(|writer| writer.sample_rate())
            }
        }
    }
}

struct CpalAudioOutput<T: Sample> {
    stream: Stream,
    tx: SyncSender<T>,
    sample_rate: u32,
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> CpalAudioOutput<T> {
    fn new(
        device: &Device,
        config: &SupportedStreamConfig,
    ) -> Result<CpalAudioOutput<T>, AudioOutputError> {
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
                        Err(TryRecvError::Empty) => T::EQUILIBRIUM,
                        Err(TryRecvError::Disconnected) => panic!("closed"),
                    }
                });
            },
            handle_err,
            None,
        )?;
        let sample_rate = config.sample_rate().0;

        Ok(CpalAudioOutput {
            stream,
            tx,
            sample_rate,
        })
    }
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> AudioOutputWrite<T>
    for CpalAudioOutput<T>
{
    fn write<S: Sample + ToSample<T>>(&mut self, samples: &SampleBuffer<f64>) {
        for sample in samples.interleaved() {
            self.tx.send(ToSample::to_sample(sample)).unwrap();
            // self.tx.send(sample.to_sample()).unwrap();
        }
    }

    fn write_symphonia(&mut self, buffer: AudioBufferRef) {
        if buffer.frames() == 0 {
            return;
        }
        let spec = buffer.spec();

        let duration = buffer.capacity() as u64;
        let mut sample_buffer = SymphoniaSampleBuffer::<T>::new(duration.into(), *spec);
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
