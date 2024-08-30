use std::{
    error::Error,
    iter,
    sync::mpsc::{self, SyncSender, TryRecvError},
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BuildStreamError, Device, OutputCallbackInfo, Sample, SampleRate, SizedSample, Stream,
    StreamConfig, StreamError, SupportedStreamConfig,
};
use rtrb::Producer;
use symphonia::core::{
    audio::{AudioBuffer, AudioBufferRef, RawSample, SampleBuffer, Signal},
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

pub(super) trait AudioOutputWriter {
    // TODO: create shared AudioBufferRef
    fn write(&mut self, data: AudioBufferRef);
    fn write_f32(&mut self, data: &[f32]);
    fn sample_rate(&self) -> &u32;
}

struct SymphoniaAudioOutputter<T: Sample> {
    stream: Stream,
    tx: SyncSender<T>,
    producer: Producer<T>,
    sample_rate: u32,
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
        let (producer, mut consumer) = rtrb::RingBuffer::new(config.sample_rate().0 as usize);
        let stream = device.build_output_stream(
            &config.config(),
            move |data, _| {
                // let chunk = consumer.read_chunk(data.len()).unwrap();
                // let (slot1, slot2) = chunk.as_slices();
                // data.iter_mut()
                //     .zip(slot1.iter().chain(slot2.iter()))
                //     .for_each(|(d, &s)| {
                //         *d = s;
                //     });
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
        stream.play().unwrap();

        Ok(Box::new(SymphoniaAudioOutputter {
            stream,
            producer,
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
        if self.sample_rate != spec.rate {
            use rubato::*;
            let mut resampler = SincFixedIn::<f32>::new(
                self.sample_rate as f64 / spec.rate as f64,
                2.0,
                SincInterpolationParameters {
                    sinc_len: 256,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 256,
                    window: WindowFunction::BlackmanHarris2,
                },
                buffer.frames(),
                spec.channels.count(),
            )
            .unwrap();
            // let mut resampler = FftFixedIn::new(
            //     self.sample_rate as usize,
            //     spec.rate as usize,
            //     buffer.frames(),
            //     2,
            //     spec.channels.count(),
            // )
            // .unwrap();
            let input_chans: Vec<&[f32]> = match buffer {
                AudioBufferRef::U8(_) => todo!(),
                AudioBufferRef::U16(_) => todo!(),
                AudioBufferRef::U24(_) => todo!(),
                AudioBufferRef::U32(_) => todo!(),
                AudioBufferRef::S8(_) => todo!(),
                AudioBufferRef::S16(_) => todo!(),
                AudioBufferRef::S24(_) => todo!(),
                AudioBufferRef::S32(_) => todo!(),
                AudioBufferRef::F32(ref buffer) => {
                    (0..spec.channels.count()).map(|c| buffer.chan(c))
                }
                AudioBufferRef::F64(_) => todo!(),
            }
            .collect();
            let mut output_buffer = Resampler::output_buffer_allocate(&resampler, true);
            let (input_frames, output_frames) = Resampler::process_into_buffer(
                &mut resampler,
                &input_chans,
                &mut output_buffer,
                None,
            )
            .unwrap();
            println!(
                "{} {} {} {}",
                input_frames,
                output_frames,
                output_buffer[0].len(),
                output_buffer.len()
            );
            let mut interleaved = Vec::with_capacity(output_frames * spec.channels.count());
            for i in 0..output_frames {
                for ch in 0..spec.channels.count() {
                    interleaved.push(output_buffer[ch][i]);
                }
            }
            interleaved.iter().for_each(|&s| {
                self.tx.send(s.into_sample()).unwrap();
            })
        } else {
            let duration = buffer.capacity() as u64;
            let mut sample_buffer = SampleBuffer::<T>::new(duration.into(), *spec);
            sample_buffer.copy_interleaved_ref(buffer);
            sample_buffer.samples().iter().for_each(|&s| {
                self.tx.send(s).unwrap();
            })
        }
    }

    fn write_f32(&mut self, data: &[f32]) {
        // let mut chunks = self.producer.write_chunk(data.len()).unwrap();
        // let (slot1, slot2) = chunks.as_mut_slices();
        // slot1
        //     .iter_mut()
        //     .chain(slot2.iter_mut())
        //     .zip(data.iter())
        //     .for_each(|(w, &d)| {
        //         *w = d.into_sample();
        //     });
        // chunks.commit(data.len());
        data.iter().for_each(|&s| {
            self.tx.send(s.into_sample()).unwrap();
        })
    }

    fn sample_rate(&self) -> &u32 {
        &self.sample_rate
    }
}
