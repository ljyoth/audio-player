use std::{borrow::Cow, cmp::min};

use rubato::{
    ResampleError, Resampler, ResamplerConstructionError, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::{AudioBuffer, AudioBufferRef, Channels, Signal},
    codecs::{self, CodecParameters},
};
use tracing::debug;

use crate::buffer::{AsSlice, SampleBuf, SampleBuffer};

#[derive(Debug, thiserror::Error)]
pub(super) enum ResamplerError {
    #[error("Invalid CodecParameters")]
    InvalidCodecParameters,
    #[error("Rubato ResamplerConstructionError: {0}")]
    RubatoResamplerConstruction(#[from] ResamplerConstructionError),
    #[error("Rubato ResampleError: {0}")]
    RubatoResample(#[from] ResampleError),
}

macro_rules! match_symphonia_buffer {
    (|$buffer:ident| { 
       f64 => $f64:expr,
       _ => $default:expr
    }) => {
        match $buffer {
            AudioBufferRef::U8($buffer) => $default,
            AudioBufferRef::U16($buffer) => $default,
            AudioBufferRef::U24($buffer) => $default,
            AudioBufferRef::U32($buffer) => $default,
            AudioBufferRef::S8($buffer) => $default,
            AudioBufferRef::S16($buffer) => $default,
            AudioBufferRef::S24($buffer) => $default,
            AudioBufferRef::S32($buffer) => $default,
            AudioBufferRef::F32($buffer) => $default,
            AudioBufferRef::F64($buffer) => $f64,
        }
    };
}

pub(super) struct RubatoResamplerBuffered {
    resampler: RubatoResampler,
    buffer: ResamplerBuffer,
    symphonia_audio_buffer: Option<AudioBuffer<f64>>
}

impl RubatoResamplerBuffered {
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
            resampler: RubatoResampler::new_inner(
                input_sample_rate,
                chunk_size,
                channels,
                output_sample_rate,
            )?,
            buffer: ResamplerBuffer::new(channels.count(), chunk_size),
            symphonia_audio_buffer: None,
        })
    }

    pub(super) fn resample(
        &mut self,
        buffer: SampleBuffer,
    ) -> Result<BufferedResamples, ResamplerError> {
        self.buffer.clear();
        match buffer {
            SampleBuffer::Buf(buffer) => self.resample_buf(&buffer),
            SampleBuffer::BufRef(buffer) => self.resample_buf(buffer),
            SampleBuffer::Symphonia(buffer) => self.resample_symphonia(buffer),
        }
    }

    fn resample_buf<'r>(
        &'r mut self,
        buffer: &SampleBuf,
    ) -> Result<BufferedResamples<'r>, ResamplerError> {
        self.buffer.fill(buffer.as_ref());
        Ok(BufferedResamples {
            resampler: &mut self.resampler,
            buffer_iter: self.buffer.iter(),
        })
    }

    fn resample_symphonia(
        &mut self,
        buffer: AudioBufferRef,
    ) -> Result<BufferedResamples, ResamplerError> {
        let buffer_f64 = match self.symphonia_audio_buffer.as_mut() {
            Some(buffer) => buffer,
            None => {
                self.symphonia_audio_buffer = Some(buffer.make_equivalent());
                self.symphonia_audio_buffer.as_mut().unwrap()
            },
        };
        let buffer = match_symphonia_buffer!(|buffer| {
            f64 => buffer,
            _ => {
                buffer.convert(buffer_f64);
                Cow::Borrowed(self.symphonia_audio_buffer.as_ref().unwrap())
            }
        });
        self.buffer.fill(buffer.planes().planes());
        let iter = self.buffer.iter();
        Ok(BufferedResamples {
            resampler: &mut self.resampler,
            buffer_iter: iter,
        })
    }
}

pub(super) struct BufferedResamples<'r> {
    resampler: &'r mut RubatoResampler,
    buffer_iter: ResamplerBufferIter<'r>,
}

impl<'r> BufferedResamples<'r> {
    pub(super) fn next<'a>(&'a mut self) -> Option<Result<&SampleBuf, ResamplerError>> {
        match self.buffer_iter.next() {
            Some(buffer) => Some(self.resampler.resample_slice(buffer, buffer[0].len())),
            None => None,
        }
    }
}

pub(super) struct RubatoResampler {
    resampler: SincFixedIn<f64>,
    output_buffer: SampleBuf,
    output_buffer_frames: usize,
}

impl RubatoResampler {
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

        // Need to pre-fill or resampler will fail
        let output_buffer = resampler.output_buffer_allocate(true);
        let output_buffer = SampleBuf::with_buffer(output_buffer);
        let output_buffer_frames = output_buffer.frames();

