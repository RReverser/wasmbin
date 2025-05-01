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

use anyhow::{bail, ensure, Result};
use fs_err::{read_dir, read_to_string};
use indexmap::IndexMap;
use libtest_mimic::{run as run_tests, Arguments, Failed, Trial};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use wasmbin::io::DecodeError;
use wasmbin::visit::{Visit, VisitError};
use wasmbin::Module;
use wast::lexer::Lexer;
use wast::parser::{parse, ParseBuffer};
use wast::{QuoteWat, Wast};

const IGNORED_ERRORS: &[&str] = &[
    // We don't perform cross-section analysis.
    "function and code section have inconsistent lengths",
    "data count section required",
    "data count and data section have inconsistent lengths",
    // We allow non-zero table and memory IDs already.
    "zero byte expected",
    // We don't perform full function analysis either.
    "too many locals",
    // We don't validate whether instructions access out-of bounds memory.
    "malformed memop flags",
];

const IGNORED_MODULES: &[&[u8]] = &[
    // Upstream malformed test that becomes well-formed if threads are enabled.
    #[cfg(feature = "threads")]
    &[
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x05, 0x03, 0x01, 0x02, 0x00,
    ],
    // Wasmbin does not yet support the memory64 proposal
    #[cfg(feature = "custom-page-sizes")]
    &[
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x02,
        0x10, 0x01, 0x04, 0x74, 0x65, 0x73, 0x74, 0x07, 0x75, 0x6E, 0x6B, 0x6E, 0x6F, 0x77, 0x6E,
        0x00, 0x00, 0x05, 0x0A, 0x01, 0x0C, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x40, 0x10,
    ],
    #[cfg(feature = "custom-page-sizes")]
    &[
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x05, 0x0A, 0x01, 0x0C, 0x81, 0x80, 0x80,
        0x80, 0x80, 0x80, 0x40, 0x10,
    ],
    #[cfg(feature = "custom-page-sizes")]
    &[
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x02,
        0x0F, 0x01, 0x04, 0x74, 0x65, 0x73, 0x74, 0x06, 0x69, 0x6D, 0x70, 0x6F, 0x72, 0x74, 0x00,
        0x00, 0x05, 0x0D, 0x01, 0x0C, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01,
        0x00,
    ],
    // Wasmbin correctly fails to parse i32 memory size that is larger than i32::MAX
    #[cfg(feature = "custom-page-sizes")]
    &[
        0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x05, 0x08, 0x01, 0x08, 0x80, 0x80, 0x80,
        0x80, 0x10, 0x00,
    ],
];

#[derive(Default)]
struct Tests {
    deduped: IndexMap<Arc<Vec<u8>>, Trial>,
}

impl Tests {
    fn read_tests_from_file(&mut self, path: &Path) -> Result<()> {
        let src = read_to_string(path)?;
        let set_err_path_text = |mut err: wast::Error| {
            err.set_path(path);
            err.set_text(&src);
            err
        };
        let mut lexer = Lexer::new(&src);
        lexer.allow_confusing_unicode(true);
        let buf = ParseBuffer::new_with_lexer(lexer).map_err(set_err_path_text)?;
        let wast = parse::<Wast>(&buf).map_err(set_err_path_text)?;
        for directive in wast.directives {
            let span = directive.span();
            let (mut module, expect_result) = match directive {
                // Expect errors for assert_malformed on binary or AST modules.
                wast::WastDirective::AssertMalformed {
                    module,
                    message,
                    ..
                }
                // Unlike other AssertInvalid, this is actually something we
                // check at the parsing time, because it's part of our
                // typesystem and doesn't require cross-function or
                // cross-section checks.
                //
                // See https://github.com/WebAssembly/simd/issues/256 for accepted
                // but pending suggestion to change proposal to match this as well.
                | wast::WastDirective::AssertInvalid {
                    module,
                    message: message @ "invalid lane index",
                    ..
                } => (module, Err(message.to_owned())),
                // Expect successful parsing for regular AST modules.
                wast::WastDirective::Module(module) => (module, Ok(())),
                // Counter-intuitively, expect successful parsing for modules that are supposed
                // to error out at runtime or linking stage, too.
                wast::WastDirective::AssertInvalid { module, .. } => (module, Ok(())),
                wast::WastDirective::AssertUnlinkable { module, .. } => (QuoteWat::Wat(module), Ok(())),
                _ => {
                    // Skipping interpreted
                    continue;
                }
            };
            let Ok(raw_module) = module.encode() else {
                // If module can't be encoded, there's nothing our parser can do.
                continue;
            };
            let raw_module = Arc::new(raw_module);
            self.deduped
                .entry(Arc::clone(&raw_module))
                .or_insert_with(|| {
                    let is_ignored = IGNORED_MODULES.contains(&raw_module.as_slice())
                        || match &expect_result {
                            Ok(()) => false,
                            Err(err) => IGNORED_ERRORS.contains(&err.as_str()),
                        };

                    let (line, col) = span.linecol_in(&src);

                    Trial::test(
                        format!("{}:{}:{}", path.display(), line + 1, col + 1),
                        move || run_test(&raw_module, expect_result).map_err(Failed::from),
                    )
                    .with_ignored_flag(is_ignored)
                });
        }
        Ok(())
    }

