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
use crate::instructions::MemId;
use crate::io::{Decode, DecodeError, Encode, Wasmbin};
use crate::visit::Visit;

/// Variant of [`MemArg`] with a fixed compile-time alignment.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct AlignedMemArg<const ALIGN_LOG2: u32> {
    pub memory: MemId,
    pub offset: u32,
}

impl<const ALIGN_LOG2: u32> From<AlignedMemArg<ALIGN_LOG2>> for MemArg {
    fn from(arg: AlignedMemArg<ALIGN_LOG2>) -> MemArg {
        MemArg {
            align_log2: ALIGN_LOG2,
            memory: arg.memory,
            offset: arg.offset,
        }
    }
}

impl<const ALIGN_LOG2: u32> Encode for AlignedMemArg<ALIGN_LOG2> {
    fn encode(&self, encoder: &mut impl std::io::Write) -> std::io::Result<()> {
        MemArg::from(self.clone()).encode(encoder)
    }
}

impl<const ALIGN_LOG2: u32> Decode for AlignedMemArg<ALIGN_LOG2> {
    fn decode(decoder: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let arg = MemArg::decode(decoder)?;
        if arg.align_log2 != ALIGN_LOG2 {
            return Err(DecodeError::unsupported_discriminant::<Self>(arg.offset));
        }
        Ok(Self {
            memory: arg.memory,
            offset: arg.offset,
        })
    }
}

pub type MemArg8 = AlignedMemArg<0>;
pub type MemArg16 = AlignedMemArg<1>;
pub type MemArg32 = AlignedMemArg<2>;
pub type MemArg64 = AlignedMemArg<3>;

/// [Atomic memory instructions](https://webassembly.github.io/threads/core/binary/instructions.html#atomic-memory-instructions).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[repr(u8)]
pub enum Atomic {
    Wake(MemArg32) = 0x00,
    I32Wait(MemArg32) = 0x01,
    I64Wait(MemArg64) = 0x02,
    I32Load(MemArg32) = 0x10,
    I64Load(MemArg64) = 0x11,
    I32Load8U(MemArg8) = 0x12,
    I32Load16U(MemArg16) = 0x13,
    I64Load8U(MemArg8) = 0x14,
    I64Load16U(MemArg16) = 0x15,
    I64Load32U(MemArg32) = 0x16,
    I32Store(MemArg32) = 0x17,
    I64Store(MemArg64) = 0x18,
    I32Store8(MemArg8) = 0x19,
    I32Store16(MemArg16) = 0x1A,
    I64Store8(MemArg8) = 0x1B,
    I64Store16(MemArg16) = 0x1C,
    I64Store32(MemArg32) = 0x1D,
    I32RmwAdd(MemArg32) = 0x1E,
    I64RmwAdd(MemArg64) = 0x1F,
    I32Rmw8AddU(MemArg8) = 0x20,
    I32Rmw16AddU(MemArg16) = 0x21,
    I64Rmw8AddU(MemArg8) = 0x22,
    I64Rmw16AddU(MemArg16) = 0x23,
    I64Rmw32AddU(MemArg32) = 0x24,
    I32RmwSub(MemArg32) = 0x25,
    I64RmwSub(MemArg64) = 0x26,
    I32Rmw8SubU(MemArg8) = 0x27,
    I32Rmw16SubU(MemArg16) = 0x28,
    I64Rmw8SubU(MemArg8) = 0x29,
    I64Rmw16SubU(MemArg16) = 0x2A,
    I64Rmw32SubU(MemArg32) = 0x2B,
    I32RmwAnd(MemArg32) = 0x2C,
    I64RmwAnd(MemArg64) = 0x2D,
    I32Rmw8AndU(MemArg8) = 0x2E,
    I32Rmw16AndU(MemArg16) = 0x2F,
    I64Rmw8AndU(MemArg8) = 0x30,
    I64Rmw16AndU(MemArg16) = 0x31,
    I64Rmw32AndU(MemArg32) = 0x32,
    I32RmwOr(MemArg32) = 0x33,
    I64RmwOr(MemArg64) = 0x34,
    I32Rmw8OrU(MemArg8) = 0x35,
    I32Rmw16OrU(MemArg16) = 0x36,
    I64Rmw8OrU(MemArg8) = 0x37,
    I64Rmw16OrU(MemArg16) = 0x38,
    I64Rmw32OrU(MemArg32) = 0x39,
    I32RmwXor(MemArg32) = 0x3A,
    I64RmwXor(MemArg64) = 0x3B,
    I32Rmw8XorU(MemArg8) = 0x3C,
    I32Rmw16XorU(MemArg16) = 0x3D,
    I64Rmw8XorU(MemArg8) = 0x3E,
    I64Rmw16XorU(MemArg16) = 0x3F,
    I64Rmw32XorU(MemArg32) = 0x40,
    I32RmwXchg(MemArg32) = 0x41,
    I64RmwXchg(MemArg64) = 0x42,
    I32Rmw8XchgU(MemArg8) = 0x43,
    I32Rmw16XchgU(MemArg16) = 0x44,
    I64Rmw8XchgU(MemArg8) = 0x45,
    I64Rmw16XchgU(MemArg16) = 0x46,
    I64Rmw32XchgU(MemArg32) = 0x47,
    I32RmwCmpXchg(MemArg32) = 0x48,
    I64RmwCmpXchg(MemArg64) = 0x49,
    I32Rmw8CmpXchgU(MemArg8) = 0x4A,
    I32Rmw16CmpXchgU(MemArg16) = 0x4B,
    I64Rmw8CmpXchgU(MemArg8) = 0x4C,
    I64Rmw16CmpXchgU(MemArg16) = 0x4D,
    I64Rmw32CmpXchgU(MemArg32) = 0x4E,
}
