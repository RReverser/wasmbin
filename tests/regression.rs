use anyhow::{Context, Result};
use std::io::Read;
use wasmbin::Module;
use wasmbin::types::HeapType;
use wasmbin::visit::Visit;

fn read_module<R: Read>(r: R) -> Result<Module> {
    let mut m = Module::decode_from(r).context("Decode error")?;
    m.visit_mut(|_: &mut HeapType| {})
        .context("Error visiting HeapTypes")?;
    Ok(m)
}

#[test]
fn heap_type_index_s33() -> Result<()> {
    let mod_bytes = include_bytes!("regression/heap_type_index_s33.wasm");
    let mut second_round = Vec::new();
    read_module(&mod_bytes[..])?.encode_into(&mut second_round)?;
    read_module(&second_round[..])?;
    Ok(())
}

fn roundtrip_legacy_modules(modules: &[&str]) -> Result<()> {
    for module in modules {
        let buffer = wast::parser::ParseBuffer::new(module)?;
        let module = wast::parser::parse::<wast::Wat>(&buffer)?.encode()?;
        let mut encoded = Vec::new();
        read_module(module.as_slice())?.encode_into(&mut encoded)?;
        read_module(&encoded[..])?;
    }
    Ok(())
}
#[test]
fn legacy_throw_ref() -> Result<()> {
    roundtrip_legacy_modules(&[
        // test.0.wasm
        r#"(module
          (type (func))
          (func (type 0)
            try
            catch 0
              throw 0
              rethrow 0
              throw_ref
            end
            try
            delegate 0
          )
        )"#,
    ])
}

#[test]
fn legacy_throw() -> Result<()> {
    roundtrip_legacy_modules(&[
        // test.0.wasm
        r#"(module
          (type (func))
          (type (func (param i32)))
          (type (func (param f32)))
          (type (func (param i64)))
          (type (func (param f64)))
          (type (func (param i32 i32)))
          (type (func (param i32) (result i32)))
          (tag (type 0))
          (tag (type 1) (param i32))
          (tag (type 2) (param f32))
          (tag (type 3) (param i64))
          (tag (type 4) (param f64))
          (tag (type 5) (param i32 i32))
          (export "throw-if" (func 0))
          (export "throw-param-f32" (func 1))
          (export "throw-param-i64" (func 2))
          (export "throw-param-f64" (func 3))
          (export "test-throw-1-2" (func 5))
          (func (type 6) (param i32) (result i32)
            local.get 0
            i32.const 0
            i32.ne
            if
              throw 0
            end
            i32.const 0
          )
          (func (type 2) (param f32)
            local.get 0
            throw 2
          )
          (func (type 3) (param i64)
            local.get 0
            throw 3
          )
          (func (type 4) (param f64)
            local.get 0
            throw 4
          )
          (func (type 0)
            i32.const 1
            i32.const 2
            throw 5
          )
          (func (type 0)
            try
              call 4
            catch 5
              i32.const 2
              i32.ne
              if
                unreachable
              end
              i32.const 1
              i32.ne
              if
                unreachable
              end
            end
          )
        )"#,
        // test.1.wasm
        r#"(module
          (type (func))
          (func (type 0)
            throw 0
          )
        )"#,
        // test.2.wasm
        r#"(module
          (type (func (param i32)))
          (type (func))
          (tag (type 0) (param i32))
          (func (type 1)
            throw 0
          )
        )"#,
        // test.3.wasm
        r#"(module
          (type (func (param i32)))
          (type (func))
          (tag (type 0) (param i32))
          (func (type 1)
            i64.const 5
            throw 0
          )
        )"#,
    ])
}