    fn read_all_tests(path: &Path) -> Result<Vec<Trial>> {
        let mut test_files = Vec::new();

        let mut add_test_files_in_dir = |path: &Path| -> Result<()> {
            for file in read_dir(path)? {
                let path = file?.path();
                if path.extension().map_or(false, |ext| ext == "wast") {
                    test_files.push(path);
                }
            }
            Ok(())
        };

        add_test_files_in_dir(path)?;

        let proposals_dir = path.join("proposals");

        macro_rules! read_proposal_tests {
            ($name:literal) => {
                add_test_files_in_dir(&proposals_dir.join($name))?;
            };

            (? $name:literal) => {
                if cfg!(feature = $name) {
                    read_proposal_tests!($name);
                }
            };
        }

        read_proposal_tests!(? "exception-handling");
        read_proposal_tests!("extended-const");
        read_proposal_tests!("multi-memory");
        read_proposal_tests!("tail-call");
        read_proposal_tests!(? "threads");
        read_proposal_tests!(? "custom-page-sizes");

        ensure!(
            !test_files.is_empty(),
            "Couldn't find any tests. Did you run `git submodule update --init`?"
        );

        Ok(test_files
            .into_par_iter()
            .try_fold(Tests::default, |mut tests, path| {
                tests.read_tests_from_file(&path)?;
                Ok::<_, anyhow::Error>(tests)
            })
            .try_reduce(Self::default, |mut a, b| {
                a.deduped.extend(b.deduped);
                Ok(a)
            })?
            .deduped
            .into_par_iter()
            .map(|(_, test)| test)
            .collect())
    }
}

fn unlazify<T: Visit>(mut wasm: T) -> Result<T, DecodeError> {
    match wasm.visit_mut(|()| {}) {
        Ok(()) => Ok(wasm),
        Err(err) => match err {
            VisitError::LazyDecode(err) => Err(err),
            VisitError::Custom(err) => match err {},
        },
    }
}

fn run_test(mut test_module: &[u8], expect_result: Result<(), String>) -> Result<()> {
    let orig_test_module = test_module;
    let module = match (Module::decode_from(&mut test_module).and_then(unlazify), &expect_result) {
        (Ok(ref module), Err(err)) => bail!("Expected an invalid module definition with an error: {err}\nParsed part: {parsed_part:02X?}\nGot module: {module:#?}", parsed_part = &orig_test_module[..orig_test_module.len() - test_module.len()]),
        (Err(err), Ok(())) => bail!(
            "Error: {err:#}\nExpected a valid module definition, but got an error\nModule: {orig_test_module:#02X?}"
        ),
        (Ok(module), Ok(())) => module,
        (Err(_), Err(_)) => return Ok(()),
    };
    let out = module.encode_into(Vec::new())?;
    if out != test_module {
        // In the rare case that binary representation doesn't match, it
        // might be because the test uses longer LEB128 form than
        // required. Verify that at least decoding it back produces the
        // same module.
        let module2 = Module::decode_from(out.as_slice())?;
        ensure!(
            module == module2,
            "Roundtrip mismatch. Old: {module:#?}\nNew: {module2:#?}"
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let tests = Tests::read_all_tests(&Path::new("tests").join("testsuite"))?;

    let mut args = Arguments::from_args();
    // Default to terse output as we have *a lot* of tests.
    if args.format.is_none() {
        args.format = Some(libtest_mimic::FormatSetting::Terse);
    }

    run_tests(&args, tests).exit()
}
