use anyhow::{Result, bail};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use std::io::{Read, Write};

/// Quantization step used by AndroidX Ink's numeric-run protobuf.
/// One 1024th of a dp is well below both digitizer precision and a display pixel.
const POSITION_SCALE: f32 = 1.0 / 1024.0;
const TIME_SCALE: f32 = 0.001;

pub fn encode_stroke_input_batch(points: &[(f32, f32)]) -> Result<Vec<u8>> {
    if points.is_empty() {
        bail!("cannot encode an empty stroke")
    }
    if points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        bail!("cannot encode non-finite ink coordinates")
    }

    let x = points.iter().map(|point| point.0).collect::<Vec<_>>();
    let y = points.iter().map(|point| point.1).collect::<Vec<_>>();
    let elapsed = (0..points.len())
        .map(|index| index as f32 * TIME_SCALE)
        .collect::<Vec<_>>();

    let mut proto = Vec::new();
    push_message(1, &numeric_run(&x, POSITION_SCALE), &mut proto);
    push_message(2, &numeric_run(&y, POSITION_SCALE), &mut proto);
    push_message(3, &numeric_run(&elapsed, TIME_SCALE), &mut proto);
    // CodedStrokeInputBatch.ToolType.STYLUS.
    push_varint_field(7, 3, &mut proto);

    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&proto)?;
    Ok(gzip.finish()?)
}

fn numeric_run(values: &[f32], scale: f32) -> Vec<u8> {
    let offset = values[0];
    let quantized = values
        .iter()
        .map(|value| ((value - offset) / scale).round() as i32)
        .collect::<Vec<_>>();
    let mut previous = 0i32;
    let deltas = quantized
        .into_iter()
        .map(|value| {
            let delta = value.saturating_sub(previous);
            previous = value;
            delta
        })
        .collect::<Vec<_>>();

    let mut packed = Vec::new();
    for delta in deltas {
        push_varint(zigzag(delta) as u64, &mut packed);
    }

    let mut run = Vec::new();
    push_bytes_field(1, &packed, &mut run);
    push_fixed32_field(2, scale, &mut run);
    push_fixed32_field(3, offset, &mut run);
    run
}

fn zigzag(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

fn push_message(field: u32, message: &[u8], output: &mut Vec<u8>) {
    push_bytes_field(field, message, output);
}

fn push_bytes_field(field: u32, bytes: &[u8], output: &mut Vec<u8>) {
    push_varint(((field << 3) | 2) as u64, output);
    push_varint(bytes.len() as u64, output);
    output.extend_from_slice(bytes);
}

fn push_varint_field(field: u32, value: u64, output: &mut Vec<u8>) {
    push_varint((field << 3) as u64, output);
    push_varint(value, output);
}

fn push_fixed32_field(field: u32, value: f32, output: &mut Vec<u8>) {
    push_varint(((field << 3) | 5) as u64, output);
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

/// Lightweight validation used by the standalone converter. Android performs the authoritative
/// semantic decode during import; this catches truncation and accidental non-gzip payloads early.
pub fn validate_gzip_proto(bytes: &[u8]) -> Result<()> {
    let mut decoder = GzDecoder::new(bytes);
    let mut proto = Vec::new();
    decoder.read_to_end(&mut proto)?;
    if proto.is_empty() || proto.first() != Some(&0x0a) {
        bail!("invalid CodedStrokeInputBatch payload")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_gzip_coded_stroke_batch() {
        let encoded = encode_stroke_input_batch(&[(10.0, 20.0), (10.5, 21.25)]).unwrap();
        assert_eq!(&encoded[..2], &[0x1f, 0x8b]);
        validate_gzip_proto(&encoded).unwrap();
    }
}