#[test]
fn legacy_rethrow() -> Result<()> {
    roundtrip_legacy_modules(&[
        // test.0.wasm
        r#"(module
          (type (func))
          (type (func (param i32) (result i32)))
          (tag (type 0))
          (tag (type 0))
          (export "catch-rethrow-0" (func 0))
          (export "catch-rethrow-1" (func 1))
          (export "catchall-rethrow-0" (func 2))
          (export "catchall-rethrow-1" (func 3))
          (export "rethrow-nested" (func 4))
          (export "rethrow-recatch" (func 5))
          (export "rethrow-stack-polymorphism" (func 6))
          (func (type 0)
            try
              throw 0
            catch 0
              rethrow 0
            end
          )
          (func (type 1) (param i32) (result i32)
            try (result i32)
              throw 0
            catch 0
              local.get 0
              i32.eqz
              if
                rethrow 1
              end
              i32.const 23
            end
          )
          (func (type 0)
            try
              throw 0
            catch_all
              rethrow 0
            end
          )
          (func (type 1) (param i32) (result i32)
            try (result i32)
              throw 0
            catch_all
              local.get 0
              i32.eqz
              if
                rethrow 1
              end
              i32.const 23
            end
          )
          (func (type 1) (param i32) (result i32)
            try (result i32)
              throw 1
            catch 1
              try (result i32)
                throw 0
              catch 0
                local.get 0
                i32.const 0
                i32.eq
                if
                  rethrow 1
                end
                local.get 0
                i32.const 1
                i32.eq
                if
                  rethrow 2
                end
                i32.const 23
              end
            end
          )
          (func (type 1) (param i32) (result i32)
            try (result i32)
              throw 0
            catch 0
              try (result i32)
                local.get 0
                i32.eqz
                if
                  rethrow 2
                end
                i32.const 42
              catch 0
                i32.const 23
              end
            end
          )
          (func (type 0)
            try
              throw 0
            catch 0
              i32.const 1
              rethrow 0
            end
          )
        )"#,
        // test.1.wasm
        r#"(module
          (type (func))
          (func (type 0)
            rethrow 0
          )
        )"#,
        // test.2.wasm
        r#"(module
          (type (func))
          (func (type 0)
            block
              rethrow 0
            end
          )
        )"#,
        // test.3.wasm
        r#"(module
          (type (func))
          (func (type 0)
            try
              rethrow 0
            delegate 0
          )
        )"#,
    ])
}

