use std::{
    error::Error,
    iter,
    sync::mpsc::{self, SyncSender, TryRecvError},
};

use cpal::{
    traits::HostTrait, BuildStreamError, Device, OutputCallbackInfo, Sample, SizedSample, Stream,
    StreamConfig, StreamError, SupportedStreamConfig,
};
use rodio::DeviceTrait;
use symphonia::core::{
    audio::{AudioBufferRef, SampleBuffer},
    conv::{ConvertibleSample, IntoSample},
};

pub(super) struct AudioOutputter;

impl AudioOutputter {
    pub(super) fn new() -> Result<Box<dyn AudioOutputWriter>, Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no device")?;
        let config = device.default_output_config()?;
        println!("default: {:?}", config);
        let supported = device
            .supported_output_configs()?
            .next()
            .ok_or("no supported output configs")?
            .with_max_sample_rate();
        println!("supported: {:?}", supported);

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
            sample_format => return Err(format!("unsupported {sample_format}").into()),
        }?;

        Ok(writer)
    }
}

trait AudioOutputWriter {
    // TODO: create shared AudioBufferRef
    fn write(&mut self, data: AudioBufferRef);
}

struct SymphoniaAudioOutputter<T: Sample> {
    stream: Stream,
    tx: SyncSender<T>,
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> SymphoniaAudioOutputter<T> {
    fn new(
        device: &Device,
        config: &SupportedStreamConfig,
    ) -> Result<Box<dyn AudioOutputWriter>, BuildStreamError> {
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
        Ok(Box::new(SymphoniaAudioOutputter { stream, tx }))
    }
}

impl<T: SizedSample + ConvertibleSample + Send + 'static> AudioOutputWriter
    for SymphoniaAudioOutputter<T>
{
    fn write(&mut self, data: AudioBufferRef) {
        let duration = data.capacity() as u64;
        let spec = data.spec();
        let mut sample_buffer = SampleBuffer::<T>::new(duration.into(), *spec);
        sample_buffer.copy_interleaved_ref(data);
        sample_buffer.samples().iter().for_each(|&s| {
            self.tx.send(s);
        });
    }
}
