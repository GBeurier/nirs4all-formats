//! Explicit non-spectral byte profiles. No artificial spectral axis is created.
use std::collections::BTreeSet;
use std::io::Read;

use flate2::read::ZlibDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MULTIMODAL_OPERATION: &str = "nirs4all.formats.multimodal.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecodedMultimodal {
    Image {
        schema: String,
        source_sha256: String,
        shape: Vec<usize>,
        pixels: Vec<u8>,
        layout: String,
        intensity_range: [u16; 2],
        warnings: Vec<String>,
    },
    Genotype {
        schema: String,
        source_sha256: String,
        sample_ids: Vec<String>,
        variant_ids: Vec<String>,
        original_variant_ids: Vec<String>,
        variant_metadata: Vec<serde_json::Value>,
        data: Vec<f64>,
        validity: Vec<u8>,
        phased_calls: usize,
        missing_calls: usize,
        warnings: Vec<String>,
    },
}

/// Registry of declared non-spectral operations, independent of SpectralRecord.
pub fn multimodal_reader_catalog() -> serde_json::Value {
    serde_json::json!([
        {"profile":"png8","operation":MULTIMODAL_OPERATION,"representation":"gray_image|rgb_image",
         "policy":"PNG, 8-bit gray/RGB, noninterlaced; no alpha/palette/16-bit/animation; raw HWC pixels, no resizing or colour management"},
        {"profile":"vcf_gt_dosage","operation":MULTIMODAL_OPERATION,"representation":"tabular_numeric",
         "policy":"VCF 4.x, biallelic diploid GT dosage; phase discarded; missing calls retained with validity false; no DS precedence or imputation"}
    ])
}

