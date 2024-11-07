use std::borrow::Cow;

use cpal::FromSample;
use rubato::{
    ResampleError, Resampler, ResamplerConstructionError, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::{AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal, SignalSpec},
    conv::IntoSample,
    sample::Sample,
    units::Duration,
};
use tracing::{debug, info};

#[derive(Debug, thiserror::Error)]
pub(super) enum ResamplerError {
    #[error("Rubato ResamplerConstructionError: {0}")]
    RubatoResamplerConstruction(#[from] ResamplerConstructionError),
    #[error("Rubato ResampleError: {0}")]
    RubatoResample(#[from] ResampleError),
}

pub(super) struct SymphoniaResampler {
    resampler: SincFixedIn<f64>,
    input_buffer: Vec<Vec<f64>>,
    output_buffer: Vec<Vec<f64>>,
    output_audio_buffer: AudioBuffer<f64>,
    interleaved: Vec<f64>,
}

impl SymphoniaResampler {
    pub(super) fn new(
        output_sample_rate: u32,
        buffer: &AudioBufferRef,
    ) -> Result<Self, ResamplerError> {
        let spec = buffer.spec();
        let resampler = SincFixedIn::new(
            output_sample_rate as f64 / spec.rate as f64,
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
        )?;
        // let mut resampler = FftFixedIn::new(
        //     self.sample_rate as usize,
        //     spec.rate as usize,
        //     buffer.frames(),
        //     2,
        //     spec.channels.count(),
        // )?;

        let input_buffer = (0..spec.channels.count())
            .map(|_| Vec::with_capacity(buffer.frames()))
            .collect();
        // Need to pre-fill or resampler will fail
        let output_buffer = Resampler::output_buffer_allocate(&resampler, true);
        let interleaved = Vec::with_capacity(output_buffer[0].len() * output_buffer.len());

        let output_audio_buffer = AudioBuffer::new(
            output_buffer[0].len() as Duration,
            SignalSpec {
                rate: output_sample_rate,
                channels: spec.channels,
            },
        );
        info!(
            "output_buffer_len: {} interleaved_length: {} output_buffer_capacity: {} output_buffer_frames: {}\n",
            output_buffer[0].len(),
            interleaved.len(),
            output_audio_buffer.capacity(),
            output_audio_buffer.frames()
        );
        Ok(Self {
            resampler,
            input_buffer,
            output_buffer,
            output_audio_buffer,
            interleaved,
        })
    }

    pub(super) fn resample(
        &mut self,
        buffer: AudioBufferRef,
    ) -> Result<AudioBufferRef, ResamplerError> {
        let spec = buffer.spec();
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

        let (input_frames, output_frames) = Resampler::process_into_buffer(
            &mut self.resampler,
            &self.input_buffer,
            &mut self.output_buffer,
            None,
        )?;

        self.output_audio_buffer.clear();
        self.output_audio_buffer
            .render_reserved(Some(output_frames));
        (0..self.output_buffer.len()).for_each(|c| {
            self.output_audio_buffer
                .chan_mut(c)
                .iter_mut()
                .zip(self.output_buffer[c][0..output_frames].iter())
                .for_each(|(s, &f)| {
                    *s = f;
                })
        });

        debug!(
            "input_buffer: {} input: {} output: {} output_buffer_capacity: {} output_buffer_frames: {}",
            buffer.frames(),
            input_frames,
            output_frames,
            self.output_audio_buffer.capacity(),
            self.output_audio_buffer.frames()
        );
        Ok(self.output_audio_buffer.as_audio_buffer_ref())
    }

    pub(super) fn resample_into_f64(
        &mut self,
        buffer: AudioBufferRef,
    ) -> Result<&[f64], ResamplerError> {
        let spec = buffer.spec();

        match buffer {
            AudioBufferRef::U8(_) => todo!(),
            AudioBufferRef::U16(_) => todo!(),
            AudioBufferRef::U24(_) => todo!(),
            AudioBufferRef::U32(_) => todo!(),
            AudioBufferRef::S8(_) => todo!(),
            AudioBufferRef::S16(_) => todo!(),
            AudioBufferRef::S24(_) => todo!(),
            AudioBufferRef::S32(ref buffer) => (0..spec.channels.count()).for_each(|c| {
                self.input_buffer[c].clear();
                buffer
                    .chan(c)
                    .iter()
                    .for_each(|&s| self.input_buffer[c].push(s.into_sample()));
            }),
            AudioBufferRef::F32(ref buffer) => (0..spec.channels.count()).for_each(|c| {
                self.input_buffer[c].clear();
                buffer
                    .chan(c)
                    .iter()
                    .for_each(|&s| self.input_buffer[c].push(s.into_sample()));
            }),
            AudioBufferRef::F64(_) => todo!(),
        };

        let (_, output_frames) = Resampler::process_into_buffer(
            &mut self.resampler,
            &self.input_buffer,
            &mut self.output_buffer,
            None,
        )?;
        self.interleaved.clear();
        for i in 0..output_frames {
            for ch in 0..spec.channels.count() {
                self.interleaved.push(self.output_buffer[ch][i]);
            }
        }
        Ok(self.interleaved.as_slice())
    }
}

fn fill_f64_buffer<S: Sample + IntoSample<f64>>(
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
