use wasmbin::Module;
use wasmbin::instructions::{Instruction, Misc};
#[cfg(feature = "wide-arithmetic")]
use wasmbin::io::Encode;
use wasmbin::io::{Decode, DecodeError, DecodeErrorKind};
use wasmbin::visit::Visit;

#[cfg(feature = "wide-arithmetic")]
fn cases() -> [(Misc, u8); 4] {
    [
        (Misc::I64Add128, 0x13),
        (Misc::I64Sub128, 0x14),
        (Misc::I64MulWideS, 0x15),
        (Misc::I64MulWideU, 0x16),
    ]
}

fn padded_opcode(opcode: u8, len: usize) -> Vec<u8> {
    let mut bytes = vec![opcode];
    while bytes.len() < len {
        *bytes.last_mut().unwrap() |= 0x80;
        bytes.push(0);
    }
    bytes
}

fn assert_unsupported(error: DecodeError, opcode: u8) {
    match error.kind {
        DecodeErrorKind::UnsupportedDiscriminant { ty, discriminant } => {
            assert_eq!(ty, std::any::type_name::<Misc>());
            assert_eq!(discriminant, i128::from(opcode));
        }
        other => panic!("expected unsupported opcode {opcode:#x}, got {other:?}"),
    }
}

#[test]
fn neighboring_opcodes_remain_unsupported() {
    for opcode in [0x12, 0x17] {
        assert_unsupported(Misc::decode(&mut &[opcode][..]).unwrap_err(), opcode);
        assert_unsupported(
            Instruction::decode(&mut &[0xFC, opcode][..]).unwrap_err(),
            opcode,
        );
    }
}

#[test]
fn malformed_subopcodes_are_rejected() {
    for opcode in 0x13..=0x16 {
        let mut malformed = vec![vec![]];
        // Unterminated encodings, including the maximum u32 length.
        for len in 1..=5 {
            let mut bytes = padded_opcode(opcode, len);
            *bytes.last_mut().unwrap() |= 0x80;
            malformed.push(bytes);
        }
        // Six-byte encoding of a small value, and a five-byte value exceeding u32.
        malformed.push(padded_opcode(opcode, 6));
        malformed.push(vec![opcode | 0x80, 0x80, 0x80, 0x80, 0x10]);
        for bytes in malformed {
            let error = Misc::decode(&mut bytes.as_slice()).unwrap_err();
            assert!(matches!(
                error.kind,
                DecodeErrorKind::Io(_) | DecodeErrorKind::Leb128(_)
            ));
            let prefixed = [vec![0xFC], bytes].concat();
            let error = Instruction::decode(&mut prefixed.as_slice()).unwrap_err();
            assert!(matches!(
                error.kind,
                DecodeErrorKind::Io(_) | DecodeErrorKind::Leb128(_)
            ));
        }
    }
}

#[cfg(feature = "wide-arithmetic")]
#[test]
fn exact_encodings_and_padded_decoding() {
    for (misc, opcode) in cases() {
        let instruction = Instruction::Misc(misc.clone());
        let mut encoded = Vec::new();
        misc.encode(&mut encoded).unwrap();
        assert_eq!(encoded, [opcode]);
        encoded.clear();
        instruction.encode(&mut encoded).unwrap();
        assert_eq!(encoded, [0xFC, opcode]);

        for len in 1..=5 {
            let bytes = padded_opcode(opcode, len);
            let mut input = bytes.as_slice();
            let decoded = Misc::decode(&mut input).unwrap();
            assert_eq!(decoded, misc);
            assert_eq!(input, []);
            encoded.clear();
            decoded.encode(&mut encoded).unwrap();
            assert_eq!(encoded, [opcode]);

            let bytes = [vec![0xFC], bytes, vec![0x01]].concat();
            let mut input = bytes.as_slice();
            let decoded = Instruction::decode(&mut input).unwrap();
            assert_eq!(decoded, instruction);
            assert_eq!(input, [0x01]);
            encoded.clear();
            decoded.encode(&mut encoded).unwrap();
            assert_eq!(encoded, [0xFC, opcode]);
            assert_eq!(Instruction::decode(&mut input).unwrap(), Instruction::Nop);
            assert_eq!(input, []);
        }
    }
}