pub fn decode_multimodal_bytes(
    profile: &str,
    bytes: &[u8],
    max_decoded_bytes: usize,
) -> Result<DecodedMultimodal, String> {
    if max_decoded_bytes == 0 {
        return Err("host decoded-byte budget must be positive".into());
    }
    match profile {
        "png8" => decode_png(bytes, max_decoded_bytes),
        "vcf_gt_dosage" => decode_vcf(bytes, max_decoded_bytes),
        _ => Err(format!(
            "unsupported declared multimodal profile '{profile}'"
        )),
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn decode_png(bytes: &[u8], budget: usize) -> Result<DecodedMultimodal, String> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("png8 profile requires the PNG signature".into());
    }
    let mut offset = 8_usize;
    let mut shape = None;
    let mut compressed = Vec::new();
    let mut ended = false;
    let mut saw_idat = false;
    let mut idat_closed = false;
    while offset < bytes.len() {
        if bytes.len() - offset < 12 {
            return Err("truncated PNG chunk".into());
        }
        let size = be32(&bytes[offset..offset + 4]) as usize;
        let end = offset
            .checked_add(size)
            .and_then(|n| n.checked_add(12))
            .ok_or("PNG chunk length overflow")?;
        if end > bytes.len() {
            return Err("truncated PNG chunk payload".into());
        }
        let name = &bytes[offset + 4..offset + 8];
        let payload = &bytes[offset + 8..end - 4];
        if crc32(&bytes[offset + 4..end - 4]) != be32(&bytes[end - 4..end]) {
            return Err("PNG chunk CRC mismatch".into());
        }
        if shape.is_none() && name != b"IHDR" {
            return Err("PNG IHDR must be first".into());
        }
        if saw_idat && name != b"IDAT" {
            idat_closed = true;
        }
        match name {
            b"IHDR" => {
                if shape.is_some() || payload.len() != 13 {
                    return Err("invalid or duplicate PNG IHDR".into());
                }
                let width = be32(&payload[..4]) as usize;
                let height = be32(&payload[4..8]) as usize;
                let channels = match payload[9] {
                    0 => 1_usize,
                    2 => 3,
                    _ => return Err("png8 refuses palette and alpha colour types".into()),
                };
                if width == 0 || height == 0 || payload[8] != 8 || payload[10..13] != [0, 0, 0] {
                    return Err("png8 requires positive dimensions, 8-bit depth, standard compression/filter and no interlace".into());
                }
                let decoded = width
                    .checked_mul(channels)
                    .and_then(|n| n.checked_add(1))
                    .and_then(|n| n.checked_mul(height))
                    .ok_or("PNG shape overflow")?;
                if decoded > budget {
                    return Err("PNG decoded pixels exceed the host byte budget".into());
                }
                shape = Some((height, width, channels));
            }
            b"IDAT" => {
                if idat_closed {
                    return Err("PNG IDAT chunks must be contiguous".into());
                }
                saw_idat = true;
                compressed.extend_from_slice(payload);
            }
            b"IEND" => {
                if !payload.is_empty() || !saw_idat || end != bytes.len() {
                    return Err("invalid PNG IEND or trailing bytes".into());
                }
                ended = true;
                break;
            }
            b"tRNS" | b"acTL" | b"fcTL" | b"fdAT" => {
                return Err("png8 refuses transparency and animated PNG".into())
            }
            _ if name[0] & 0x20 == 0 => {
                return Err("png8 refuses an unsupported critical PNG chunk".into())
            }
            _ => {}
        }
        offset = end;
    }
    if !ended {
        return Err("PNG IEND is missing".into());
    }
    let (height, width, channels) = shape.ok_or("PNG shape is missing")?;
    let row_bytes = width * channels;
    let expected = (row_bytes + 1) * height;
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut filtered = Vec::with_capacity(expected);
    decoder
        .by_ref()
        .take(expected as u64 + 1)
        .read_to_end(&mut filtered)
        .map_err(|error| format!("PNG zlib stream: {error}"))?;
    if filtered.len() != expected || decoder.total_in() as usize != compressed.len() {
        return Err("PNG decoded length or compressed stream extent mismatch".into());
    }
    let mut pixels = vec![0_u8; height * row_bytes];
    for row in 0..height {
        let filter = filtered[row * (row_bytes + 1)];
        if filter > 4 {
            return Err("unsupported PNG row filter".into());
        }
        for column in 0..row_bytes {
            let index = row * row_bytes + column;
            let left = if column >= channels {
                pixels[index - channels]
            } else {
                0
            };
            let up = if row > 0 {
                pixels[index - row_bytes]
            } else {
                0
            };
            let diagonal = if row > 0 && column >= channels {
                pixels[index - row_bytes - channels]
            } else {
                0
            };
            let predictor = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                _ => {
                    let p = i32::from(left) + i32::from(up) - i32::from(diagonal);
                    let a = (p - i32::from(left)).abs();
                    let b = (p - i32::from(up)).abs();
                    let c = (p - i32::from(diagonal)).abs();
                    if a <= b && a <= c {
                        left
                    } else if b <= c {
                        up
                    } else {
                        diagonal
                    }
                }
            };
            pixels[index] = filtered[row * (row_bytes + 1) + column + 1].wrapping_add(predictor);
        }
    }
    Ok(DecodedMultimodal::Image {
        schema: MULTIMODAL_OPERATION.into(),
        source_sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        shape: vec![height, width, channels],
        pixels,
        layout: "HWC".into(),
        intensity_range: [0, 255],
        warnings: vec![
            "Raw encoded pixel values; no colour management, resizing or intensity normalization"
                .into(),
        ],
    })
}

