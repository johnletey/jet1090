/// Beast frame parsing utilities
///
/// Beast format:
///  - `0x1a 0x32` + 6-byte MLAT timestamp + 1-byte signal + 7-byte Mode S short = 16 bytes
///  - `0x1a 0x33` + 6-byte MLAT timestamp + 1-byte signal + 14-byte Mode S long = 23 bytes
use rs1090::decode::bds::DecodedBds;

/// Extract timestamp (seconds since midnight) from 6-byte Beast MLAT field.
///
/// The 6 bytes are zero-padded into a big-endian u64.
/// Upper 18 bits → seconds, lower 30 bits → nanoseconds.
pub fn decode_beast_timestamp(ts_bytes: &[u8]) -> f64 {
    let mut ts_array = [0u8; 8];
    ts_array[2..8].copy_from_slice(ts_bytes);
    let ts_u64 = u64::from_be_bytes(ts_array);
    let seconds = ts_u64 >> 30;
    let nanos = ts_u64 & 0x00003FFFFFFF;
    seconds as f64 + nanos as f64 * 1e-9
}

/// Extract the Mode S payload from a raw Beast frame (bytes already decoded from hex).
///
/// Returns `(timestamp, frame)` where `frame` is the Mode S message bytes,
/// or `None` if the bytes are not a valid Beast frame.
pub fn extract_beast_frame(bytes: &[u8]) -> Option<(f64, Vec<u8>)> {
    if bytes.len() < 16 || bytes[0] != 0x1a {
        return None;
    }

    match bytes[1] {
        0x32 if bytes.len() >= 16 => {
            let timestamp = decode_beast_timestamp(&bytes[2..8]);
            let frame = bytes[9..16].to_vec();
            Some((timestamp, frame))
        }
        0x33 if bytes.len() >= 23 => {
            let timestamp = decode_beast_timestamp(&bytes[2..8]);
            let frame = bytes[9..23].to_vec();
            Some((timestamp, frame))
        }
        _ => None,
    }
}

/// Map a `DecodedBds` variant to its numeric BDS code.
pub fn bds_code_from_decoded(decoded: &DecodedBds) -> u8 {
    match decoded {
        DecodedBds::Bds05(_) => 0x05,
        DecodedBds::Bds06(_) => 0x06,
        DecodedBds::Bds08(_) => 0x08,
        DecodedBds::Bds09(_) => 0x09,
        DecodedBds::Bds10(_) => 0x10,
        DecodedBds::Bds17(_) => 0x17,
        DecodedBds::Bds18(_) => 0x18,
        DecodedBds::Bds19(_) => 0x19,
        DecodedBds::Bds20(_) => 0x20,
        DecodedBds::Bds21(_) => 0x21,
        DecodedBds::Bds30(_) => 0x30,
        DecodedBds::Bds40(_) => 0x40,
        DecodedBds::Bds44(_) => 0x44,
        DecodedBds::Bds45(_) => 0x45,
        DecodedBds::Bds50(_) => 0x50,
        DecodedBds::Bds60(_) => 0x60,
        DecodedBds::Bds61(_) => 0x61,
        DecodedBds::Bds62(_) => 0x62,
        DecodedBds::Bds65(_) => 0x65,
    }
}
