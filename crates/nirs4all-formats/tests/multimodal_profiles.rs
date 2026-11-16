//! CC0 synthetic profile fixtures; these are not vendor conformance claims.
use nirs4all_formats::{decode_multimodal_bytes, DecodedMultimodal};

#[test]
fn png_pixels_preserve_hwc_and_refuse_alpha_crc_and_budget() {
    let bytes = include_bytes!("fixtures/multimodal/gray-2x2.png");
    match decode_multimodal_bytes("png8", bytes, 1024).unwrap() {
        DecodedMultimodal::Image {
            shape,
            pixels,
            layout,
            ..
        } => {
            assert_eq!(shape, [2, 2, 1]);
            assert_eq!(pixels, [0, 64, 128, 255]);
            assert_eq!(layout, "HWC");
        }
        _ => panic!("expected image"),
    }
    assert!(decode_multimodal_bytes("png8", bytes, 5).is_err());
    assert!(decode_multimodal_bytes(
        "png8",
        include_bytes!("fixtures/multimodal/alpha-refused.png"),
        1024
    )
    .is_err());
    let mut corrupt = bytes.to_vec();
    corrupt[20] ^= 1;
    assert!(decode_multimodal_bytes("png8", &corrupt, 1024).is_err());
}

#[test]
fn vcf_retains_exact_sample_order_phase_and_missingness() {
    let text = "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tb\ta\n1\t10\tv1\tA\tG\t.\tPASS\t.\tGT:DP\t0|1:9\t1/1:8\n1\t20\tv2\tC\tT\t.\tPASS\t.\tGT\t./.\t0/0\n";
    match decode_multimodal_bytes("vcf_gt_dosage", text.as_bytes(), 4096).unwrap() {
        DecodedMultimodal::Genotype {
            sample_ids,
            variant_ids,
            data,
            validity,
            phased_calls,
            missing_calls,
            ..
        } => {
            assert_eq!(sample_ids, ["b", "a"]);
            assert_eq!(variant_ids, ["1:10:A:G", "1:20:C:T"]);
            assert_eq!(data, [1.0, 0.0, 2.0, 0.0]);
            assert_eq!(validity, [1, 0, 1, 1]);
            assert_eq!(phased_calls, 1);
            assert_eq!(missing_calls, 1);
        }
        _ => panic!("expected genotype"),
    }
    for replacement in ["0/2:9", "0:9", "0/1/1:9"] {
        assert!(decode_multimodal_bytes(
            "vcf_gt_dosage",
            text.replace("0|1:9", replacement).as_bytes(),
            4096
        )
        .is_err());
    }
    assert!(decode_multimodal_bytes(
        "vcf_gt_dosage",
        text.replace("\tA\tG\t", "\tA\tG,T\t").as_bytes(),
        4096
    )
    .is_err());
}
