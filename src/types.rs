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
    Decode, DecodeError, DecodeWithDiscriminant, Encode, PathItem, Wasmbin, encode_decode_as,
};
use crate::visit::Visit;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt::{self, Debug, Formatter};

const OP_CODE_EMPTY_BLOCK: u8 = 0x40;

/// [Value type](https://webassembly.github.io/spec/core/binary/types.html#value-types).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum ValueType {
    /// [Vector types](https://webassembly.github.io/spec/core/binary/types.html#binary-vectype).
    V128 = 0x7B,
    // [Number types](https://webassembly.github.io/spec/core/binary/types.html#binary-numtype).
    F64 = 0x7C,
    F32 = 0x7D,
    I64 = 0x7E,
    I32 = 0x7F,
    /// [Reference type](https://webassembly.github.io/spec/core/binary/types.html#binary-reftype).
    Ref(RefType),
}

/// [Block type](https://webassembly.github.io/spec/core/binary/instructions.html#control-instructions).
#[derive(Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Limits {
    pub min: u64,
    pub max: Option<u64>,
    pub is_64: bool,
    #[cfg(feature = "threads")]
    pub is_shared: bool,
    #[cfg(feature = "custom-page-sizes")]
    pub page_size: Option<PageSize>,
}

impl Debug for Limits {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}..", self.min)?;
        if let Some(max) = self.max {
            write!(f, "={max}")?;
        }
        if self.is_64 {
            write!(f, "_u64")?;
        }
        #[cfg(feature = "threads")]
        if self.is_shared {
            write!(f, " (shared)")?;
        }
        #[cfg(feature = "custom-page-sizes")]
        if let Some(page_size) = self.page_size {
            write!(f, " (page size: {page_size:?})")?;
        }
        Ok(())
    }
}

#[derive(Wasmbin)]
#[repr(u8)]
enum LimitsRepr {
    Min {
        min: u64,
    } = 0x00,
    MinMax {
        min: u64,
        max: u64,
    } = 0x01,
    #[cfg(feature = "threads")]
    MinShared {
        min: u64,
    } = 0x02,
    #[cfg(feature = "threads")]
    MinMaxShared {
        min: u64,
        max: u64,
    } = 0x03,
    Min64 {
        min: u64,
    } = 0x04,
    MinMax64 {
        min: u64,
        max: u64,
    } = 0x05,
    #[cfg(feature = "threads")]
    Min64Shared {
        min: u64,
    } = 0x06,
    #[cfg(feature = "threads")]
    MinMax64Shared {
        min: u64,
        max: u64,
    } = 0x07,
    #[cfg(feature = "custom-page-sizes")]
    MinCustom {
        min: u64,
        page_size: PageSize,
    } = 0x08,
    #[cfg(feature = "custom-page-sizes")]
    MinMaxCustom {
        min: u64,
        max: u64,
        page_size: PageSize,
    } = 0x09,
    #[cfg(all(feature = "custom-page-sizes", feature = "threads"))]
    MinSharedCustom {
        min: u64,
        page_size: PageSize,
    } = 0x0A,
    #[cfg(all(feature = "custom-page-sizes", feature = "threads"))]
    MinMaxSharedCustom {
        min: u64,
        max: u64,
        page_size: PageSize,
    } = 0x0B,
    #[cfg(feature = "custom-page-sizes")]
    Min64Custom {
        min: u64,
        page_size: PageSize,
    } = 0x0C,
    #[cfg(feature = "custom-page-sizes")]
    MinMax64Custom {
        min: u64,
        max: u64,
        page_size: PageSize,
    } = 0x0D,
    #[cfg(all(feature = "custom-page-sizes", feature = "threads"))]
    Min64SharedCustom {
        min: u64,
        page_size: PageSize,
    } = 0x0E,
    #[cfg(all(feature = "custom-page-sizes", feature = "threads"))]
    MinMax64SharedCustom {
        min: u64,
        max: u64,
        page_size: PageSize,
    } = 0x0F,
}

#[cfg(all(not(feature = "threads"), not(feature = "custom-page-sizes")))]
encode_decode_as!(Limits, {
    (Limits { min, max: None, is_64: false }) <=> (LimitsRepr::Min { min }),
    (Limits { min, max: Some(max), is_64: false }) <=> (LimitsRepr::MinMax { min, max }),
    (Limits { min, max: None, is_64: true }) <=> (LimitsRepr::Min64 { min }),
    (Limits { min, max: Some(max), is_64: true }) <=> (LimitsRepr::MinMax64 { min, max }),
});

