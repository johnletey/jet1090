use deku::prelude::*;
#[cfg(feature = "bds-infer")]
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

/**
 * ## Aircraft and Airline Registration Markings (BDS 2,1)
 *
 * Comm-B message providing aircraft and airline registration information.  
 * Per ICAO Doc 9871 Table A-2-33: BDS code 2,1 — Aircraft and airline registration markings
 *
 * Purpose: To permit ground systems to identify the aircraft without the
 * necessity of compiling and maintaining continuously updated data banks.
 *
 * Message Structure (56 bits):
 * | AC_STAT | AC_REG (7 chars) | AL_STAT | AL_REG (2 chars) |
 * |---------|------------------|---------|------------------|
 * | 1       | 42 (6×7)         | 1       | 12 (6×2)         |
 *
 * Field Encoding per ICAO Doc 9871:
 *
 * **Aircraft Registration Status** (bit 1):
 *   - 0 = aircraft registration not available or invalid
 *   - 1 = aircraft registration available and valid
 *
 * **Aircraft Registration Number** (bits 2-43): 7 characters, 6 bits each
 *   - Character encoding per ICAO Annex 10, Vol IV, Table 3-7
 *   - Valid characters: A-Z (1-26), 0-9 (48-57), # (0), space (32)
 *   - Example formats: "N12345", "GABCD", "VHVKI" (national dash stripped)
 *   - Must match pattern: [A-Z0-9]+[\\s#]?[A-Z0-9]+
 *
 * **Airline Registration Status** (bit 44):
 *   - 0 = airline designation not available or invalid
 *   - 1 = airline designation available and valid
 *
 * **ICAO Airline Registration Marking** (bits 45-56): 2 characters, 6 bits each
 *   - Character encoding per ICAO Annex 10, Vol IV, Table 3-7
 *   - Valid characters: A-Z (1-26), 0-9 (48-57)
 *   - Note: Most transponders don't implement this field (status=0)
 *
 * Character Set (6-bit encoding):
 * - 0 (000000) = # (no character marker)
 * - 1-26 (000001-011010) = A-Z
 * - 32 (100000) = space
 * - 48-57 (110000-111001) = 0-9
 *
 * Validation Rules:
 * - If status bit is 0, all character bits must be 0
 * - If status bit is 1, characters must form valid registration
 * - Aircraft registration must match expected national format
 * - Airline designation rarely implemented in practice
 *
 * Note: This provides aircraft tail number/registration separate from
 * callsign (BDS 2,0/0,8), allowing ground systems to identify aircraft
 * without maintaining extensive databases.
 *
 * Reference: ICAO Doc 9871 Table A-2-33, Annex 10 Vol IV Table 3-7
 */

#[derive(
    Debug, PartialEq, Serialize, Deserialize, DekuRead, Clone, Default,
)]
#[serde(tag = "bds", rename = "21")]
pub struct AircraftAndAirlineRegistrationMarkings {
    #[deku(bits = "1")]
    #[serde(skip)]
    /// Aircraft Registration Status
    pub ac_status: bool,

    #[deku(reader = "aircraft_registration_read(deku::reader, *ac_status)")]
    #[serde(rename = "registration")]
    /// Aircraft Registration Number (7 characters)
    pub aircraft_registration: Option<String>,

    #[deku(bits = "1")]
    #[serde(skip)]
    /// Airline Registration Status
    pub al_status: bool,

    #[deku(reader = "airline_registration_read(deku::reader, *al_status)")]
    #[serde(rename = "airline", skip_serializing_if = "Option::is_none")]
    /// ICAO Airline Registration Marking (2 characters)
    pub airline_registration: Option<String>,
}

// Per ICAO Annex 10 Vol IV Table 3-7, the 6-bit BDS 2,1 alphabet only defines
// A–Z (1–26), SPACE (32), and 0–9 (48–57). There is **no hyphen character**
// in the alphabet. Real registrations such as "B-2487" must be transmitted
// without the dash; the avionics use one of three placeholder values for
// the dash slot, and we treat all three the same way (strip them):
//
//   * 32 (100000)  — SPACE: the standards-conformant placeholder.
//   * 0  (000000)  — NULL:  used by some implementations; not in the alphabet
//                    but interpreted here as an empty character (≡ space).
//   * 45 (101101)  — dash placeholder observed empirically in our Jan 2025
//                    dataset (B#7838, VH#8MR, XA#ADD, ...). Code 45 is in
//                    the reserved range (33–47) but is used consistently by
//                    multiple avionics vendors as the national dash. Treated
//                    as a placeholder rather than noise.
//
// Other reserved code points (27–31, 33–44, 46–47, 58–63) are kept as '#'
// so that genuinely-corrupt or out-of-band payloads (e.g. random BDS 4,0
// bits decoded as BDS 2,1 character noise) remain visible to downstream.
const CHAR_LOOKUP: &[u8; 64] =
    b" ABCDEFGHIJKLMNOPQRSTUVWXYZ##### ############-##0123456789######";