#[cfg(feature = "wide-arithmetic")]
#[test]
fn expression_boundaries() {
    let bytes = [0xFC, 0x13, 0xFC, 0x14, 0xFC, 0x15, 0xFC, 0x16, 0x0B, 0x01];
    let mut input = bytes.as_slice();
    let expression = Vec::<Instruction>::decode(&mut input).unwrap();
    let expected: Vec<_> = cases()
        .into_iter()
        .map(|(misc, _)| Instruction::Misc(misc))
        .collect();
    assert_eq!(expression, expected);
    assert_eq!(input, [0x01]);
    let mut encoded = Vec::new();
    expression.encode(&mut encoded).unwrap();
    assert_eq!(encoded, bytes[..9]);
}

fn encode_wat(source: &str) -> Vec<u8> {
    let buffer = wast::parser::ParseBuffer::new(source).unwrap();
    wast::parser::parse::<wast::Wat>(&buffer)
        .unwrap()
        .encode()
        .unwrap()
}

#[cfg(feature = "wide-arithmetic")]
const MODULE: &str = r"(module
    (func (param i64 i64 i64 i64) (result i64 i64)
        local.get 0 local.get 1 local.get 2 local.get 3 i64.add128)
    (func (param i64 i64 i64 i64) (result i64 i64)
        local.get 0 local.get 1 local.get 2 local.get 3 i64.sub128)
    (func (param i64 i64) (result i64 i64)
        local.get 0 local.get 1 i64.mul_wide_s)
    (func (param i64 i64) (result i64 i64)
        local.get 0 local.get 1 i64.mul_wide_u)
)";

#[cfg(feature = "wide-arithmetic")]
#[test]
fn module_roundtrip_and_edit() {
    let bytes = encode_wat(MODULE);
    let mut module = Module::decode_from(bytes.as_slice()).unwrap();
    let mut seen = Vec::new();
    // Mutable visitation forces decoding and re-encoding instead of copying lazy bytes.
    module
        .visit_mut(|misc: &mut Misc| seen.push(misc.clone()))
        .unwrap();
    let expected: Vec<_> = cases().into_iter().map(|(misc, _)| misc).collect();
    assert_eq!(seen, expected);
    assert_eq!(module.encode_into(Vec::new()).unwrap(), bytes);

    module
        .visit_mut(|misc: &mut Misc| match misc {
            Misc::I64Add128 => *misc = Misc::I64Sub128,
            Misc::I64MulWideS => *misc = Misc::I64MulWideU,
            _ => {}
        })
        .unwrap();
    let expected_bytes = encode_wat(
        &MODULE
            .replace("i64.add128", "i64.sub128")
            .replace("i64.mul_wide_s", "i64.mul_wide_u"),
    );
    let edited = module.encode_into(Vec::new()).unwrap();
    assert_eq!(edited, expected_bytes);
    let mut decoded = Module::decode_from(edited.as_slice()).unwrap();
    decoded.visit_mut(|()| {}).unwrap();
    assert_eq!(decoded, module);
}

#[cfg(not(feature = "wide-arithmetic"))]
#[test]
fn instructions_require_feature() {
    for opcode in 0x13..=0x16 {
        for len in 1..=5 {
            let bytes = padded_opcode(opcode, len);
            assert_unsupported(Misc::decode(&mut bytes.as_slice()).unwrap_err(), opcode);
            let bytes = [vec![0xFC], bytes].concat();
            assert_unsupported(
                Instruction::decode(&mut bytes.as_slice()).unwrap_err(),
                opcode,
            );
        }
    }
}

#[cfg(not(feature = "wide-arithmetic"))]
#[test]
fn lazy_function_bodies_require_feature() {
    for (name, opcode, params) in [
        ("i64.add128", 0x13, "i64 i64 i64 i64"),
        ("i64.sub128", 0x14, "i64 i64 i64 i64"),
        ("i64.mul_wide_s", 0x15, "i64 i64"),
        ("i64.mul_wide_u", 0x16, "i64 i64"),
    ] {
        let args = if opcode <= 0x14 {
            "local.get 0 local.get 1 local.get 2 local.get 3"
        } else {
            "local.get 0 local.get 1"
        };
        let bytes = encode_wat(&format!(
            "(module (func (param {params}) (result i64 i64) {args} {name}))"
        ));
        let mut module = Module::decode_from(bytes.as_slice()).unwrap();
        let error = module.visit_mut(|()| {}).unwrap_err();
        assert_unsupported(error.into(), opcode);
    }
}
