use wasmbin::Module;
use wasmbin::builtins::Lazy;
use wasmbin::indices::FuncId;
use wasmbin::io::{Decode, DecodeError, DecodeErrorKind, Encode};
use wasmbin::sections::{
    BranchHint, BranchHintValue, CustomSection, FunctionBranchHints, RawCustomSection, Section,
};
use wasmbin::visit::Visit;

const NAME: &str = "metadata.code.branch_hint";

fn encode(value: &impl Encode) -> Vec<u8> {
    let mut bytes = Vec::new();
    value.encode(&mut bytes).unwrap();
    bytes
}

fn hint(offset: u32, value: BranchHintValue) -> BranchHint {
    BranchHint { offset, value }
}

fn function(func: u32, hints: Vec<BranchHint>) -> FunctionBranchHints {
    FunctionBranchHints {
        func: func.into(),
        hints,
    }
}

fn raw_section(name: &str, data: &[u8]) -> Section {
    CustomSection::Other(RawCustomSection {
        name: name.into(),
        data: data.to_vec().into(),
    })
    .into()
}

fn module_bytes(payload: &[u8]) -> Vec<u8> {
    encode(&Module {
        sections: vec![
            raw_section(NAME, payload),
            Section::Start(FuncId::from(7).into()),
        ],
    })
}

fn hints(module: &Module) -> &Lazy<Vec<FunctionBranchHints>> {
    module
        .sections
        .iter()
        .find_map(|section| {
            if let Section::Custom(custom) = section
                && let CustomSection::BranchHints(hints) = custom.try_contents().unwrap()
            {
                return Some(hints);
            }
            None
        })
        .expect("typed branch hints")
}

fn padded(value: u8, len: usize) -> Vec<u8> {
    let mut bytes = vec![value];
    while bytes.len() < len {
        *bytes.last_mut().unwrap() |= 0x80;
        bytes.push(0);
    }
    bytes
}

#[test]
fn exact_encodings_and_boundaries() {
    use BranchHintValue::{LikelyFalse, LikelyTrue};

    for (value, byte) in [(LikelyFalse, 0), (LikelyTrue, 1)] {
        assert_eq!(encode(&value), [byte]);
        assert_eq!(encode(&hint(128, value)), [0x80, 1, 1, byte]);
        let bytes = [0x80, 1, 1, byte, 0xFF];
        let mut input = bytes.as_slice();
        assert_eq!(BranchHint::decode(&mut input).unwrap(), hint(128, value));
        assert_eq!(input, [0xFF]);
    }

    let cases = [
        (vec![], vec![0]),
        (vec![function(0, vec![])], vec![1, 0, 0]),
        (
            vec![
                function(0, vec![hint(3, LikelyFalse), hint(128, LikelyTrue)]),
                function(128, vec![hint(u32::MAX, LikelyFalse)]),
                function(u32::MAX, vec![]),
            ],
            vec![
                3, 0, 2, 3, 1, 0, 0x80, 1, 1, 1, 0x80, 1, 1, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 1, 0,
                0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0,
            ],
        ),
    ];
    for (expected, bytes) in cases {
        assert_eq!(encode(&expected), bytes);
        let mut input = bytes.as_slice();
        assert_eq!(
            Vec::<FunctionBranchHints>::decode(&mut input).unwrap(),
            expected
        );
        assert!(input.is_empty());
        let custom = CustomSection::BranchHints(expected.into());
        assert_eq!(custom.name(), NAME);
        let mut expected_bytes = vec![25];
        expected_bytes.extend_from_slice(NAME.as_bytes());
        expected_bytes.extend_from_slice(&bytes);
        assert_eq!(encode(&custom), expected_bytes);
    }
}

#[test]
fn padded_integers_are_preserved_until_mutation() {
    for value in [0, 1] {
        let canonical = [1, 0, 1, 3, 1, value];
        // Function count, function index, hint count, offset, fixed value, hint value.
        for field in 0..canonical.len() {
            for len in 2..=5 {
                let mut payload = canonical.to_vec();
                payload.splice(field..=field, padded(canonical[field], len));
                let bytes = module_bytes(&payload);
                let mut module = Module::decode_from(bytes.as_slice()).unwrap();
                assert_eq!(encode(&module), bytes);
                let expected = vec![function(
                    0,
                    vec![hint(
                        3,
                        if value == 0 {
                            BranchHintValue::LikelyFalse
                        } else {
                            BranchHintValue::LikelyTrue
                        },
                    )],
                )];
                assert_eq!(hints(&module).try_contents().unwrap(), &expected);
                assert_eq!(hints(&module).try_as_raw().unwrap().as_ref(), payload);
                assert_eq!(encode(&module), bytes);

                let mut seen = Vec::new();
                module
                    .visit_mut(|hint: &mut BranchHint| seen.push(*hint))
                    .unwrap();
                assert_eq!(seen, expected[0].hints);
                assert!(hints(&module).try_as_raw().is_err());
                assert_eq!(encode(&module), module_bytes(&canonical));
            }
        }
    }
}