#[cfg(all(feature = "threads", not(feature = "custom-page-sizes")))]
encode_decode_as!(Limits, {
    (Limits { min, max: None, is_64: false, is_shared: false }) <=> (LimitsRepr::Min { min }),
    (Limits { min, max: Some(max), is_64: false, is_shared: false }) <=> (LimitsRepr::MinMax { min, max }),
    (Limits { min, max: None, is_64: true, is_shared: false }) <=> (LimitsRepr::Min64 { min }),
    (Limits { min, max: Some(max), is_64: true, is_shared: false }) <=> (LimitsRepr::MinMax64 { min, max }),
    (Limits { min, max: None, is_64: false, is_shared: true }) <=> (LimitsRepr::MinShared { min }),
    (Limits { min, max: Some(max), is_64: false, is_shared: true }) <=> (LimitsRepr::MinMaxShared { min, max }),
    (Limits { min, max: None, is_64: true, is_shared: true }) <=> (LimitsRepr::Min64Shared { min }),
    (Limits { min, max: Some(max), is_64: true, is_shared: true }) <=> (LimitsRepr::MinMax64Shared { min, max }),
});

#[cfg(all(feature = "custom-page-sizes", not(feature = "threads")))]
encode_decode_as!(Limits, {
    (Limits { min, max: None, is_64: false, page_size: None }) <=> (LimitsRepr::Min { min }),
    (Limits { min, max: Some(max), is_64: false, page_size: None }) <=> (LimitsRepr::MinMax { min, max }),
    (Limits { min, max: None, is_64: true, page_size: None }) <=> (LimitsRepr::Min64 { min }),
    (Limits { min, max: Some(max), is_64: true, page_size: None }) <=> (LimitsRepr::MinMax64 { min, max }),
    (Limits { min, max: None, is_64: false, page_size: Some(page_size) }) <=> (LimitsRepr::MinCustom { min, page_size }),
    (Limits { min, max: Some(max), is_64: false, page_size: Some(page_size) }) <=> (LimitsRepr::MinMaxCustom { min, max, page_size }),
    (Limits { min, max: None, is_64: true, page_size: Some(page_size) }) <=> (LimitsRepr::Min64Custom { min, page_size }),
    (Limits { min, max: Some(max), is_64: true, page_size: Some(page_size) }) <=> (LimitsRepr::MinMax64Custom { min, max, page_size }),
});

#[cfg(all(feature = "custom-page-sizes", feature = "threads"))]
encode_decode_as!(Limits, {
    (Limits { min, max: None, is_64: false, is_shared: false, page_size: None }) <=> (LimitsRepr::Min { min }),
    (Limits { min, max: Some(max), is_64: false, is_shared: false, page_size: None }) <=> (LimitsRepr::MinMax { min, max }),
    (Limits { min, max: None, is_64: true, is_shared: false, page_size: None }) <=> (LimitsRepr::Min64 { min }),
    (Limits { min, max: Some(max), is_64: true, is_shared: false, page_size: None }) <=> (LimitsRepr::MinMax64 { min, max }),
    (Limits { min, max: None, is_64: false, is_shared: true, page_size: None }) <=> (LimitsRepr::MinShared { min }),
    (Limits { min, max: Some(max), is_64: false, is_shared: true, page_size: None }) <=> (LimitsRepr::MinMaxShared { min, max }),
    (Limits { min, max: None, is_64: true, is_shared: true, page_size: None }) <=> (LimitsRepr::Min64Shared { min }),
    (Limits { min, max: Some(max), is_64: true, is_shared: true, page_size: None }) <=> (LimitsRepr::MinMax64Shared { min, max }),
    (Limits { min, max: None, is_64: false, is_shared: false, page_size: Some(page_size) }) <=> (LimitsRepr::MinCustom { min, page_size }),
    (Limits { min, max: Some(max), is_64: false, is_shared: false, page_size: Some(page_size) }) <=> (LimitsRepr::MinMaxCustom { min, max, page_size }),
    (Limits { min, max: None, is_64: true, is_shared: false, page_size: Some(page_size) }) <=> (LimitsRepr::Min64Custom { min, page_size }),
    (Limits { min, max: Some(max), is_64: true, is_shared: false, page_size: Some(page_size) }) <=> (LimitsRepr::MinMax64Custom { min, max, page_size }),
    (Limits { min, max: None, is_64: false, is_shared: true, page_size: Some(page_size) }) <=> (LimitsRepr::MinSharedCustom { min, page_size }),
    (Limits { min, max: Some(max), is_64: false, is_shared: true, page_size: Some(page_size) }) <=> (LimitsRepr::MinMaxSharedCustom { min, max, page_size }),
    (Limits { min, max: None, is_64: true, is_shared: true, page_size: Some(page_size) }) <=> (LimitsRepr::Min64SharedCustom { min, page_size }),
    (Limits { min, max: Some(max), is_64: true, is_shared: true, page_size: Some(page_size) }) <=> (LimitsRepr::MinMax64SharedCustom { min, max, page_size }),
});