#[test]
fn legacy_try_catch() -> Result<()> {
    roundtrip_legacy_modules(&[
        // test.0.wasm
        r#"(module
          (type (func))
          (tag (type 0))
          (export "e0" (tag 0))
          (export "throw" (func 0))
          (func (type 0)
            throw 0
          )
        )"#,
        // test.1.wasm
        r#"(module
          (type (func))
          (type (func (param i32)))
          (type (func (param f32)))
          (type (func (param i64)))
          (type (func (param f64)))
          (type (func (param i32) (result i32)))
          (type (func (param i32 i32) (result i32)))
          (type (func (param f32) (result f32)))
          (type (func (param i64) (result i64)))
          (type (func (param f64) (result f64)))
          (type (func (result i32)))
          (import "test" "e0" (tag (type 0)))
          (import "test" "throw" (func (type 0)))
          (table 1 1 funcref)
          (tag (type 0))
          (tag (type 0))
          (tag (type 0))
          (tag (type 1) (param i32))
          (tag (type 2) (param f32))
          (tag (type 3) (param i64))
          (tag (type 4) (param f64))
          (export "empty-catch" (func 2))
          (export "simple-throw-catch" (func 3))
          (export "unreachable-not-caught" (func 4))
          (export "trap-in-callee" (func 6))
          (export "catch-complex-1" (func 7))
          (export "catch-complex-2" (func 8))
          (export "throw-catch-param-i32" (func 9))
          (export "throw-catch-param-f32" (func 10))
          (export "throw-catch-param-i64" (func 11))
          (export "throw-catch-param-f64" (func 12))
          (export "catch-param-i32" (func 14))
          (export "catch-imported" (func 15))
          (export "catchless-try" (func 16))
          (export "return-call-in-try-catch" (func 18))
          (export "return-call-indirect-in-try-catch" (func 19))
          (export "break-try-catch" (func 20))
          (export "break-try-catch_all" (func 21))
          (elem (i32.const 0) func 17)
          (func (type 5) (param i32) (result i32)
            local.get 0
            i32.const 0
            i32.ne
            if
              throw 1
            end
            i32.const 0
          )
          (func (type 0)
            try
            catch 1
            end
          )
          (func (type 5) (param i32) (result i32)
            try (result i32)
              local.get 0
              i32.eqz
              if
                throw 1
              end
              i32.const 42
            catch 1
              i32.const 23
            end
          )
          (func (type 0)
            try
              unreachable
            catch_all
            end
          )
          (func (type 6) (param i32 i32) (result i32)
            local.get 0
            local.get 1
            i32.div_u
          )
          (func (type 6) (param i32 i32) (result i32)
            try (result i32)
              local.get 0
              local.get 1
              call 5
            catch_all
              i32.const 11
            end
          )
          (func (type 5) (param i32) (result i32)
            try (result i32)
              try (result i32)
                local.get 0
                i32.eqz
                if
                  throw 1
                else
                  local.get 0
                  i32.const 1
                  i32.eq
                  if
                    throw 2
                  else
                    throw 3
                  end
                end
                i32.const 2
              catch 1
                i32.const 3
              end
            catch 2
              i32.const 4
            end
          )
          (func (type 5) (param i32) (result i32)
            try (result i32)
              local.get 0
              i32.eqz
              if
                throw 1
              else
                local.get 0
                i32.const 1
                i32.eq
                if
                  throw 2
                else
                  throw 3
                end
              end
              i32.const 2
            catch 1
              i32.const 3
            catch 2
              i32.const 4
            end
          )
          (func (type 5) (param i32) (result i32)
            try (result i32)
              local.get 0
              throw 4
              i32.const 2
            catch 4
              return
            end
          )
          (func (type 7) (param f32) (result f32)
            try (result f32)
              local.get 0
              throw 5
              f32.const 0x0p+0
            catch 5
              return
            end
          )
          (func (type 8) (param i64) (result i64)
            try (result i64)
              local.get 0
              throw 6
              i64.const 2
            catch 6
              return
            end
          )
          (func (type 9) (param f64) (result f64)
            try (result f64)
              local.get 0
              throw 7
              f64.const 0x0p+0
            catch 7
              return
            end
          )
          (func (type 1) (param i32)
            local.get 0
            throw 4
          )
          (func (type 5) (param i32) (result i32)
            try (result i32)
              i32.const 0
              local.get 0
              call 13
            catch 4
            end
          )
          (func (type 10) (result i32)
            try (result i32)
              i32.const 1
              call 0
            catch 0
              i32.const 2
            end
          )
          (func (type 5) (param i32) (result i32)
            try (result i32)
              try (result i32)
                local.get 0
                call 1
              end
            catch 1
              i32.const 1
            end
          )
          (func (type 0)
            throw 1
          )
          (func (type 0)
            try
              return_call 17
            catch 1
            end
          )
          (func (type 0)
            try
              i32.const 0
              return_call_indirect (type 0)
            catch 1
            end
          )
          (func (type 0)
            try
              br 0
            catch 1
            end
          )
          (func (type 0)
            try
              br 0
            catch_all
            end
          )
        )"#,
        // test.2.wasm
        r#"(module
          (type (func))
          (type (func (result i32)))
          (import "test" "throw" (func (type 0)))
          (tag (type 0))
          (export "imported-mismatch" (func 1))
          (func (type 1) (result i32)
            try (result i32)
              try (result i32)
                i32.const 1
                call 0
              catch 0
                i32.const 2
              end
            catch_all
              i32.const 3
            end
          )
        )"#,
        // test.6.wasm
        r#"(module
          (type (func (result i32)))
          (func (type 0) (result i32)
            try (result i32)
            end
          )
        )"#,
        // test.7.wasm
        r#"(module
          (type (func (result i32)))
          (func (type 0) (result i32)
            try (result i32)
              i64.const 42
            end
          )
        )"#,
        // test.8.wasm
        r#"(module
          (type (func))
          (tag (type 0))
          (func (type 0)
            try
            catch 0
              i32.const 42
            end
          )
        )"#,
        // test.9.wasm
        r#"(module
          (type (func (param i64)))
          (type (func (result i32)))
          (tag (type 0) (param i64))
          (func (type 1) (result i32)
            try (result i32)
              i32.const 42
            catch 0
            end
          )
        )"#,
        // test.10.wasm
        r#"(module
          (type (func))
          (func (type 0)
            try
            catch_all
              i32.const 42
            end
          )
        )"#,
    ])
}