fn assert_integer_error(error: &DecodeError) {
    assert!(
        matches!(
            error.kind,
            DecodeErrorKind::Io(_) | DecodeErrorKind::Leb128(_)
        ),
        "{error}"
    );
}

fn assert_preserved_error(payload: &[u8]) -> DecodeError {
    let bytes = module_bytes(payload);
    let mut module = Module::decode_from(bytes.as_slice()).unwrap();
    let error = hints(&module).try_contents().unwrap_err();
    assert_eq!(encode(&module), bytes);
    let mut seen = Vec::new();
    module.visit(|func: &FuncId| seen.push(func.index)).unwrap();
    assert_eq!(
        seen,
        [7],
        "traversal must continue after malformed custom data"
    );
    module
        .visit_mut(|_: &mut BranchHint| -> () { panic!("malformed hints must not be visited") })
        .unwrap();
    assert_eq!(encode(&module), bytes);
    assert!(hints(&module).try_contents().is_err());
    error
}

#[test]
fn malformed_integer_fields() {
    let canonical = [1, 0, 1, 3, 1, 1];
    for field in 0..canonical.len() {
        for len in 1..=5 {
            // Truncate at each integer, with every possible unterminated length.
            let mut payload = canonical[..field].to_vec();
            payload.extend(vec![0x80; len]);
            assert_integer_error(&assert_preserved_error(&payload));
        }
        for invalid in [
            padded(canonical[field], 6),
            vec![0x80, 0x80, 0x80, 0x80, 0x10],
        ] {
            let mut payload = canonical.to_vec();
            payload.splice(field..=field, invalid);
            assert_integer_error(&assert_preserved_error(&payload));
        }
    }
    for len in 0..canonical.len() {
        assert_integer_error(&assert_preserved_error(&canonical[..len]));
    }
}

#[test]
fn invalid_constants_counts_and_trailing_data() {
    for (field, invalid_values, ty, path) in [
        (
            4,
            vec![0, 2, 128, u32::MAX],
            std::any::type_name::<BranchHint>(),
            ".size",
        ),
        (
            5,
            vec![2, 128, u32::MAX],
            std::any::type_name::<BranchHintValue>(),
            ".value",
        ),
    ] {
        for value in invalid_values {
            let mut payload = vec![1, 0, 1, 3, 1, 1];
            payload.splice(field..=field, encode(&value));
            let error = assert_preserved_error(&payload);
            let message = error.to_string();
            assert!(
                message.contains("[0]") && message.contains(".hints[0]") && message.contains(path),
                "{message}"
            );
            match error.kind {
                DecodeErrorKind::UnsupportedDiscriminant {
                    ty: actual_ty,
                    discriminant,
                } => {
                    assert_eq!(actual_ty, ty);
                    assert_eq!(discriminant, i128::from(value));
                }
                other => panic!("unexpected error {other:?}"),
            }
        }
    }
    for payload in [vec![2, 0, 0], vec![1, 0, 2, 3, 1, 1]] {
        assert_integer_error(&assert_preserved_error(&payload));
    }
    for payload in [vec![0, 0], vec![1, 0, 1, 3, 1, 1, 0]] {
        assert!(matches!(
            assert_preserved_error(&payload).kind,
            DecodeErrorKind::UnrecognizedData
        ));
    }
}

fn annotated_module(first: &str, second: &str) -> Vec<u8> {
    let source = format!(
        r#"(module
        (import "host" "function" (func))
        (func)
        (func (param i32) (local i64)
            local.get 0
            (@metadata.code.branch_hint "{first}") if
                local.get 0
                (@metadata.code.branch_hint "{second}") br_if 0
            end
            {padding}
            local.get 0
            (@metadata.code.branch_hint "{first}") if end)
        (func (param i32)
            local.get 0
            (@metadata.code.branch_hint "{second}") br_if 0)
    )"#,
        padding = "nop ".repeat(130)
    );
    let buffer = wast::parser::ParseBuffer::new(&source).unwrap();
    wast::parser::parse::<wast::Wat>(&buffer)
        .unwrap()
        .encode()
        .unwrap()
}

