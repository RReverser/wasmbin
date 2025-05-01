//! [Specification types](https://webassembly.github.io/spec/core/binary/types.html).

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

use crate::builtins::WasmbinCountable;
use crate::indices::TypeId;
use crate::io::{
    encode_decode_as, Decode, DecodeError, DecodeWithDiscriminant, Encode, PathItem, Wasmbin,
};
use crate::visit::Visit;
use std::convert::TryFrom;
use std::fmt::{self, Debug, Formatter};

const OP_CODE_EMPTY_BLOCK: u8 = 0x40;

/// [Value type](https://webassembly.github.io/spec/core/binary/types.html#value-types).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[repr(u8)]
pub enum ValueType {
    /// [SIMD vector type](https://webassembly.github.io/spec/core/binary/types.html#vector-types).
    V128 = 0x7B,
    F64 = 0x7C,
    F32 = 0x7D,
    I64 = 0x7E,
    I32 = 0x7F,
    /// [Reference type](https://webassembly.github.io/spec/core/binary/types.html#reference-types).
    Ref(RefType),
}

/// [Block type](https://webassembly.github.io/spec/core/binary/instructions.html#control-instructions).
#[derive(Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[repr(u8)]
pub enum BlockType {
    /// Block without a return value.
    Empty,
    /// Block with a single return value.
    Value(ValueType),
    /// Block returning multiple values.
    ///
    /// The actual list of value types is stored as a function signature in the type section
    /// and referenced here by its ID.
    MultiValue(TypeId),
}

impl Encode for BlockType {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        match self {
            BlockType::Empty => OP_CODE_EMPTY_BLOCK.encode(w),
            BlockType::Value(ty) => ty.encode(w),
            BlockType::MultiValue(id) => i64::from(id.index).encode(w),
        }
    }
}

impl Decode for BlockType {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let discriminant = u8::decode(r)?;
        if discriminant == OP_CODE_EMPTY_BLOCK {
            return Ok(BlockType::Empty);
        }
        if let Some(ty) = ValueType::maybe_decode_with_discriminant(discriminant, r)
            .map_err(|err| err.in_path(PathItem::Variant("BlockType::Value")))?
        {
            return Ok(BlockType::Value(ty));
        }
        let index = (move || -> Result<_, DecodeError> {
            // We have already read one byte that could've been either a
            // discriminant or a part of an s33 LEB128 specially used for
            // type indices.
            //
            // To recover the LEB128 sequence, we need to chain it back.
            let buf = [discriminant];
            let mut r = std::io::Read::chain(&buf[..], r);
            let as_i64 = i64::decode(&mut r)?;
            // These indices are encoded as positive signed integers.
            // Convert them to unsigned integers and error out if they're out of range.
            let index = u32::try_from(as_i64)?;
            Ok(index)
        })()
        .map_err(|err| err.in_path(PathItem::Variant("BlockType::MultiValue")))?;
        Ok(BlockType::MultiValue(TypeId { index }))
    }
}

/// [Function type](https://webassembly.github.io/spec/core/binary/types.html#function-types).
#[derive(Wasmbin, WasmbinCountable, PartialEq, Eq, Hash, Clone, Visit)]
#[wasmbin(discriminant = 0x60)]
pub struct FuncType {
    pub params: Vec<ValueType>,
    pub results: Vec<ValueType>,
}

impl Debug for FuncType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fn encode_types(types: &[ValueType], f: &mut Formatter) -> fmt::Result {
            f.write_str("(")?;
            for (i, ty) in types.iter().enumerate() {
                if i != 0 {
                    f.write_str(", ")?;
                }
                ty.fmt(f)?;
            }
            f.write_str(")")
        }

        encode_types(&self.params, f)?;
        f.write_str(" -> ")?;
        encode_types(&self.results, f)
    }
}

/// [Limits](https://webassembly.github.io/spec/core/binary/types.html#limits) type.
#[derive(PartialEq, Eq, Hash, Clone, Visit)]
pub struct Limits {
    pub min: u32,
    pub max: Option<u32>,
}

impl Debug for Limits {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}..", self.min)?;
        if let Some(max) = self.max {
            write!(f, "={max}")?;
        }
        Ok(())
    }
}

#[derive(Wasmbin)]
#[repr(u8)]
enum LimitsRepr {
    Min { min: u32 } = 0x00,
    MinMax { min: u32, max: u32 } = 0x01,
}

encode_decode_as!(Limits, {
    (Limits { min, max: None }) <=> (LimitsRepr::Min { min }),
    (Limits { min, max: Some(max) }) <=> (LimitsRepr::MinMax { min, max }),
});

#[cfg(any(feature = "threads", feature = "custom-page-sizes"))]
#[derive(Wasmbin)]
#[repr(u8)]
enum MemTypeRepr {
    Unshared(LimitsRepr),
    #[cfg(feature = "threads")]
    SharedMin {
        min: u32,
    } = 0x02,
    #[cfg(feature = "threads")]
    SharedMinMax {
        min: u32,
        max: u32,
    } = 0x03,
    #[cfg(feature = "custom-page-sizes")]
    UnsharedMinCustom {
        min: u32,
        page_size: PageSize,
    } = 0x08,
    #[cfg(feature = "custom-page-sizes")]
    UnsharedMinMaxCustom {
        min: u32,
        max: u32,
        page_size: PageSize,
    } = 0x09,
    #[cfg(all(feature = "threads", feature = "custom-page-sizes"))]
    SharedMinCustom {
        min: u32,
        page_size: PageSize,
    } = 0x0A,
    #[cfg(all(feature = "threads", feature = "custom-page-sizes"))]
    SharedMinMaxCustom {
        min: u32,
        max: u32,
        page_size: PageSize,
    } = 0x0B,
}

