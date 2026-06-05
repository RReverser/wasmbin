// Copyright 2020 Google Inc. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::MemArg;
use crate::io::{Decode, DecodeError, Encode, Wasmbin};
use crate::visit::Visit;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

/// A SIMD lane index in the `0..MAX` range.
#[derive(PartialEq, Eq, Clone, Copy, Hash, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(transparent)]
pub struct LaneId<const MAX: u8>(u8);

impl<const MAX: u8> std::fmt::Debug for LaneId<MAX> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "LaneId#{}", self.0)
    }
}

impl<const MAX: u8> From<LaneId<MAX>> for u8 {
    fn from(id: LaneId<MAX>) -> u8 {
        id.0
    }
}

impl<const MAX: u8> TryFrom<u8> for LaneId<MAX> {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value < MAX {
            Ok(Self(value))
        } else {
            Err(value)
        }
    }
}

impl<const MAX: u8> Encode for LaneId<MAX> {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        self.0.encode(w)
    }
}

impl<const MAX: u8> LaneId<MAX> {
    // Private helper as don't want to commit to a public TryFrom API.
    fn decode_from(value: u8) -> Result<Self, DecodeError> {
        Self::try_from(value).map_err(DecodeError::unsupported_discriminant::<Self>)
    }
}

impl<const MAX: u8> Decode for LaneId<MAX> {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        Self::decode_from(u8::decode(r)?)
    }
}

pub type LaneId2 = LaneId<2>;
pub type LaneId4 = LaneId<4>;
pub type LaneId8 = LaneId<8>;
pub type LaneId16 = LaneId<16>;
pub type LaneId32 = LaneId<32>;

impl<const MAX: u8, const N: usize> Encode for [LaneId<MAX>; N] {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        unsafe {
            let as_ptr: *const Self = self;
            &*as_ptr.cast::<[u8; N]>()
        }
        .encode(w)
    }
}

impl<const MAX: u8, const N: usize> Decode for [LaneId<MAX>; N] {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let bytes = <[u8; N]>::decode(r)?;
        for &b in &bytes {
            <LaneId<MAX>>::decode_from(b)?;
        }
        // transmute_copy because Rust can't prove they're the same size
        Ok(unsafe { std::mem::transmute_copy::<[u8; N], [LaneId<MAX>; N]>(&bytes) })
    }
}

/// [SIMD (vector) instructions](https://webassembly.github.io/spec/core/binary/instructions.html#vector-instructions).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u32)]
pub enum SIMD {
    // Vector loads and stores
    V128Load(MemArg) = 0,
    V128Load8x8S(MemArg) = 1,
    V128Load8x8U(MemArg) = 2,
    V128Load16x4S(MemArg) = 3,
    V128Load16x4U(MemArg) = 4,
    V128Load32x2S(MemArg) = 5,
    V128Load32x2U(MemArg) = 6,
    V128Load8Splat(MemArg) = 7,
    V128Load16Splat(MemArg) = 8,
    V128Load32Splat(MemArg) = 9,
    V128Load64Splat(MemArg) = 10,
    V128Store(MemArg) = 11,
    V128Load8Lane { mem_arg: MemArg, lane_id: LaneId16 } = 84,
    V128Load16Lane { mem_arg: MemArg, lane_id: LaneId8 } = 85,
    V128Load32Lane { mem_arg: MemArg, lane_id: LaneId4 } = 86,
    V128Load64Lane { mem_arg: MemArg, lane_id: LaneId2 } = 87,
    V128Store8Lane { mem_arg: MemArg, lane_id: LaneId16 } = 88,
    V128Store16Lane { mem_arg: MemArg, lane_id: LaneId8 } = 89,
    V128Store32Lane { mem_arg: MemArg, lane_id: LaneId4 } = 90,
    V128Store64Lane { mem_arg: MemArg, lane_id: LaneId2 } = 91,
    V128Load32Zero(MemArg) = 92,
    V128Load64Zero(MemArg) = 93,

    // Const instruction for vectors
    V128Const([u8; 16]) = 12,

    // Shuffle instructions
    I8x16Shuffle([LaneId32; 16]) = 13,
    I8x16Swizzle = 14,
    I8x16RelaxedSwizzle = 256,

