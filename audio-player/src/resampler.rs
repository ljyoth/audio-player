use std::borrow::Cow;

use rubato::{
    ResampleError, Resampler, ResamplerConstructionError, SincFixedIn,
    SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::{AsAudioBufferRef, AudioBuffer, AudioBufferRef, Signal, SignalSpec},
    codecs::CodecParameters,
    conv::IntoSample,
    sample::Sample,
    units::Duration,
};
use tracing::{debug, info};

#[derive(Debug, thiserror::Error)]
pub(super) enum ResamplerError {
    #[error("Invalid CodecParameters")]
    InvalidCodecParameters,
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
        let output_buffer = Resampler::output_buffer_allocate(&resampler, true);
        let interleaved = Vec::with_capacity(output_buffer[0].len() * output_buffer.len());

        let output_audio_buffer = AudioBuffer::new(
            output_buffer[0].len() as Duration,
            SignalSpec {
                rate: output_sample_rate,
                channels,
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