fn decode_vcf(bytes: &[u8], budget: usize) -> Result<DecodedMultimodal, String> {
    if bytes.len() > budget {
        return Err("VCF bytes exceed the host decode budget".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "VCF requires UTF-8 text")?;
    let mut lines = text.lines();
    if !lines
        .next()
        .is_some_and(|line| line.starts_with("##fileformat=VCFv4."))
    {
        return Err("vcf_gt_dosage requires an explicit VCF 4.x header".into());
    }
    let mut samples = Vec::new();
    let mut variant_ids = Vec::new();
    let mut original_variant_ids = Vec::new();
    let mut variant_metadata = Vec::new();
    let mut variants = Vec::<Vec<(f64, u8)>>::new();
    let mut seen_variants = BTreeSet::new();
    let mut phased_calls = 0;
    let mut missing_calls = 0;
    for (line_index, line) in lines.enumerate() {
        if line.starts_with("##") && samples.is_empty() {
            continue;
        }
        if line.starts_with("#CHROM\t") {
            if !samples.is_empty() {
                return Err("duplicate VCF sample header".into());
            }
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() < 10
                || fields[..9]
                    != [
                        "#CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO", "FORMAT",
                    ]
            {
                return Err("VCF requires fixed columns, FORMAT and sample IDs".into());
            }
            samples = fields[9..]
                .iter()
                .map(|value| (*value).to_string())
                .collect();
            if samples.iter().any(|id| id.trim().is_empty())
                || samples.iter().collect::<BTreeSet<_>>().len() != samples.len()
            {
                return Err("VCF sample IDs must be nonempty and unique".into());
            }
            continue;
        }
        if samples.is_empty() || line.starts_with('#') || line.is_empty() {
            return Err(format!("unexpected VCF line {}", line_index + 2));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != samples.len() + 9 {
            return Err("VCF row/sample field count mismatch".into());
        }
        let position = fields[1]
            .parse::<u64>()
            .map_err(|_| "VCF POS must be a positive integer")?;
        let allele =
            |value: &str| !value.is_empty() && value.bytes().all(|c| b"ACGTNacgtn".contains(&c));
        if position == 0
            || fields[0].is_empty()
            || !allele(fields[3])
            || !allele(fields[4])
            || fields[3] == fields[4]
        {
            return Err("VCF profile requires explicit biallelic sequence REF/ALT and positive POS; multiallelic/symbolic records are refused".into());
        }
        let variant = format!("{}:{}:{}:{}", fields[0], position, fields[3], fields[4]);
        if !seen_variants.insert(variant.clone()) {
            return Err("duplicate VCF variant identity".into());
        }
        let formats = fields[8].split(':').collect::<Vec<_>>();
        if formats.iter().collect::<BTreeSet<_>>().len() != formats.len() {
            return Err("duplicate VCF FORMAT key".into());
        }
        let gt_index = formats
            .iter()
            .position(|name| *name == "GT")
            .ok_or("VCF profile requires GT; DS is not substituted")?;
        let mut calls = Vec::with_capacity(samples.len());
        for cell in &fields[9..] {
            let values = cell.split(':').collect::<Vec<_>>();
            let gt = values.get(gt_index).ok_or("missing VCF GT field")?;
            if gt.contains('|') && gt.contains('/') {
                return Err("ambiguous VCF GT separators".into());
            }
            let alleles = gt.split(['|', '/']).collect::<Vec<_>>();
            if alleles.len() != 2 || alleles.iter().any(|value| !["0", "1", "."].contains(value)) {
                return Err("VCF profile supports diploid 0/1 alleles only".into());
            }
            if gt.contains('|') {
                phased_calls += 1;
            }
            if alleles.contains(&".") {
                missing_calls += 1;
                calls.push((0.0, 0));
            } else {
                calls.push((
                    f64::from(u8::from(alleles[0] == "1") + u8::from(alleles[1] == "1")),
                    1,
                ));
            }
        }
        let cells = (variants.len() + 1)
            .checked_mul(samples.len())
            .and_then(|n| n.checked_mul(9))
            .ok_or("VCF dosage shape overflow")?;
        if cells > budget {
            return Err("VCF dosage matrix exceeds the host decoded-byte budget".into());
        }
        variant_ids.push(variant);
        original_variant_ids.push(fields[2].to_string());
        variant_metadata.push(serde_json::json!({"chrom":fields[0],"position":position,"id":fields[2],
            "reference":fields[3],"alternate":fields[4],"quality":fields[5],"filter":fields[6],"info":fields[7],
            "format":fields[8],"sample_fields":fields[9..]}));
        variants.push(calls);
    }
    if samples.is_empty() || variants.is_empty() {
        return Err("VCF has no sample header or genotype records".into());
    }
    let mut data = Vec::with_capacity(samples.len() * variants.len());
    let mut validity = Vec::with_capacity(data.capacity());
    for sample in 0..samples.len() {
        for variant in &variants {
            data.push(variant[sample].0);
            validity.push(variant[sample].1);
        }
    }
    Ok(DecodedMultimodal::Genotype { schema: MULTIMODAL_OPERATION.into(), source_sha256: Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect(),
        sample_ids: samples, variant_ids, original_variant_ids, variant_metadata, data, validity, phased_calls, missing_calls,
        warnings: vec!["GT dosage uses the alternate-allele count; phase discarded; missing calls retain validity=false, never imputed".into()] })
}
