use core::panic;
use std::{borrow::Cow, collections::VecDeque, marker::PhantomData};

use cpal::Sample;
use rubato::{
    ResampleError, Resampler, ResamplerConstructionError, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::{AudioBuffer, AudioBufferRef, Channels, Signal, SignalSpec},
    codecs::{self, CodecParameters},
    conv::IntoSample,
    sample,
    units::Duration,
};
use tracing::{debug, info};

use crate::buffer::SampleBuffer;

#[derive(Debug, thiserror::Error)]
pub(super) enum ResamplerError {
    #[error("Invalid CodecParameters")]
    InvalidCodecParameters,
    #[error("Rubato ResamplerConstructionError: {0}")]
    RubatoResamplerConstruction(#[from] ResamplerConstructionError),
    #[error("Rubato ResampleError: {0}")]
    RubatoResample(#[from] ResampleError),
}

pub(super) struct SymphoniaResamplerBuffered<T: Sample> {
    resampler: SymphoniaResampler,
    // queue: VecDeque<f64>,
    queue: VecDeque<AudioBuffer<f64>>,
    buffer_position: usize,
    phantom: PhantomData<T>,
}

impl<T: Sample> SymphoniaResamplerBuffered<T> {
    const DEFAULT_CHUNK_SIZE: usize = 1024;

    pub(super) fn new(
        codec_params: &CodecParameters,
        output_sample_rate: u32,
    ) -> Result<Self, ResamplerError> {
        debug!("SymphoniaResamplerBuffered::new: {codec_params:?}");
        let input_sample_rate = codec_params
            .sample_rate
            .ok_or(ResamplerError::InvalidCodecParameters)?;
        let chunk_size = if let Some(chunk_size) = codec_params.max_frames_per_packet {
            chunk_size as usize
        } else if codec_params.codec == codecs::CODEC_TYPE_MP3 {
            debug!("mp3 codec: using samples_per_frame: 1152");
            1152
        } else {
            Self::DEFAULT_CHUNK_SIZE
        };
        let channels = codec_params
            .channels
            .ok_or(ResamplerError::InvalidCodecParameters)?;

        Ok(Self {
            resampler: SymphoniaResampler::new_inner(
                input_sample_rate,
                chunk_size,
                channels,
                output_sample_rate,
            )?,
            queue: VecDeque::new(),
            buffer_position: 0,
            phantom: PhantomData,
        })
    }

    pub(super) fn resample(
        &mut self,
        buffer: AudioBufferRef,
    ) -> Result<SymphoniaBufferedResamples<T>, ResamplerError> {
        // TODO: reduce allocations
        let mut b = AudioBuffer::new(buffer.capacity() as u64, *buffer.spec());
        buffer.convert(&mut b);
        self.queue.push_back(b);
        Ok(SymphoniaBufferedResamples { resampler: self })
        // match buffer {
        //     AudioBufferRef::U8(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::U16(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::U24(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::U32(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::S8(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::S16(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::S24(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::S32(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::F32(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        //     AudioBufferRef::F64(ref buffer) => fill_f64_buffer_3(
        //         buffer,
        //         self.buffer_position,
        //         &mut self.resampler.input_buffer,
        //     ),
        // };
    }

    fn resample_next(&mut self) -> Result<Option<&SampleBuffer<f64>>, ResamplerError> {
        assert!(self.resampler.input_buffer.len() > 0);
        println!(
            "len {} capacity {}",
            self.resampler.input_buffer[0].len(),
            self.resampler.input_buffer[0].capacity()
        );
        while self.resampler.input_buffer[0].len() < self.resampler.input_buffer[0].capacity() {
            println!("len: {:?}", self.queue.len());
            let buffer = match self.queue.front() {
                Some(buffer) => buffer,
                None => return Ok(None),
            };
            self.buffer_position = fill_f64_buffer_21(
                buffer,
                self.buffer_position,
                &mut self.resampler.input_buffer,
            );
            println!(
                "position {} frames {}",
                self.buffer_position,
                buffer.frames()
            );
            if self.buffer_position >= buffer.frames() {
                self.queue.pop_front();
                self.buffer_position = 0;
            }
        }
        let output_buffer = self.resampler.resample_inner()?;
        Ok(Some(output_buffer))
    }
}

pub(super) struct SymphoniaBufferedResamples<'r, T: Sample> {
    resampler: &'r mut SymphoniaResamplerBuffered<T>,
}

impl<'r, T: Sample> SymphoniaBufferedResamples<'r, T> {
    pub(super) fn next<'a>(&'a mut self) -> Option<Result<&SampleBuffer<f64>, ResamplerError>> {
        self.resampler.resample_next().transpose()
    }
}