    // Lane instructions
    I8x16ExtractLaneS(LaneId16) = 21,
    I8x16ExtractLaneU(LaneId16) = 22,
    I8x16ReplaceLane(LaneId16) = 23,
    I16x8ExtractLaneS(LaneId8) = 24,
    I16x8ExtractLaneU(LaneId8) = 25,
    I16x8ReplaceLane(LaneId8) = 26,
    I32x4ExtractLane(LaneId4) = 27,
    I32x4ReplaceLane(LaneId4) = 28,
    I64x2ExtractLane(LaneId2) = 29,
    I64x2ReplaceLane(LaneId2) = 30,
    F32x4ExtractLane(LaneId4) = 31,
    F32x4ReplaceLane(LaneId4) = 32,
    F64x2ExtractLane(LaneId2) = 33,
    F64x2ReplaceLane(LaneId2) = 34,

    // All other vector instructions
    I8x16Splat = 15,
    I16x8Splat = 16,
    I32x4Splat = 17,
    I64x2Splat = 18,
    F32x4Splat = 19,
    F64x2Splat = 20,

    I8x16Eq = 35,
    I8x16Ne = 36,
    I8x16LtS = 37,
    I8x16LtU = 38,
    I8x16GtS = 39,
    I8x16GtU = 40,
    I8x16LeS = 41,
    I8x16LeU = 42,
    I8x16GeS = 43,
    I8x16GeU = 44,
    I16x8Eq = 45,
    I16x8Ne = 46,
    I16x8LtS = 47,
    I16x8LtU = 48,
    I16x8GtS = 49,
    I16x8GtU = 50,
    I16x8LeS = 51,
    I16x8LeU = 52,
    I16x8GeS = 53,
    I16x8GeU = 54,
    I32x4Eq = 55,
    I32x4Ne = 56,
    I32x4LtS = 57,
    I32x4LtU = 58,
    I32x4GtS = 59,
    I32x4GtU = 60,
    I32x4LeS = 61,
    I32x4LeU = 62,
    I32x4GeS = 63,
    I32x4GeU = 64,
    I64x2Eq = 214,
    I64x2Ne = 215,
    I64x2LtS = 216,
    I64x2GtS = 217,
    I64x2LeS = 218,
    I64x2GeS = 219,

    F32x4Eq = 65,
    F32x4Ne = 66,
    F32x4Lt = 67,
    F32x4Gt = 68,
    F32x4Le = 69,
    F32x4Ge = 70,
    F64x2Eq = 71,
    F64x2Ne = 72,
    F64x2Lt = 73,
    F64x2Gt = 74,
    F64x2Le = 75,
    F64x2Ge = 76,

    V128Not = 77,
    V128And = 78,
    V128Andnot = 79,
    V128Or = 80,
    V128Xor = 81,
    V128Bitselect = 82,
    V128AnyTrue = 83,

    I8x16Abs = 96,
    I8x16Neg = 97,
    I8x16Popcnt = 98,
    I8x16AllTrue = 99,
    I8x16Bitmask = 100,
    I8x16NarrowI16x8S = 101,
    I8x16NarrowI16x8U = 102,
    I8x16Shl = 107,
    I8x16ShrS = 108,
    I8x16ShrU = 109,
    I8x16Add = 110,
    I8x16AddSatS = 111,
    I8x16AddSatU = 112,
    I8x16Sub = 113,
    I8x16SubSatS = 114,
    I8x16SubSatU = 115,
    I8x16MinS = 118,
    I8x16MinU = 119,
    I8x16MaxS = 120,
    I8x16MaxU = 121,
    I8x16AvgrU = 123,

    I16x8ExtaddPairwiseI8x16S = 124,
    I16x8ExtaddPairwiseI8x16U = 125,
    I16x8Abs = 128,
    I16x8Neg = 129,
    I16x8AllTrue = 131,
    I16x8Bitmask = 132,
    I16x8NarrowI32x4S = 133,
    I16x8NarrowI32x4U = 134,
    I16x8ExtendLowI8x16S = 135,
    I16x8ExtendHighI8x16S = 136,
    I16x8ExtendLowI8x16U = 137,
    I16x8ExtendHighI8x16U = 138,
    I16x8Shl = 139,
    I16x8ShrS = 140,
    I16x8ShrU = 141,
    I16x8Q15mulrSatS = 130,
    I16x8Add = 142,
    I16x8AddSatS = 143,
    I16x8AddSatU = 144,
    I16x8Sub = 145,
    I16x8SubSatS = 146,
    I16x8SubSatU = 147,
    I16x8Mul = 149,
    I16x8MinS = 150,
    I16x8MinU = 151,
    I16x8MaxS = 152,
    I16x8MaxU = 153,
    I16x8AvgrU = 155,
    I16x8RelaxedQ15mulrS = 273,
    I16x8ExtmulLowI8x16S = 156,
    I16x8ExtmulHighI8x16S = 157,
    I16x8ExtmulLowI8x16U = 158,
    I16x8ExtmulHighI8x16U = 159,
    I16x8RelaxedDotI8x16S = 274,