#[test]
fn wast_interoperability_and_edits() {
    use BranchHintValue::{LikelyFalse, LikelyTrue};
    let bytes = annotated_module("\\00", "\\01");
    let mut module = Module::decode_from(bytes.as_slice()).unwrap();
    let expected = vec![
        function(
            2,
            vec![
                hint(5, LikelyFalse),
                hint(9, LikelyTrue),
                hint(144, LikelyFalse),
            ],
        ),
        function(3, vec![hint(3, LikelyTrue)]),
    ];
    assert_eq!(hints(&module).try_contents().unwrap(), &expected);
    // Force every lazy layer to encode, and prove the hint codec ran.
    let mut seen = Vec::new();
    module
        .visit_mut(|entry: &mut FunctionBranchHints| seen.push(entry.clone()))
        .unwrap();
    assert_eq!(seen, expected);
    module.visit_mut(|()| {}).unwrap();
    assert_eq!(encode(&module), bytes);

    let mut visited = 0;
    module
        .visit_mut(|value: &mut BranchHintValue| {
            visited += 1;
            *value = match *value {
                LikelyFalse => LikelyTrue,
                LikelyTrue => LikelyFalse,
            };
        })
        .unwrap();
    assert_eq!(visited, 4);
    let edited = encode(&module);
    assert_eq!(edited, annotated_module("\\01", "\\00"));
    let decoded = Module::decode_from(edited.as_slice()).unwrap();
    assert_eq!(
        hints(&decoded).try_contents().unwrap(),
        hints(&module).try_contents().unwrap()
    );

    let mut funcs = Vec::new();
    module
        .visit_mut(|func: &mut FuncId| {
            funcs.push(func.index);
            func.index += 128;
        })
        .unwrap();
    assert_eq!(funcs, [2, 3]);
    let remapped = encode(&module);
    let decoded = Module::decode_from(remapped.as_slice()).unwrap();
    let entries = hints(&decoded).try_contents().unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.func.index)
            .collect::<Vec<_>>(),
        [130, 131]
    );
    assert_eq!(
        entries[0].hints,
        hints(&module).try_contents().unwrap()[0].hints
    );
}

#[test]
fn semantic_constraints_are_left_to_callers() {
    use BranchHintValue::{LikelyFalse, LikelyTrue};
    // Deliberately unsorted, duplicate, nonexistent function indices and offsets.
    let entries = vec![
        function(
            7,
            vec![
                hint(99, LikelyFalse),
                hint(0, LikelyTrue),
                hint(0, LikelyFalse),
            ],
        ),
        function(0, vec![]),
        function(7, vec![]),
    ];
    let section = CustomSection::BranchHints(entries.clone().into());
    let module = Module {
        sections: vec![
            Section::Code(Vec::new().into()),
            section.clone().into(),
            section.into(),
        ],
    };
    let bytes = encode(&module);
    let mut decoded = Module::decode_from(bytes.as_slice()).unwrap();
    let mut seen = Vec::new();
    decoded
        .visit_mut(|entry: &mut FunctionBranchHints| seen.push(entry.clone()))
        .unwrap();
    assert_eq!(seen, [entries.clone(), entries].concat());
    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn other_custom_section_names_remain_raw() {
    for name in [
        "metadata.code.branch_hint.extra",
        "metadata.code",
        "unknown",
    ] {
        let section = raw_section(name, &[0xFF]);
        let bytes = encode(&section);
        let decoded = Section::decode(&mut bytes.as_slice()).unwrap();
        let Section::Custom(custom) = decoded else {
            panic!("custom section")
        };
        let CustomSection::Other(raw) = custom.try_contents().unwrap() else {
            panic!("raw section")
        };
        assert_eq!(raw.name, name);
        assert_eq!(raw.data.as_ref(), [0xFF]);
    }
}

#[cfg(feature = "serde")]
#[test]
fn public_types_support_serde() {
    fn assert_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}
    assert_serde::<BranchHintValue>();
    assert_serde::<BranchHint>();
    assert_serde::<FunctionBranchHints>();
    assert_serde::<CustomSection>();
}