pub(super) struct SymphoniaResampler {
    resampler: SincFixedIn<f64>,
    input_buffer: Vec<Vec<f64>>,
    output_buffer: SampleBuffer<f64>,
    output_buffer_frames: usize,
    output_audio_buffer: AudioBuffer<f64>,
    interleaved: Vec<f64>,
}

impl SymphoniaResampler {
    fn new_inner(
        input_sample_rate: u32,
        chunk_size: usize,
        channels: Channels,
        output_sample_rate: u32,
    ) -> Result<Self, ResamplerError> {
        debug!("SymphoniaResampler::new_inner: chunk_size {chunk_size}");
        let resampler = SincFixedIn::new(
            output_sample_rate as f64 / input_sample_rate as f64,
            2.0,
            SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            },
            chunk_size,
            channels.count(),
        )?;
        // let mut resampler = FftFixedIn::new(
        //     input_sample_rate as usize,
        //     output_sample_rate as usize,
        //     chunk_size,
        //     2,
        //     channels.count(),
        // )?;

        let input_buffer = (0..channels.count())
            .map(|_| Vec::with_capacity(chunk_size))
            .collect();
        // Need to pre-fill or resampler will fail
        let output_buffer = resampler.output_buffer_allocate(true);
        let output_buffer = SampleBuffer::with_buffer(output_buffer);
        let output_buffer_frames = output_buffer.frames();
        let interleaved = Vec::with_capacity(output_buffer.channels() * output_buffer.frames());