#[test]
fn legacy_try_delegate() -> Result<()> {
    roundtrip_legacy_modules(&[
        // test.0.wasm
        r#"(module
          (type (func))
          (type (func (result i32)))
          (type (func (param i32)))
          (type (func (param i32) (result i32)))
          (type (func (param i32 i32) (result i32)))
          (table 1 1 funcref)
          (tag (type 0))
          (tag (type 0))
          (export "delegate-no-throw" (func 0))
          (export "delegate-throw" (func 2))
          (export "delegate-skip" (func 3))
          (export "delegate-to-block" (func 4))
          (export "delegate-to-catch" (func 5))
          (export "delegate-to-caller-trivial" (func 6))
          (export "delegate-to-caller-skipping" (func 7))
          (export "delegate-merge" (func 9))
          (export "delegate-throw-no-catch" (func 10))
          (export "delegate-correct-targets" (func 11))
          (export "return-call-in-try-delegate" (func 13))
          (export "return-call-indirect-in-try-delegate" (func 14))
          (export "break-try-delegate" (func 15))
          (export "break-and-call-throw" (func 16))
          (export "break-and-throw" (func 17))
          (elem (i32.const 0) func 12)
          (func (type 1) (result i32)
            try (result i32)
              try (result i32)
                i32.const 1
              delegate 0
            catch 0
              i32.const 2
            end
          )
          (func (type 2) (param i32)
            local.get 0
            if
              throw 0
            end
          )
          (func (type 3) (param i32) (result i32)
            try (result i32)
              try (result i32)
                local.get 0
                call 1
                i32.const 1
              delegate 0
            catch 0
              i32.const 2
            end
          )
          (func (type 1) (result i32)
            try (result i32)
              try (result i32)
                try (result i32)
                  throw 0
                  i32.const 1
                delegate 1
              catch 0
                i32.const 2
              end
            catch 0
              i32.const 3
            end
          )
          (func (type 1) (result i32)
            try (result i32)
              block
                try
                  throw 0
                delegate 0
              end
              i32.const 0
            catch_all
              i32.const 1
            end
          )
          (func (type 1) (result i32)
            try (result i32)
              try
                throw 0
              catch 0
                try
                  rethrow 1
                delegate 0
              end
              i32.const 0
            catch_all
              i32.const 1
            end
          )
          (func (type 0)
            try
              throw 0
            delegate 0
          )
          (func (type 0)
            try
              try
                throw 0
              delegate 1
            catch_all
            end
          )
          (func (type 2) (param i32)
            block
              block
                block
                  local.get 0
                  br_table 0 1 2
                end
                return
              end
              throw 0
            end
            throw 1
          )
          (func (type 4) (param i32 i32) (result i32)
            try (result i32)
              local.get 0
              call 8
              try (result i32)
                local.get 1
                call 8
                i32.const 1
              delegate 0
            catch 0
              i32.const 2
            end
          )
          (func (type 1) (result i32)
            try (result i32)
              try (result i32)
                throw 0
                i32.const 1
              delegate 0
            catch 1
              i32.const 2
            end
          )
          (func (type 1) (result i32)
            try (result i32)
              try
                try
                  try
                    try
                      try
                        throw 0
                      delegate 1
                    catch_all
                      unreachable
                    end
                  delegate 1
                catch_all
                  unreachable
                end
              catch_all
                try
                  throw 0
                delegate 0
              end
              unreachable
            catch_all
              i32.const 1
            end
          )
          (func (type 0)
            throw 0
          )
          (func (type 0)
            try
              try
                return_call 12
              delegate 0
            catch 0
            end
          )
          (func (type 0)
            try
              try
                i32.const 0
                return_call_indirect (type 0)
              delegate 0
            catch 0
            end
          )
          (func (type 0)
            try
              br 0
            delegate 0
          )
          (func (type 1) (result i32)
            try (result i32)
              try (result i32)
                block
                  try
                    br 1
                  delegate 2
                end
                call 12
                i32.const 0
              catch 0
                i32.const 1
              end
            catch 0
              i32.const 2
            end
          )
          (func (type 1) (result i32)
            try (result i32)
              try (result i32)
                block
                  try
                    br 1
                  delegate 2
                end
                throw 0
                i32.const 0
              catch 0
                i32.const 1
              end
            catch 0
              i32.const 2
            end
          )
        )"#,
        // test.5.wasm
        r#"(module
          (type (func))
          (func (type 0)
            try
            delegate 1
          )
        )"#,
    ])
}