#[cfg(feature = "custom-page-sizes")]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Visit)]
pub struct PageSize(u32);

#[cfg(feature = "custom-page-sizes")]
impl PageSize {
    /// Minimum supported page size (pagesize 1)
    pub const MIN: Self = Self::new(0).unwrap();

    /// Default webassembly page size (pagesize 65536)
    pub const DEFAULT: Self = Self::new(16).unwrap();

    /// Returns a custom page size that is valid according to the spec
    pub const fn new(size_log2: u32) -> Option<Self> {
        if size_log2 <= 64 {
            Some(Self(size_log2))
        } else {
            None
        }
    }

    pub const fn size_log2(&self) -> u32 {
        self.0
    }

    /// Returns human-readable page size as bytes
    pub const fn size(&self) -> u64 {
        u64::pow(2, self.0)
    }
}

#[cfg(feature = "custom-page-sizes")]
impl Encode for PageSize {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        self.0.encode(w)
    }
}

#[cfg(feature = "custom-page-sizes")]
impl Decode for PageSize {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        u32::decode(r).and_then(|x| {
            PageSize::new(x).ok_or(DecodeError::unsupported_discriminant::<PageSize>(x))
        })
    }
}

/// [Memory type](https://webassembly.github.io/spec/core/binary/types.html#memory-types).
#[cfg_attr(
    all(not(feature = "threads"), not(feature = "custom-page-sizes")),
    derive(Wasmbin)
)]
#[derive(WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct MemType {
    #[cfg(feature = "custom-page-sizes")]
    pub page_size: Option<PageSize>,
    #[cfg(feature = "threads")]
    pub is_shared: bool,
    pub limits: Limits,
}

#[cfg(all(feature = "threads", not(feature = "custom-page-sizes")))]
encode_decode_as!(MemType, {
    (MemType { is_shared: false, limits: Limits { min, max: None } }) <=> (MemTypeRepr::Unshared(LimitsRepr::Min { min })),
    (MemType { is_shared: false, limits: Limits { min, max: Some(max) } }) <=> (MemTypeRepr::Unshared(LimitsRepr::MinMax { min, max })),
    (MemType { is_shared: true, limits: Limits { min, max: None } }) <=> (MemTypeRepr::SharedMin { min }),
    (MemType { is_shared: true, limits: Limits { min, max: Some(max) } }) <=> (MemTypeRepr::SharedMinMax { min, max }),
});

#[cfg(all(not(feature = "threads"), feature = "custom-page-sizes"))]
encode_decode_as!(MemType, {
    (MemType { page_size: None, limits: Limits { min, max: None } }) <=> (MemTypeRepr::Unshared(LimitsRepr::Min { min })),
    (MemType { page_size: None, limits: Limits { min, max: Some(max) } }) <=> (MemTypeRepr::Unshared(LimitsRepr::MinMax { min, max })),
    (MemType { page_size: Some(page_size), limits: Limits { min, max: None } }) <=> (MemTypeRepr::UnsharedMinCustom { min, page_size }),
    (MemType { page_size: Some(page_size), limits: Limits { min, max: Some(max) } }) <=> (MemTypeRepr::UnsharedMinMaxCustom { min, max, page_size }),
});

#[cfg(all(feature = "threads", feature = "custom-page-sizes"))]
encode_decode_as!(MemType, {
    (MemType { is_shared: false, page_size: None, limits: Limits { min, max: None } })
        <=> (MemTypeRepr::Unshared(LimitsRepr::Min { min })),
    (MemType { is_shared: false, page_size: None, limits: Limits { min, max: Some(max) } })
        <=> (MemTypeRepr::Unshared(LimitsRepr::MinMax { min, max })),
    (MemType { is_shared: true, page_size: None, limits: Limits { min, max: None } })
        <=> (MemTypeRepr::SharedMin { min }),
    (MemType { is_shared: true, page_size: None, limits: Limits { min, max: Some(max) } })
        <=> (MemTypeRepr::SharedMinMax { min, max }),
    (MemType { is_shared: false, page_size: Some(page_size), limits: Limits { min, max: None } })
        <=> (MemTypeRepr::UnsharedMinCustom { min, page_size }),
    (MemType { is_shared: false, page_size: Some(page_size), limits: Limits { min, max: Some(max) } })
        <=> (MemTypeRepr::UnsharedMinMaxCustom { min, max, page_size }),
    (MemType { is_shared: true, page_size: Some(page_size), limits: Limits { min, max: None } })
        <=> (MemTypeRepr::SharedMinCustom { min, page_size }),
    (MemType { is_shared: true, page_size: Some(page_size), limits: Limits { min, max: Some(max) } })
        <=> (MemTypeRepr::SharedMinMaxCustom { min, max, page_size }),
});

/// [Reference type](https://webassembly.github.io/spec/core/binary/types.html#reference-types).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[repr(u8)]
pub enum RefType {
    Func = 0x70,
    Extern = 0x6F,
    #[cfg(feature = "exception-handling")]
    Exception = 0x69,
}

/// [Table type](https://webassembly.github.io/spec/core/binary/types.html#table-types).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct TableType {
    pub elem_type: RefType,
    pub limits: Limits,
}

/// [Global type](https://webassembly.github.io/spec/core/binary/types.html#global-types).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
pub struct GlobalType {
    pub value_type: ValueType,
    pub mutable: bool,
}

/// [Exception tag type](https://webassembly.github.io/exception-handling/core/binary/types.html#tag-types).
#[cfg(feature = "exception-handling")]
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[wasmbin(discriminant = 0x00)]
pub struct ExceptionType {
    pub func_type: TypeId,
}
