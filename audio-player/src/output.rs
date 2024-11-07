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
pub(super) enum AudioOutputterError {
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

pub(super) struct AudioOutputter;

impl AudioOutputter {
    pub(super) fn new() -> Result<Box<dyn AudioOutputWriter>, AudioOutputterError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioOutputterError::OutputDeviceUnavailable)?;
        let config = device.default_output_config()?;
        info!("default: {:?}", config);
        let supported = device
            .supported_output_configs()?
            .next()
            .ok_or(AudioOutputterError::NoSupportedOutputConfigs)?
            .with_max_sample_rate();
        info!("supported: {:?}", supported);

        let writer = match config.sample_format() {
            cpal::SampleFormat::I8 => SymphoniaAudioOutputter::<i8>::new(&device, &config),
            cpal::SampleFormat::I16 => SymphoniaAudioOutputter::<i16>::new(&device, &config),
            cpal::SampleFormat::I32 => SymphoniaAudioOutputter::<i32>::new(&device, &config),
            // cpal::SampleFormat::I64 => SymphoniaAudioOutputter::<i64>::new(&device, &config),
            cpal::SampleFormat::U8 => SymphoniaAudioOutputter::<u8>::new(&device, &config),
            cpal::SampleFormat::U16 => SymphoniaAudioOutputter::<u16>::new(&device, &config),
            cpal::SampleFormat::U32 => SymphoniaAudioOutputter::<u32>::new(&device, &config),
            // cpal::SampleFormat::U64 => SymphoniaAudioOutputter::<u64>::new(&device, &config),
            cpal::SampleFormat::F32 => SymphoniaAudioOutputter::<f32>::new(&device, &config),
            cpal::SampleFormat::F64 => SymphoniaAudioOutputter::<f64>::new(&device, &config),
            sample_format => {
                return Err(AudioOutputterError::UnsupportedSampleFormat(sample_format))
            }
        }?;

        Ok(writer)
    }
}

pub(super) trait AudioOutputWriter {
    // TODO: create shared AudioBufferRef
    fn write(&mut self, data: AudioBufferRef);
    fn play(&mut self) -> Result<(), AudioOutputterError>;
    fn pause(&mut self) -> Result<(), AudioOutputterError>;
    fn sample_rate(&self) -> &u32;
}

struct SymphoniaAudioOutputter<T: Sample> {
    stream: Stream,
    tx: SyncSender<T>,
    sample_rate: u32,
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> SymphoniaAudioOutputter<T> {
    fn new(
        device: &Device,
        config: &SupportedStreamConfig,
    ) -> Result<Box<dyn AudioOutputWriter>, AudioOutputterError> {
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

        Ok(Box::new(SymphoniaAudioOutputter {
            stream,
            tx,
            sample_rate: config.sample_rate().0,
        }))
    }
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> AudioOutputWriter
    for SymphoniaAudioOutputter<T>
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

    fn play(&mut self) -> Result<(), AudioOutputterError> {
        self.stream.play()?;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AudioOutputterError> {
        self.stream.pause()?;
        Ok(())
    }

    fn sample_rate(&self) -> &u32 {
        &self.sample_rate
    }
}