        let output_audio_buffer = AudioBuffer::new(
            output_buffer.frames() as Duration,
            SignalSpec {
                rate: output_sample_rate,
                channels,
            },
        );
        info!(
            "output_buffer_len: {} interleaved_length: {} output_buffer_capacity: {} output_buffer_frames: {}\n",
            output_buffer.frames(),
            interleaved.len(),
            output_audio_buffer.capacity(),
            output_audio_buffer.frames()
        );
        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            output_buffer_frames,
            output_audio_buffer,
            interleaved,
        })
    }

    pub(super) fn new(
        codec_params: &CodecParameters,
        output_sample_rate: u32,
    ) -> Result<Self, ResamplerError> {
        let input_sample_rate = codec_params
            .sample_rate
            .ok_or(ResamplerError::InvalidCodecParameters)?;
        let chunk_size = codec_params
            .max_frames_per_packet
            .ok_or(ResamplerError::InvalidCodecParameters)? as usize;
        let channels = codec_params
            .channels
            .ok_or(ResamplerError::InvalidCodecParameters)?;
        Self::new_inner(input_sample_rate, chunk_size, channels, output_sample_rate)
    }

    pub(super) fn new_with_buffer(
        buffer: &AudioBufferRef,
        output_sample_rate: u32,
    ) -> Result<Self, ResamplerError> {
        let spec = buffer.spec();
        Self::new_inner(
            spec.rate,
            buffer.frames(),
            spec.channels,
            output_sample_rate,
        )
    }

    fn resample_inner(&mut self) -> Result<&SampleBuffer<f64>, ResamplerError> {
        // need to resize output_buffer to match `self.resampler` expected size
        self.output_buffer
            .resize(self.output_buffer.channels(), self.output_buffer_frames);
        let (input_frames, output_frames) = self.resampler.process_into_buffer(
            &self.input_buffer,
            self.output_buffer.as_mut(),
            None,
        )?;

        // input_frames should always be everything
        assert_eq!(input_frames, self.input_buffer[0].len());

        // TODO: should move this outside this function, but lifetime issue so deal with later
        self.input_buffer
            .iter_mut()
            .for_each(|buffer| buffer.clear());

        self.output_buffer
            .resize(self.output_buffer.channels(), output_frames);

        debug!(
            "input: {} output: {} output_buffer_capacity: {} output_buffer_frames: {}",
            input_frames,
            output_frames,
            self.output_audio_buffer.capacity(),
            self.output_audio_buffer.frames()
        );
        Ok(&self.output_buffer)
    }

    pub(super) fn resample_buffer(
        &mut self,
        buffer: AudioBufferRef,
    ) -> Result<&SampleBuffer<f64>, ResamplerError> {
        match buffer {
            AudioBufferRef::U8(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::U16(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::U24(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::U32(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::S8(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::S16(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::S24(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::S32(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::F32(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
            AudioBufferRef::F64(ref buffer) => fill_f64_buffer(buffer, &mut self.input_buffer),
        };

        self.resampler.set_chunk_size(buffer.frames())?;
        self.resample_inner()
        // debug!(
        //     "input_buffer: {} input: {} output: {} output_buffer_capacity: {} output_buffer_frames: {}",
        //     buffer.frames(),
        //     input_frames,
        //     output_frames,
        //     self.output_audio_buffer.capacity(),
        //     self.output_audio_buffer.frames()
        // );
    }
}

macro_rules! match_audio_buffer_ref {
    (|$buffer:ident| $expression:expr) => {
        match $writer {
            AudioBufferRef::U8($buffer) => $expression,
            AudioBufferRef::U16($buffer) => $expression,
            AudioBufferRef::U24($buffer) => $expression,
            AudioBufferRef::U32($buffer) => $expression,
            AudioBufferRef::S8($buffer) => $expression,
            AudioBufferRef::S16($buffer) => $expression,
            AudioBufferRef::S24($buffer) => $expression,
            AudioBufferRef::S32($buffer) => $expression,
            AudioBufferRef::F32($buffer) => $expression,
            AudioBufferRef::F64($buffer) => $expression,
        }
    };
}

fn fill_f64_buffer<S: sample::Sample + IntoSample<f64>>(
    buffer: &Cow<'_, AudioBuffer<S>>,
    f64_buffer: &mut Vec<Vec<f64>>,
) {
    (0..f64_buffer.len()).for_each(|c| {
        f64_buffer[c].clear();
        buffer
            .chan(c)
            .iter()
            .for_each(|&s| f64_buffer[c].push(s.into_sample()));
    })
}

/// `end`: Exclusive
fn fill_f64_buffer_2<S: sample::Sample + IntoSample<f64>>(
    buffer: &Cow<'_, AudioBuffer<S>>,
    start: usize,
    f64_buffer: &mut Vec<Vec<f64>>,
) -> usize {
    let mut next = start;
    (0..f64_buffer.len()).for_each(|c| {
        let to_take = f64_buffer[c].capacity() - f64_buffer.len();
        buffer
            .chan(c)
            .iter()
            .skip(start)
            .take(to_take)
            .for_each(|&s| {
                f64_buffer[c].push(s.into_sample());
                next += 1;
            });
    });
    next
}

/// `end`: Exclusive
fn fill_f64_buffer_21<S: sample::Sample + IntoSample<f64>>(
    buffer: &AudioBuffer<S>,
    start: usize,
    f64_buffer: &mut Vec<Vec<f64>>,
) -> usize {
    let mut pushed = 0;
    (0..f64_buffer.len()).for_each(|c| {
        let to_take = f64_buffer[c].capacity() - f64_buffer[c].len();
        println!(
            "to_take {} cap {} len {} ",
            to_take,
            f64_buffer[c].capacity(),
            f64_buffer[c].len()
        );
        buffer
            .chan(c)
            .iter()
            .skip(start)
            .take(to_take)
            .for_each(|&s| {
                f64_buffer[c].push(s.into_sample());
                pushed += 1;
            });
        println!("pushed: {} cap: {}", pushed, f64_buffer[0].capacity());
        if pushed == 0 {
            panic!()
        }
    });
    pushed / f64_buffer.len() + start
}

fn fill_f64_buffer_3<S: sample::Sample + IntoSample<f64>>(
    buffer: &Cow<'_, AudioBuffer<S>>,
    f64_buffer: &mut Vec<VecDeque<f64>>,
) {
    (0..f64_buffer.len()).for_each(|c| {
        buffer.chan(c).iter().for_each(|&s| {
            f64_buffer[c].push_back(s.into_sample());
        });
    });
}
