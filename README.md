# wasmbin

wasmbin is a library implementing parsing and serialization WebAssembly binaries.

## Public API

The entry point is the [`Module`](https://docs.rs/wasmbin/latest/wasmbin/module/struct.Module.html) object.

From there you can explore the module contents by simply looking up the nested fields. The structures map pretty much 1:1 to raw WebAssembly binary format, so you can also use the [specification](https://webassembly.github.io/spec/core/binary/modules.html) as a reference.

## Supported feature extensions

Following WebAssembly proposals are supported in addition to the core spec and can be enabled via corresponding Cargo features:

- [`exception-handling`](https://github.com/WebAssembly/exception-handling)
- [`extended-name-section`](https://github.com/WebAssembly/extended-name-section)
- [`multi-memory`](https://github.com/WebAssembly/multi-memory)
- [`tail-call`](https://github.com/WebAssembly/tail-call)
- [`threads`](https://github.com/WebAssembly/threads)

## Motivation

Original blog post explaining motivation and internals: 
[wasmbin: a self-generating WebAssembly parser & serializer](https://rreverser.com/wasmbin-yet-another-webassembly-parser-serializer/).

This crate intends to provide a low-level representation of the WebAssembly module that is fully described by Rust type system. It also leverages the said type system in conjunction with custom proc-macros to autogenerate parsing/serialization/visitation code for any complex types (structures and enums).

On the user's side this approach allows any type can be used independently to represent/parse/serialize only part of the module, while on the maintainers' side it makes adding and testing new WebAssembly features as quick and easy as adding new types, fields, and variants, without having to write any manual code at all.

One other notable feature is a [`Lazy<T>`](https://docs.rs/wasmbin/latest/wasmbin/builtins/struct.Lazy.html) wrapper used in `wasmbin` whenever the spec permits efficiently skipping over some contents (e.g. function bodies). It allows minimally invasive, efficient, "zero-cost" editing of WebAssembly modules: during decoding anything that can be skipped, is skipped over lazily (the `Lazy<...>` container simply stores the raw bytes), and during encoding only the modified parts of the module are re-encoded, while any untouched parts are copied verbatim as raw bytes from the source.
