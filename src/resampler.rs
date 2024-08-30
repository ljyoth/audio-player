use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use symphonia::core::audio::{AudioBuffer, AudioBufferRef, Signal};

use crate::decoder::DecodedTrack;

pub(super) struct SymphoniaResampler {
    resampler: SincFixedIn<f32>,
    input_buffer: Vec<Vec<f32>>,
    output_buffer: Vec<Vec<f32>>,
    interleaved: Vec<f32>,
}

impl SymphoniaResampler {
    pub(super) fn new(output_sample_rate: u32, buffer: &AudioBufferRef) -> Self {
        let spec = buffer.spec();
        let resampler = SincFixedIn::<f32>::new(
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

        let input_buffer: Vec<Vec<f32>> = (0..spec.channels.count())
            .map(|_| Vec::with_capacity(buffer.frames()))
            .collect();
        let output_buffer = Resampler::output_buffer_allocate(&resampler, true);
        let interleaved = Vec::with_capacity(output_buffer[0].len() * output_buffer.len());
        println!("{} {}", output_buffer[0].len(), interleaved.len());
        Self {
            resampler,
            input_buffer,
            output_buffer,
            interleaved,
        }
    }

    pub(super) fn resample(&mut self, buffer: AudioBufferRef) -> &[f32] {
        let spec = buffer.spec();

        // fn convert_to_input_buffer<T>(buffer: AudioBuffer<T>, &mut input_buffer: Vec<Vec<f32>>) {

        // }
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
                    .for_each(|&s| self.input_buffer[c].push(s as f32));
            }),
            AudioBufferRef::F32(ref buffer) => (0..spec.channels.count()).for_each(|c| {
                self.input_buffer[c].clear();
                buffer
                    .chan(c)
                    .iter()
                    .for_each(|&s| self.input_buffer[c].push(s as f32));
            }),
            AudioBufferRef::F64(_) => todo!(),
        };

        let (input_frames, output_frames) = Resampler::process_into_buffer(
            &mut self.resampler,
            &self.input_buffer,
            &mut self.output_buffer,
            None,
        )
        .unwrap();
        println!(
            "input_buffer: {} input: {} output: {} output_buffer: {} channels: {}",
            buffer.frames(),
            input_frames,
            output_frames,
            self.output_buffer[0].len(),
            self.output_buffer.len()
        );
        self.interleaved.clear();
        for i in 0..output_frames {
            for ch in 0..spec.channels.count() {
                self.interleaved.push(self.output_buffer[ch][i]);
            }
        }
        self.interleaved.as_slice()
    }
}
