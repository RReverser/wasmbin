//! WebAssembly [instruction set](https://webassembly.github.io/exception-handling/core/binary/instructions.html).

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

use crate::builtins::FloatConst;
use crate::indices::{FuncId, GlobalId, LabelId, LocalId, MemId, TableId, TypeId};
use crate::io::{Decode, DecodeError, DecodeWithDiscriminant, Encode, PathItem, Wasmbin};
use crate::types::{BlockType, RefType, ValueType};
use crate::visit::Visit;
use thiserror::Error;

const OP_CODE_BLOCK_START: u8 = 0x02;
const OP_CODE_LOOP_START: u8 = 0x03;
const OP_CODE_IF_START: u8 = 0x04;
const OP_CODE_END: u8 = 0x0B;

#[derive(Debug, Error)]
#[error("Mismatched block depth")]
struct DepthError;

impl From<DepthError> for std::io::Error {
    fn from(err: DepthError) -> Self {
        Self::new(std::io::ErrorKind::InvalidData, err)
    }
}

#[derive(Default)]
struct DepthTracker {
    depth: u32,
}

impl DepthTracker {
    fn inc(&mut self) {
        self.depth += 1;
    }

    // Returns a bool indicating whether to continue, or an error state.
    fn try_dec(&mut self) -> Result<(), DepthError> {
        self.depth = self.depth.checked_sub(1).ok_or(DepthError)?;
        Ok(())
    }

    fn assert_end(self) -> Result<(), DepthError> {
        match self.depth {
            0 => Ok(()),
            _ => Err(DepthError),
        }
    }
}

impl Encode for [Instruction] {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        let mut depth_tracker = DepthTracker::default();
        for instr in self {
            match instr {
                Instruction::BlockStart(_)
                | Instruction::LoopStart(_)
                | Instruction::IfStart(_) => {
                    depth_tracker.inc();
                }
                Instruction::End => {
                    depth_tracker.try_dec()?;
                }
                _ => {}
            }
            instr.encode(w)?;
        }
        depth_tracker.assert_end()?;
        OP_CODE_END.encode(w)
    }
}

impl Decode for Vec<Instruction> {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let mut res = Vec::new();
        let mut depth_tracker = DepthTracker::default();
        loop {
            let op_code = u8::decode(r)?;
            match op_code {
                OP_CODE_BLOCK_START | OP_CODE_LOOP_START | OP_CODE_IF_START => {
                    depth_tracker.inc();
                }
                OP_CODE_END => {
                    if depth_tracker.try_dec().is_err() {
                        break;
                    }
                }
                _ => {}
            }
            let i = res.len();
            res.push(
                Instruction::decode_with_discriminant(op_code, r)
                    .map_err(move |err| err.in_path(PathItem::Index(i)))?,
            );
        }
        Ok(res)
    }
}

/// [Expression](https://webassembly.github.io/spec/core/binary/instructions.html#expressions), aka a terminated list of [instructions](Instruction).
pub type Expression = Vec<Instruction>;

impl crate::builtins::WasmbinCountable for Expression {}

/// [Memory immediate argument](https://webassembly.github.io/spec/core/binary/instructions.html#memory-instructions).
#[derive(Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct MemArg {
    pub align_log2: u32,
    pub memory: MemId,
    pub offset: u32,
}

const MULTI_MEMORY_FLAG: u32 = 1 << 6;

impl Encode for MemArg {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        if self.memory.index != 0 {
            (self.align_log2 | MULTI_MEMORY_FLAG).encode(w)?;
            self.memory.encode(w)?;
        } else {
            self.align_log2.encode(w)?;
        }
        self.offset.encode(w)
    }
}

impl Decode for MemArg {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let mut align_log2 = u32::decode(r)?;
        let memory = if align_log2 & MULTI_MEMORY_FLAG != 0 {
            align_log2 &= !MULTI_MEMORY_FLAG;
            MemId::decode(r)?
        } else {
            MemId::from(0)
        };
        Ok(Self {
            align_log2,
            memory,
            offset: u32::decode(r)?,
        })
    }
}

/// An [indirect call](https://webassembly.github.io/spec/core/binary/instructions.html#control-instructions).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct CallIndirect {
    pub ty: TypeId,
    pub table: TableId,
}

