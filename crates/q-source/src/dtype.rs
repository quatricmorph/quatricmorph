//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4.1).
//!
//! SafeTensors dtypes and exact scalar decoding.
//!
//! Decoding is deliberately per-scalar / per-slice: there is no API here that
//! materializes a whole tensor, because a whole tensor at checkpoint scale does
//! not fit anywhere. Callers decode the bytes they range-read and nothing else.

use crate::error::{QError, Result};
use serde::{Deserialize, Serialize};

/// The SafeTensors dtype tags Quatricmorph recognises.
///
/// `size_in_bytes` is the *storage* width. Whether a dtype can be decoded to an
/// exact `f64` for the scalar API is a separate question — see
/// [`DType::supports_exact_scalar_read`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DType {
    Bool,
    U8,
    I8,
    F8E4M3,
    F8E5M2,
    I16,
    U16,
    F16,
    BF16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
}

impl DType {
    pub fn size_in_bytes(self) -> u64 {
        match self {
            DType::Bool | DType::U8 | DType::I8 | DType::F8E4M3 | DType::F8E5M2 => 1,
            DType::I16 | DType::U16 | DType::F16 | DType::BF16 => 2,
            DType::I32 | DType::U32 | DType::F32 => 4,
            DType::I64 | DType::U64 | DType::F64 => 8,
        }
    }

    /// SafeTensors header spelling, e.g. `"F32"`, `"BF16"`.
    pub fn as_safetensors_str(self) -> &'static str {
        match self {
            DType::Bool => "BOOL",
            DType::U8 => "U8",
            DType::I8 => "I8",
            DType::F8E4M3 => "F8_E4M3",
            DType::F8E5M2 => "F8_E5M2",
            DType::I16 => "I16",
            DType::U16 => "U16",
            DType::F16 => "F16",
            DType::BF16 => "BF16",
            DType::I32 => "I32",
            DType::U32 => "U32",
            DType::F32 => "F32",
            DType::I64 => "I64",
            DType::U64 => "U64",
            DType::F64 => "F64",
        }
    }

    pub fn parse_safetensors(s: &str) -> Result<DType> {
        Ok(match s {
            "BOOL" => DType::Bool,
            "U8" => DType::U8,
            "I8" => DType::I8,
            "F8_E4M3" => DType::F8E4M3,
            "F8_E5M2" => DType::F8E5M2,
            "I16" => DType::I16,
            "U16" => DType::U16,
            "F16" => DType::F16,
            "BF16" => DType::BF16,
            "I32" => DType::I32,
            "U32" => DType::U32,
            "F32" => DType::F32,
            "I64" => DType::I64,
            "U64" => DType::U64,
            "F64" => DType::F64,
            other => {
                return Err(QError::UnsupportedDType {
                    dtype: other.to_string(),
                    operation: "safetensors header parse".into(),
                })
            }
        })
    }

    /// Whether [`DType::decode_scalar`] can produce a value for this dtype.
    ///
    /// `false` here is a promise, not a limitation to route around: the scalar
    /// API returns [`QError::UnsupportedDType`] rather than an approximation.
    pub fn supports_exact_scalar_read(self) -> bool {
        !matches!(self, DType::F8E4M3 | DType::F8E5M2)
    }

    /// Decode exactly one element from its little-endian storage bytes.
    ///
    /// `bytes.len()` must equal [`DType::size_in_bytes`].
    ///
    /// Float widths narrower than f32 widen losslessly (every f16/bf16 value is
    /// exactly representable in f32, and every f32 in f64), so the returned
    /// `f64` is the *exact* stored value, not a rounded one.
    pub fn decode_scalar(self, bytes: &[u8]) -> Result<f64> {
        let want = self.size_in_bytes() as usize;
        if bytes.len() != want {
            return Err(QError::malformed(
                "scalar decode",
                format!("dtype {self:?} needs {want} bytes, got {}", bytes.len()),
            ));
        }
        Ok(match self {
            DType::Bool => (bytes[0] != 0) as u8 as f64,
            DType::U8 => bytes[0] as f64,
            DType::I8 => (bytes[0] as i8) as f64,
            DType::I16 => i16::from_le_bytes([bytes[0], bytes[1]]) as f64,
            DType::U16 => u16::from_le_bytes([bytes[0], bytes[1]]) as f64,
            DType::F16 => f16_bits_to_f32(u16::from_le_bytes([bytes[0], bytes[1]])) as f64,
            DType::BF16 => bf16_bits_to_f32(u16::from_le_bytes([bytes[0], bytes[1]])) as f64,
            DType::I32 => i32::from_le_bytes(bytes.try_into().unwrap()) as f64,
            DType::U32 => u32::from_le_bytes(bytes.try_into().unwrap()) as f64,
            DType::F32 => f32::from_le_bytes(bytes.try_into().unwrap()) as f64,
            DType::I64 => i64::from_le_bytes(bytes.try_into().unwrap()) as f64,
            DType::U64 => u64::from_le_bytes(bytes.try_into().unwrap()) as f64,
            DType::F64 => f64::from_le_bytes(bytes.try_into().unwrap()),
            DType::F8E4M3 | DType::F8E5M2 => {
                return Err(QError::UnsupportedDType {
                    dtype: self.as_safetensors_str().to_string(),
                    operation: "exact scalar decode".into(),
                })
            }
        })
    }

    /// Decode a contiguous run of elements. `bytes.len()` must be an exact
    /// multiple of the element width.
    ///
    /// Callers are responsible for keeping `bytes` bounded — see
    /// [`crate::budget::MemoryBudget`].
    pub fn decode_run(self, bytes: &[u8]) -> Result<Vec<f64>> {
        let w = self.size_in_bytes() as usize;
        if w == 0 || bytes.len() % w != 0 {
            return Err(QError::malformed(
                "run decode",
                format!("{} bytes is not a multiple of {w}", bytes.len()),
            ));
        }
        bytes
            .chunks_exact(w)
            .map(|c| self.decode_scalar(c))
            .collect()
    }
}