/// Returns true when this 6-bit code is a national-dash placeholder that
/// should be stripped from the decoded registration string.
#[inline]
fn is_dash_placeholder(c: u8) -> bool {
    c == 0 || c == 32 || c == 45
}

pub fn aircraft_registration_read<
    R: deku::no_std_io::Read + deku::no_std_io::Seek,
>(
    reader: &mut Reader<R>,
    status: bool,
) -> Result<Option<String>, DekuError> {
    let mut chars = vec![];
    for _ in 0..=6 {
        let c = u8::from_reader_with_ctx(reader, deku::ctx::BitSize(6))?;
        trace!("Reading letter {}", CHAR_LOOKUP[c as usize] as char);
        // Strip national-dash placeholders (space / null / 45) so the decoded
        // string is the dash-less registration (e.g. "B 2487", "B\x002487"
        // and "B-2487" all decode to "B2487"). Downstream callers can re-insert
        // the dash from the icao24 range (see patterns.json).
        if !is_dash_placeholder(c) {
            chars.push(c);
        }
    }

    let all_zeros = chars.is_empty();
    let encoded = chars
        .into_iter()
        .map(|b| CHAR_LOOKUP[b as usize] as char)
        .collect::<String>();
    debug!("Decoded registration: {}", encoded);

    if status {
        #[cfg(not(feature = "bds-infer"))]
        {
            Ok(Some(encoded))
        }
        #[cfg(feature = "bds-infer")]
        {
            let re = Regex::new(r"^[A-Z0-9]+[\s#]?[A-Z0-9]+$").unwrap();
            if re.is_match(&encoded) {
                Ok(Some(encoded))
            } else {
                Err(DekuError::Assertion(
                    format!("Invalid aircraft registration {encoded}").into(),
                ))
            }
        }
    } else if all_zeros {
        Ok(None)
    } else {
        Err(DekuError::Assertion(
            format!(
                "Non-null value after invalid aircraft registration status: {encoded}"
            )
            .into(),
        ))
    }
}

pub fn airline_registration_read<
    R: deku::no_std_io::Read + deku::no_std_io::Seek,
>(
    reader: &mut Reader<R>,
    status: bool,
) -> Result<Option<String>, DekuError> {
    let mut chars = vec![];
    for _ in 0..2 {
        let c = u8::from_reader_with_ctx(reader, deku::ctx::BitSize(6))?;
        trace!("Reading letter {}", CHAR_LOOKUP[c as usize] as char);
        if !is_dash_placeholder(c) {
            chars.push(c);
        }
    }
    let all_zeros = chars.is_empty();
    let encoded = chars
        .into_iter()
        .map(|b| CHAR_LOOKUP[b as usize] as char)
        .collect::<String>();

    if status {
        // Ok((inside_rest, Some(encoded)))
        Err(DekuError::Assertion(
            format!(
                "Most transponders don't implement this field. (value = {encoded})"
            )
            .into(),
        ))
    } else if all_zeros {
        Ok(None)
    } else {
        Err(DekuError::Assertion(
            format!(
                "Non-null value after invalid airline registration status: {encoded}"
            )
            .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use hexlit::hex;

    #[test]
    fn test_valid_bds21() {
        let bytes = hex!("a00002bf940f19680c0000000000");
        let (_, msg) = Message::from_bytes((&bytes, 0)).unwrap();
        if let CommBAltitudeReply { bds, .. } = msg.df {
            let AircraftAndAirlineRegistrationMarkings {
                aircraft_registration,
                ..
            } = bds.bds21.unwrap();
            assert_eq!(aircraft_registration, Some("JA824A".to_string()));
        } else {
            unreachable!();
        }

        let bytes = hex!("a00002988230c3b470a000000000");
        let (_, msg) = Message::from_bytes((&bytes, 0)).unwrap();
        if let CommBAltitudeReply { bds, .. } = msg.df {
            let AircraftAndAirlineRegistrationMarkings {
                aircraft_registration,
                ..
            } = bds.bds21.unwrap();
            assert_eq!(aircraft_registration, Some("AFFGZNE".to_string()));
        } else {
            unreachable!();
        }
        let bytes = hex!("a0000793ac45ab164c0000000000");
        let (_, msg) = Message::from_bytes((&bytes, 0)).unwrap();
        if let CommBAltitudeReply { bds, .. } = msg.df {
            let AircraftAndAirlineRegistrationMarkings {
                aircraft_registration,
                ..
            } = bds.bds21.unwrap();
            assert_eq!(aircraft_registration, Some("VHVKI".to_string()));
        } else {
            unreachable!();
        }
    }
}