/// WebAssembly [instruction set](https://webassembly.github.io/spec/core/binary/instructions.html).
///
/// In most cases, these will map 1:1 to the instructions in the spec, but an exception is made for
/// structured control flow instructions (`block`, `loop` and `if`). Representing those as nested
/// blocks would be ideal semantically, but is very expensive and tends to blow up the stack for even
/// moderately-sized modules. Instead, we follow the other WebAssembly parsers and represent them as
/// a start (`BlockStart`, `LoopStart` or `IfStart`) instruction followed by the contents of the block,
/// and an `End` instruction - all in the same flat instruction list.
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[repr(u8)]
pub enum Instruction {
    Unreachable = 0x00,
    Nop = 0x01,
    BlockStart(BlockType) = OP_CODE_BLOCK_START,
    LoopStart(BlockType) = OP_CODE_LOOP_START,
    IfStart(BlockType) = OP_CODE_IF_START,
    IfElse = 0x05,
    #[cfg(feature = "exception-handling")]
    Throw(crate::indices::ExceptionId) = 0x08,
    #[cfg(feature = "exception-handling")]
    ThrowRef = 0x0A,
    End = OP_CODE_END,
    Br(LabelId) = 0x0C,
    BrIf(LabelId) = 0x0D,
    BrTable {
        branches: Vec<LabelId>,
        otherwise: LabelId,
    } = 0x0E,
    Return = 0x0F,
    Call(FuncId) = 0x10,
    CallIndirect(CallIndirect) = 0x11,
    ReturnCall(FuncId) = 0x12,
    ReturnCallIndirect(CallIndirect) = 0x13,
    Drop = 0x1A,
    Select = 0x1B,
    SelectWithTypes(Vec<ValueType>) = 0x1C,
    #[cfg(feature = "exception-handling")]
    TryTable(TryTable) = 0x1F,
    LocalGet(LocalId) = 0x20,
    LocalSet(LocalId) = 0x21,
    LocalTee(LocalId) = 0x22,
    GlobalGet(GlobalId) = 0x23,
    GlobalSet(GlobalId) = 0x24,
    TableGet(TableId) = 0x25,
    TableSet(TableId) = 0x26,
    I32Load(MemArg) = 0x28,
    I64Load(MemArg) = 0x29,
    F32Load(MemArg) = 0x2A,
    F64Load(MemArg) = 0x2B,
    I32Load8S(MemArg) = 0x2C,
    I32Load8U(MemArg) = 0x2D,
    I32Load16S(MemArg) = 0x2E,
    I32Load16U(MemArg) = 0x2F,
    I64Load8S(MemArg) = 0x30,
    I64Load8U(MemArg) = 0x31,
    I64Load16S(MemArg) = 0x32,
    I64Load16U(MemArg) = 0x33,
    I64Load32S(MemArg) = 0x34,
    I64Load32U(MemArg) = 0x35,
    I32Store(MemArg) = 0x36,
    I64Store(MemArg) = 0x37,
    F32Store(MemArg) = 0x38,
    F64Store(MemArg) = 0x39,
    I32Store8(MemArg) = 0x3A,
    I32Store16(MemArg) = 0x3B,
    I64Store8(MemArg) = 0x3C,
    I64Store16(MemArg) = 0x3D,
    I64Store32(MemArg) = 0x3E,
    MemorySize(MemId) = 0x3F,
    MemoryGrow(MemId) = 0x40,
    I32Const(i32) = 0x41,
    I64Const(i64) = 0x42,
    F32Const(FloatConst<f32>) = 0x43,
    F64Const(FloatConst<f64>) = 0x44,
    I32Eqz = 0x45,
    I32Eq = 0x46,
    I32Ne = 0x47,
    I32LtS = 0x48,
    I32LtU = 0x49,
    I32GtS = 0x4A,
    I32GtU = 0x4B,
    I32LeS = 0x4C,
    I32LeU = 0x4D,
    I32GeS = 0x4E,
    I32GeU = 0x4F,
    I64Eqz = 0x50,
    I64Eq = 0x51,
    I64Ne = 0x52,
    I64LtS = 0x53,
    I64LtU = 0x54,
    I64GtS = 0x55,
    I64GtU = 0x56,
    I64LeS = 0x57,
    I64LeU = 0x58,
    I64GeS = 0x59,
    I64GeU = 0x5A,
    F32Eq = 0x5B,
    F32Ne = 0x5C,
    F32Lt = 0x5D,
    F32Gt = 0x5E,
    F32Le = 0x5F,
    F32Ge = 0x60,
    F64Eq = 0x61,
    F64Ne = 0x62,
    F64Lt = 0x63,
    F64Gt = 0x64,
    F64Le = 0x65,
    F64Ge = 0x66,
    I32Clz = 0x67,
    I32Ctz = 0x68,
    I32PopCnt = 0x69,
    I32Add = 0x6A,
    I32Sub = 0x6B,
    I32Mul = 0x6C,
    I32DivS = 0x6D,
    I32DivU = 0x6E,
    I32RemS = 0x6F,
    I32RemU = 0x70,
    I32And = 0x71,
    I32Or = 0x72,
    I32Xor = 0x73,
    I32Shl = 0x74,
    I32ShrS = 0x75,
    I32ShrU = 0x76,
    I32RotL = 0x77,
    I32RotR = 0x78,
    I64Clz = 0x79,
    I64Ctz = 0x7A,
    I64PopCnt = 0x7B,
    I64Add = 0x7C,
    I64Sub = 0x7D,
    I64Mul = 0x7E,
    I64DivS = 0x7F,
    I64DivU = 0x80,
    I64RemS = 0x81,
    I64RemU = 0x82,
    I64And = 0x83,
    I64Or = 0x84,
    I64Xor = 0x85,
    I64Shl = 0x86,
    I64ShrS = 0x87,
    I64ShrU = 0x88,
    I64RotL = 0x89,
    I64RotR = 0x8A,
    F32Abs = 0x8B,
    F32Neg = 0x8C,
    F32Ceil = 0x8D,
    F32Floor = 0x8E,
    F32Trunc = 0x8F,
    F32Nearest = 0x90,
    F32Sqrt = 0x91,
    F32Add = 0x92,
    F32Sub = 0x93,
    F32Mul = 0x94,
    F32Div = 0x95,
    F32Min = 0x96,
    F32Max = 0x97,
    F32CopySign = 0x98,
    F64Abs = 0x99,
    F64Neg = 0x9A,
    F64Ceil = 0x9B,
    F64Floor = 0x9C,
    F64Trunc = 0x9D,
    F64Nearest = 0x9E,
    F64Sqrt = 0x9F,
    F64Add = 0xA0,
    F64Sub = 0xA1,
    F64Mul = 0xA2,
    F64Div = 0xA3,
    F64Min = 0xA4,
    F64Max = 0xA5,
    F64CopySign = 0xA6,
    I32WrapI64 = 0xA7,
    I32TruncF32S = 0xA8,
    I32TruncF332U = 0xA9,
    I32TruncF64S = 0xAA,
    I32TruncF64U = 0xAB,
    I64ExtendI32S = 0xAC,
    I64ExtendI32U = 0xAD,
    I64TruncF32S = 0xAE,
    I64TruncF32U = 0xAF,
    I64TruncF64S = 0xB0,
    I64TruncF64U = 0xB1,
    F32ConvertI32S = 0xB2,
    F32ConvertI32U = 0xB3,
    F32ConvertI64S = 0xB4,
    F32ConvertI64U = 0xB5,
    F32DemoteF64 = 0xB6,
    F64ConvertI32S = 0xB7,
    F64ConvertI32U = 0xB8,
    F64ConvertI64S = 0xB9,
    F64ConvertI64U = 0xBA,
    F64PromoteF32 = 0xBB,
    I32ReinterpretF32 = 0xBC,
    I64ReinterpretF64 = 0xBD,
    F32ReinterpretI32 = 0xBE,
    F64ReinterpretI64 = 0xBF,
    I32Extend8S = 0xC0,
    I32Extend16S = 0xC1,
    I64Extend8S = 0xC2,
    I64Extend16S = 0xC3,
    I64Extend32S = 0xC4,
    RefNull(RefType) = 0xD0,
    RefIsNull = 0xD1,
    RefFunc(FuncId) = 0xD2,
    Misc(Misc) = 0xFC,
    SIMD(SIMD) = 0xFD,
    #[cfg(feature = "threads")]
    Atomic(Atomic) = 0xFE,
}

mod misc;
pub use misc::Misc;

pub mod simd;
pub use simd::SIMD;

#[cfg(feature = "threads")]
pub mod threads;
#[cfg(feature = "threads")]
pub use threads::Atomic;

#[cfg(feature = "exception-handling")]
pub mod exceptions;
#[cfg(feature = "exception-handling")]
pub use exceptions::{Catch, TryTable};