/// IEEE-754 binary16 -> binary32. Exact for all inputs including NaN payloads
/// and subnormals.
pub fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) as u32) << 31;
    let exp = ((bits >> 10) & 0x1f) as u32;
    let frac = (bits & 0x3ff) as u32;
    let out = match exp {
        0 if frac == 0 => sign,
        // Subnormal: renormalize into f32's exponent range. A subnormal f16 is
        // (frac / 2^10) * 2^-14; shifting left by `s` until bit 10 is set makes
        // it 1.m * 2^(-14 - s), so the f32 exponent field is 127 - 14 - s.
        0 => {
            let mut shifts = 0u32;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                shifts += 1;
            }
            let mantissa = (f & 0x3ff) << 13;
            sign | ((127 - 14 - shifts) << 23) | mantissa
        }
        0x1f => sign | 0x7f80_0000 | (frac << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (frac << 13),
    };
    f32::from_bits(out)
}

/// bfloat16 -> f32. bf16 *is* the high half of an f32, so this is a shift.
pub fn bf16_bits_to_f32(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtype_widths_match_safetensors() {
        assert_eq!(DType::F32.size_in_bytes(), 4);
        assert_eq!(DType::BF16.size_in_bytes(), 2);
        assert_eq!(DType::I64.size_in_bytes(), 8);
        assert_eq!(DType::Bool.size_in_bytes(), 1);
    }

    #[test]
    fn dtype_string_round_trip() {
        for d in [
            DType::Bool,
            DType::U8,
            DType::I8,
            DType::I16,
            DType::U16,
            DType::F16,
            DType::BF16,
            DType::I32,
            DType::U32,
            DType::F32,
            DType::I64,
            DType::U64,
            DType::F64,
            DType::F8E4M3,
            DType::F8E5M2,
        ] {
            assert_eq!(DType::parse_safetensors(d.as_safetensors_str()).unwrap(), d);
        }
    }

    #[test]
    fn unknown_dtype_is_rejected_not_guessed() {
        let err = DType::parse_safetensors("F4_SECRET").unwrap_err();
        assert!(matches!(err, QError::UnsupportedDType { .. }));
    }

    #[test]
    fn f32_decode_is_exact() {
        let v: f32 = 0.006408154;
        assert_eq!(
            DType::F32.decode_scalar(&v.to_le_bytes()).unwrap(),
            v as f64
        );
    }

    #[test]
    fn bf16_is_high_half_of_f32() {
        // 1.0f32 == 0x3F800000; bf16 1.0 == 0x3F80
        assert_eq!(bf16_bits_to_f32(0x3F80), 1.0);
        assert_eq!(
            DType::BF16.decode_scalar(&0x3F80u16.to_le_bytes()).unwrap(),
            1.0
        );
    }

    #[test]
    fn f16_handles_normal_subnormal_and_inf() {
        assert_eq!(f16_bits_to_f32(0x3C00), 1.0); // 1.0
        assert_eq!(f16_bits_to_f32(0xC000), -2.0); // -2.0
        assert_eq!(f16_bits_to_f32(0x0001), 2f32.powi(-24)); // smallest subnormal
        assert!(f16_bits_to_f32(0x7C00).is_infinite());
        assert!(f16_bits_to_f32(0xFC00).is_sign_negative());
    }

    #[test]
    fn fp8_refuses_rather_than_approximates() {
        assert!(!DType::F8E4M3.supports_exact_scalar_read());
        assert!(matches!(
            DType::F8E4M3.decode_scalar(&[0x42]),
            Err(QError::UnsupportedDType { .. })
        ));
    }

    #[test]
    fn decode_run_rejects_ragged_input() {
        assert!(DType::F32.decode_run(&[0u8; 7]).is_err());
        assert_eq!(DType::F32.decode_run(&[0u8; 8]).unwrap().len(), 2);
    }
}