        Ok(Self {
            resampler,
            output_buffer,
            output_buffer_frames,
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

    pub(super) fn resample(&mut self, buffer: &SampleBuffer) -> Result<&SampleBuf, ResamplerError> {
        match buffer {
            SampleBuffer::Buf(buffer) => self.resample_buf(&buffer),
            SampleBuffer::BufRef(buffer) => self.resample_buf(buffer),
            SampleBuffer::Symphonia(buffer) => self.resample_symphonia(buffer.clone()),
        }
    }

    fn resample_buf(&mut self, buffer: &SampleBuf) -> Result<&SampleBuf, ResamplerError> {
        self.resample_slice(buffer.as_ref(), buffer.frames())
    }

    fn resample_symphonia(&mut self, buffer: AudioBufferRef) -> Result<&SampleBuf, ResamplerError> {
        let buffer = match_symphonia_buffer!(|buffer| {
            f64 => buffer,
            _ => {
                let mut buffer_f64 = buffer.make_equivalent::<f64>();
                buffer.convert(&mut buffer_f64);
                Cow::Owned(buffer_f64)
            }
        });
        let planes = buffer.planes();
        self.resample_slice(planes.planes(), buffer.frames())
    }

    fn resample_slice<B: AsRef<[f64]>>(
        &mut self,
        buffer: &[B],
        frames: usize,
    ) -> Result<&SampleBuf, ResamplerError> {
        self.resampler.set_chunk_size(frames)?;
        // need to resize output_buffer to match `self.resampler` expected size
        self.output_buffer
            .resize(self.output_buffer.channels(), self.output_buffer_frames);
        let (input_frames, output_frames) =
            self.resampler
                .process_into_buffer(buffer, self.output_buffer.as_mut(), None)?;

        // input_frames should always be everything
        // assert_eq!(input_frames, buffer[0].len());

        self.output_buffer
            .resize(self.output_buffer.channels(), output_frames);

        debug!("input: {} output: {}", input_frames, output_frames,);
        Ok(&self.output_buffer)
    }
}

struct ResamplerBuffer {
    buffers: Vec<Vec<Vec<f64>>>,
    current_buffer: usize,
    channels: usize,
    frames: usize,
}

impl ResamplerBuffer {
    fn new(channels: usize, frames: usize) -> Self {
        Self {
            buffers: vec![],
            current_buffer: 0,
            channels,
            frames,
        }
    }

    fn add_buffer(&mut self) {
        self.buffers
            .push((0..self.channels).map(|c| Vec::with_capacity(c)).collect());
    }

    fn fill<B: AsSlice<f64>>(&mut self, input: &[B]) {
        assert_eq!(self.channels, input.len());
        debug!(
            "fill: available: {} buffers: {}",
            self.available_all(),
            self.buffers.len()
        );
        while self.available_all() < input[0].as_slice().len() {
            self.add_buffer();
        }
        let mut current_buffer = self.current_buffer;
        for channel in 0..self.channels {
            current_buffer = self.current_buffer;
            let mut input_position = 0;
            while input_position < input[channel].as_slice().len() {
                let next_position = min(
                    input[channel].as_slice().len(),
                    input_position + self.available(current_buffer, channel),
                );
                debug!(
                    "position: {} available: {} next_position: {} current_buffer: {} current_len: {}",
                    input_position,
                    self.available(current_buffer, channel),
                    next_position,
                    current_buffer,
                    self.buffers[current_buffer][channel].len()
                );
                self.buffers[current_buffer][channel]
                    .extend_from_slice(&input[channel].as_slice()[input_position..next_position]);

                if self.available(current_buffer, channel) == 0 {
                    current_buffer = self.next_buffer(current_buffer);
                }
                input_position = next_position
            }
        }
        self.current_buffer = current_buffer;
    }

    fn clear(&mut self) {
        let to_clear = !self.buffers.is_empty() && self.available(self.current_buffer, 0) == 0;
        for (i, buffer) in self.buffers.iter_mut().enumerate() {
            if i == self.current_buffer && !to_clear {
                continue;
            }
            for channel in buffer {
                channel.clear();
            }
        }
    }

    fn available_all(&self) -> usize {
        if self.buffers.is_empty() {
            return 0;
        }
        (self.buffers.len() - 1) * self.frames + self.available(self.current_buffer, 0)
    }

    fn available(&self, buffer_index: usize, channel: usize) -> usize {
        self.frames - self.buffers[buffer_index][channel].len()
    }

    fn next_buffer(&self, buffer_index: usize) -> usize {
        (buffer_index + 1) % self.buffers.len()
    }

    fn iter(&self) -> ResamplerBufferIter {
        let mut iter_pos = self.next_buffer(self.current_buffer);
        while self.available(iter_pos, 0) == self.frames {
            iter_pos = self.next_buffer(iter_pos)
        }
        ResamplerBufferIter {
            buffer: self,
            iter_pos: Some(iter_pos),
        }
    }
}

struct ResamplerBufferIter<'b> {
    buffer: &'b ResamplerBuffer,
    iter_pos: Option<usize>,
}

impl<'b> Iterator for ResamplerBufferIter<'b> {
    type Item = &'b [Vec<f64>];

    fn next(&mut self) -> Option<Self::Item> {
        let iter_pos = match self.iter_pos {
            Some(iter_pos) => iter_pos,
            None => return None,
        };
        let next_iter_pos = if iter_pos == self.buffer.current_buffer {
            if self.buffer.available(iter_pos, 0) > 0 {
                return None;
            } else {
                None
            }
        } else {
            Some(self.buffer.next_buffer(iter_pos))
        };
        let item = &self.buffer.buffers[iter_pos];
        self.iter_pos = next_iter_pos;
        Some(item)
    }
}
