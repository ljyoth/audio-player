use core::panic;
use std::collections::VecDeque;

use rubato::{
    ResampleError, Resampler, ResamplerConstructionError, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::Channels,
    codecs::{self, CodecParameters},
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

pub(super) struct RubatoResamplerBuffered {
    resampler: RubatoResampler,
    queue: VecDeque<SampleBuffer>,
    buffer_position: usize,
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
            queue: VecDeque::new(),
            buffer_position: 0,
        })
    }

    pub(super) fn resample(
        &mut self,
        buffer: SampleBuffer,
    ) -> Result<BufferedResamples, ResamplerError> {
        self.queue.push_back(buffer);
        Ok(BufferedResamples { resampler: self })
    }

    fn resample_next(&mut self) -> Result<Option<&SampleBuffer>, ResamplerError> {
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
            self.buffer_position = fill_f64_buffer_22(
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

pub(super) struct BufferedResamples<'r> {
    resampler: &'r mut RubatoResamplerBuffered,
}

impl<'r> BufferedResamples<'r> {
    pub(super) fn next<'a>(&'a mut self) -> Option<Result<&SampleBuffer, ResamplerError>> {
        self.resampler.resample_next().transpose()
    }
}

pub(super) struct RubatoResampler {
    resampler: SincFixedIn<f64>,
    input_buffer: Vec<Vec<f64>>,
    output_buffer: SampleBuffer,
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

        let input_buffer = (0..channels.count())
            .map(|_| Vec::with_capacity(chunk_size))
            .collect();
        // Need to pre-fill or resampler will fail
        let output_buffer = resampler.output_buffer_allocate(true);
        let output_buffer = SampleBuffer::with_buffer(output_buffer);
        let output_buffer_frames = output_buffer.frames();

        Ok(Self {
            resampler,
            input_buffer,
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

    fn resample_inner(&mut self) -> Result<&SampleBuffer, ResamplerError> {
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

        debug!("input: {} output: {}", input_frames, output_frames,);
        Ok(&self.output_buffer)
    }
}

/// `end`: Exclusive
fn fill_f64_buffer_22(
    buffer: &SampleBuffer,
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
            .samples(c)
            .unwrap()
            .iter()
            .skip(start)
            .take(to_take)
            .for_each(|&s| {
                f64_buffer[c].push(s);
                pushed += 1;
            });
        println!("pushed: {} cap: {}", pushed, f64_buffer[0].capacity());
        if pushed == 0 {
            panic!()
        }
    });
    pushed / f64_buffer.len() + start
}