    I32x4ExtaddPairwiseI16x8S = 126,
    I32x4ExtaddPairwiseI16x8U = 127,
    I32x4Abs = 160,
    I32x4Neg = 161,
    I32x4AllTrue = 163,
    I32x4Bitmask = 164,
    I32x4ExtendLowI16x8S = 167,
    I32x4ExtendHighI16x8S = 168,
    I32x4ExtendLowI16x8U = 169,
    I32x4ExtendHighI16x8U = 170,
    I32x4Shl = 171,
    I32x4ShrS = 172,
    I32x4ShrU = 173,
    I32x4Add = 174,
    I32x4Sub = 177,
    I32x4Mul = 181,
    I32x4MinS = 182,
    I32x4MinU = 183,
    I32x4MaxS = 184,
    I32x4MaxU = 185,
    I32x4DotI16x8S = 186,
    I32x4ExtmulLowI16x8S = 188,
    I32x4ExtmulHighI16x8S = 189,
    I32x4ExtmulLowI16x8U = 190,
    I32x4ExtmulHighI16x8U = 191,
    I32x4RelaxedDotAddI16x8S = 275,

    I64x2Abs = 192,
    I64x2Neg = 193,
    I64x2AllTrue = 195,
    I64x2Bitmask = 196,
    I64x2ExtendLowI32x4S = 199,
    I64x2ExtendHighI32x4S = 200,
    I64x2ExtendLowI32x4U = 201,
    I64x2ExtendHighI32x4U = 202,
    I64x2Shl = 203,
    I64x2ShrS = 204,
    I64x2ShrU = 205,
    I64x2Add = 206,
    I64x2Sub = 209,
    I64x2Mul = 213,
    I64x2ExtmulLowI32x4S = 220,
    I64x2ExtmulHighI32x4S = 221,
    I64x2ExtmulLowI32x4U = 222,
    I64x2ExtmulHighI32x4U = 223,

    F32x4Ceil = 103,
    F32x4Floor = 104,
    F32x4Trunc = 105,
    F32x4Nearest = 106,
    F32x4Abs = 224,
    F32x4Neg = 225,
    F32x4Sqrt = 227,
    F32x4Add = 228,
    F32x4Sub = 229,
    F32x4Mul = 230,
    F32x4Div = 231,
    F32x4Min = 232,
    F32x4Max = 233,
    F32x4Pmin = 234,
    F32x4Pmax = 235,
    F32x4RelaxedMin = 269,
    F32x4RelaxedMax = 270,
    F32x4RelaxedMadd = 261,
    F32x4RelaxedNmadd = 262,

    F64x2Ceil = 116,
    F64x2Floor = 117,
    F64x2Trunc = 122,
    F64x2Nearest = 148,
    F64x2Abs = 236,
    F64x2Neg = 237,
    F64x2Sqrt = 239,
    F64x2Add = 240,
    F64x2Sub = 241,
    F64x2Mul = 242,
    F64x2Div = 243,
    F64x2Min = 244,
    F64x2Max = 245,
    F64x2Pmin = 246,
    F64x2Pmax = 247,
    F64x2RelaxedMin = 271,
    F64x2RelaxedMax = 272,
    F64x2RelaxedMadd = 263,
    F64x2RelaxedNmadd = 264,
    I8x16RelaxedLaneselect = 265,
    I16x8RelaxedLaneselect = 266,
    I32x4RelaxedLaneselect = 267,
    I64x2RelaxedLaneselect = 268,

    F32x4DemoteF64x2Zero = 94,
    F64x2PromoteLowF32x4 = 95,
    I32x4TruncSatF32x4S = 248,
    I32x4TruncSatF32x4U = 249,
    F32x4ConvertI32x4S = 250,
    F32x4ConvertI32x4U = 251,
    I32x4TruncSatF64x2SZero = 252,
    I32x4TruncSatF64x2UZero = 253,
    F64x2ConvertLowI32x4S = 254,
    F64x2ConvertLowI32x4U = 255,
    I32x4RelaxedTruncSF32x4 = 257,
    I32x4RelaxedTruncUF32x4 = 258,
    I32x4RelaxedTruncSZeroF64x2 = 259,
    I32x4RelaxedTruncUZeroF64x2 = 260,
}