#[cfg(feature = "custom-page-sizes")]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
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
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemType {
    pub limits: Limits,
}

/// [Reference type](https://webassembly.github.io/spec/core/binary/types.html#reference-types).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum RefType {
    NullableHeapType(HeapType) = 0x63,
    HeapType(HeapType) = 0x64,
    AbstractHeapType(AbstractHeapType),
}

/// [Heap type](https://webassembly.github.io/spec/core/binary/types.html#heap-types).
#[derive(Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum HeapType {
    Abstract(AbstractHeapType),
    TypeIndex(TypeId),
}

impl Encode for HeapType {
    fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        match self {
            HeapType::Abstract(abs_ty) => abs_ty.encode(w),
            // Heap type indices are encoded as s33
            HeapType::TypeIndex(type_id) => i64::from(type_id.index).encode(w),
        }
    }
}

impl Decode for HeapType {
    fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
        let item = u8::decode(r)?;
        // First try to decode as an abstract heap type.
        if let Some(abs_ty) = AbstractHeapType::maybe_decode_with_discriminant(item, r)
            .map_err(|err| err.in_path(PathItem::Variant("HeapType::Abstract")))?
        {
            return Ok(HeapType::Abstract(abs_ty));
        }
        // If it wasn't an abstract heap type, decode as a type index (s33).
        let buf = [item];
        let mut r = std::io::Read::chain(&buf[..], r);
        let as_i64 = i64::decode(&mut r)
            .map_err(|err| err.in_path(PathItem::Variant("HeapType::TypeIndex")))?;
        // These indices are encoded as positive signed integers.
        // Convert them to unsigned integers and error out if they're out of range.
        let index = u32::try_from(as_i64)?;
        Ok(HeapType::TypeIndex(index.into()))
    }
}

/// [Abstract heap type](https://webassembly.github.io/spec/core/binary/types.html#binary-absheaptype).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum AbstractHeapType {
    Exception = 0x69,
    Array = 0x6A,
    Struct = 0x6B,
    I31 = 0x6C,
    Eq = 0x6D,
    Any = 0x6E,
    Extern = 0x6F,
    Func = 0x70,
    None = 0x71,
    NoExtern = 0x72,
    NoFunc = 0x73,
    NoException = 0x74,
}

/// [Table type](https://webassembly.github.io/spec/core/binary/types.html#table-types).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableType {
    pub elem_type: RefType,
    pub limits: Limits,
}

/// [Global type](https://webassembly.github.io/spec/core/binary/types.html#global-types).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GlobalType {
    pub value_type: ValueType,
    pub mutable: bool,
}

/// [Exception tag type](https://webassembly.github.io/exception-handling/core/binary/types.html#tag-types).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[wasmbin(discriminant = 0x00)]
pub struct ExceptionType {
    pub func_type: TypeId,
}

/// [Recursive type](https://webassembly.github.io/spec/core/binary/types.html#binary-rectype).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum RecursiveType {
    SubTypes(Vec<SubType>) = 0x4E,
    SubType(SubType),
}

/// Fields of a [sub type](https://webassembly.github.io/spec/core/binary/types.html#binary-subtype).
#[derive(Wasmbin, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubTypeData {
    pub super_types: Vec<TypeId>,
    pub composite_type: CompositeType,
}

/// [Sub type](https://webassembly.github.io/spec/core/binary/types.html#binary-subtype).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum SubType {
    FinalSubType(SubTypeData) = 0x4F,
    SubType(SubTypeData) = 0x50,
    FinalEmptySubType(CompositeType),
}

/// [Composite type](https://webassembly.github.io/spec/core/binary/types.html#binary-comptype).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum CompositeType {
    Array(FieldType) = 0x5E,
    Struct(Vec<FieldType>) = 0x5f,
    Func(FuncType),
}

/// [Field type](https://webassembly.github.io/spec/core/binary/types.html#binary-fieldtype).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FieldType {
    pub storage_type: StorageType,
    pub mutable: bool,
}

/// [Storage type](https://webassembly.github.io/spec/core/binary/types.html#binary-storagetype).
#[derive(Wasmbin, WasmbinCountable, Debug, PartialEq, Eq, Hash, Clone, Visit)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum StorageType {
    ValueType(ValueType),
    I16 = 0x77,
    I8 = 0x78,
}
