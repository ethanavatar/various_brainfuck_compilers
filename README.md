# Various Brainfuck Compilers

A *soon to be* collection of brainfuck compilers to serve as minimal examples using many different compiler backends.

Considering:
- [bytecodealliance/cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift)
- [taricorp/llvm-sys.rs](https://gitlab.com/taricorp/llvm-sys.rs) (Basically just LLVM C)
- [QBE](https://c9x.me/compile/)
- [libfirm/libfirm](https://github.com/libfirm/libfirm)


In working condition:
- [LLVM Inkwell](#llvm-inkwell)

## LLVM Inkwell

This compiler uses the [TheDan64/inkwell](https://github.com/TheDan64/inkwell) backend. It is a safe wrapper around [taricorp/llvm-sys.rs](https://gitlab.com/taricorp/llvm-sys.rs) which directly wraps the LLVM C API.

To build this example on windows, have a **source build** of [LLVM 16.0.x](https://github.com/llvm/llvm-project/releases/tag/llvmorg-16.0.6) ([build instructions](https://llvm.org/docs/CMake.html)) on your system, and the path to its installation directory in the `LLVM_SYS_160_PREFIX` environment variable.

On linux, just install whatever your distro's equivalent of `llvm-dev` is.

Additionally, you'll need a way of linking `.o` (object) files or compiling `.ll` (LLVM IR) files. `clang` is a good choice for both.

```sh
$ cargo run --release
Usage: llvm_inkwell_bfc.exe [OPTIONS] <INPUT>

Arguments:
  <INPUT>  The input file to compile

Options:
  -t, --target <TARGET>  The target triple to compile for (e.g. x86_64-pc-linux-gnu, x86_64-pc-windows-msvc). if not specified, the module will be output as LLVM IR
  -o, --opt <OPT>        Optimization level
  -h, --help             Print help
```

## Example

```sh
$ cargo run --release -- --target x86_64-pc-windows-msvc --opt 3 ../bf_examples/hello.bf
Compiling for target: x86_64-pc-windows-msvc
Wrote module to object file: hello.o

$ clang hello.o -o hello.exe
$ ./hello.exe
Hello World!
```

