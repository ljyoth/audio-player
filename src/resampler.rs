use rubato::Resampler;
use symphonia::core::audio::{AudioBufferRef, Signal};

// fn convert_symphonia_buffer_to_rubato_buffer<T: rubato::Sample>(
//     buffer: AudioBufferRef,
// ) -> Vec<&[f32]> {
//     let buffer: Vec<&[f32]> = match buffer {
//         AudioBufferRef::U8(_) => todo!(),
//         AudioBufferRef::U16(_) => todo!(),
//         AudioBufferRef::U24(_) => todo!(),
//         AudioBufferRef::U32(_) => todo!(),
//         AudioBufferRef::S8(_) => todo!(),
//         AudioBufferRef::S16(_) => todo!(),
//         AudioBufferRef::S24(_) => todo!(),
//         AudioBufferRef::S32(_) => todo!(),
//         AudioBufferRef::F32(ref buffer) => (0..2).map(|c| buffer.chan(c)),
//         AudioBufferRef::F64(_) => todo!(),
//     }
//     .collect();
//     buffer
// }
