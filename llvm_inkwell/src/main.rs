use std::path::{Path, PathBuf};
use inkwell::targets::{Target, TargetTriple};

use clap::Parser;

#[derive(Parser)]
struct Args {
    /// The target triple to compile for (e.g. x86_64-pc-linux-gnu, x86_64-pc-windows-msvc).
    /// If not specified, the module will be output as LLVM IR
    #[clap(short, long)]
    target: Option<String>,

    /// Optimization level
    #[clap(short, long)]
    opt: Option<String>,

    /// The input file to compile
    #[clap()]
    input: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let file = std::fs::read_to_string(&args.input)?;
    let module_name = args.input.file_stem().unwrap().to_str().unwrap();

    let content = file
        .chars()
        .filter(|c| "+-<>[],.".contains(*c))
        .collect::<String>();

    // create an array of strings where each string is a single character
    // or a sequence of many + or - characters
    let grouped = content.chars().fold(Vec::<String>::new(), |mut acc, c| {
        acc.last_mut()
            .filter(|last| last.starts_with(c) && c == '+' || last.starts_with(c) && c == '-')
            .map(|last| last.push(c))
            .unwrap_or_else(|| acc.push(c.to_string()));
        acc
    });

    let context = inkwell::context::Context::create();
    let i8_type = context.i8_type();

    let module = context.create_module(module_name);
    let builder = context.create_builder();

    let putchar_type = context.i32_type().fn_type(&[i8_type.into()], false);
    let putchar_func = module.add_function("putchar", putchar_type, Some(inkwell::module::Linkage::External));

    let getchar_type = i8_type.fn_type(&[], false);
    let getchar_func = module.add_function("getchar", getchar_type, Some(inkwell::module::Linkage::External));

    let i8_ptr_type = i8_type.ptr_type(inkwell::AddressSpace::default());

    let main_type = i8_type.fn_type(&[], false);
    let main_func = module.add_function("main", main_type, None);
    let basic_block = context.append_basic_block(main_func, "entry");
    builder.position_at_end(basic_block);

    let memory_size = i8_type.const_int(30_000, false);
    let memory = builder.build_array_malloc(i8_type, memory_size, "memory")?;
    let memory = builder.build_pointer_cast(memory, i8_ptr_type, "memory")?;

    builder.build_memset(memory, 2, i8_type.const_int(0, false), memory_size)?;

    let i64_type = context.i64_type();
    let tape_pointer = builder.build_alloca(i64_type, "pointer")?;

    let mut loop_stack = vec![];

    for c in grouped {
        match c.as_str() {
            s if s.starts_with('+') => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = unsafe { builder.build_gep(i8_type, memory, &[i], "ptr")? };
                let value = builder.build_load(i8_type, cell, "value")?;
                let value = builder.build_int_add(value.into_int_value(), i8_type.const_int(s.len() as u64, false), "value")?;
                builder.build_store(cell, value)?;
            },
            s if s.starts_with('-') => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = unsafe { builder.build_gep(i8_type, memory, &[i], "ptr")? };
                let value = builder.build_load(i8_type, cell, "value")?;
                let value = builder.build_int_sub(value.into_int_value(), i8_type.const_int(s.len() as u64, false), "value")?;
                builder.build_store(cell, value)?;
            },
            ">" => {
                let pointer = builder.build_load(i64_type, tape_pointer, "pointer")?.into_int_value();
                let pointer = builder.build_int_add(pointer, i64_type.const_int(1, false), "pointer")?;
                builder.build_store(tape_pointer, pointer)?;
            },
            "<" => {
                let pointer = builder.build_load(i64_type, tape_pointer, "pointer")?.into_int_value();
                let pointer = builder.build_int_sub(pointer, i64_type.const_int(1, false), "pointer")?;
                builder.build_store(tape_pointer, pointer)?;
            },
            "[" => {
                let loop_start = context.append_basic_block(main_func, "loop_start");
                let loop_end = context.append_basic_block(main_func, "loop_end");

                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();

                let cell = unsafe { builder.build_gep(i8_type, memory, &[i], "ptr")? };
                let value = builder.build_load(i8_type, cell, "value")?;
                let cond = builder.build_int_compare(inkwell::IntPredicate::EQ, value.into_int_value(), i8_type.const_int(0, false), "cond")?;
                builder.build_conditional_branch(cond, loop_end, loop_start)?;

                builder.position_at_end(loop_start);
                loop_stack.push((loop_start, loop_end));
            },
            "]" => {
                let (loop_start, loop_end) = loop_stack.pop().unwrap();

                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();

                let cell = unsafe { builder.build_gep(i8_type, memory, &[i], "ptr")? };
                let value = builder.build_load(i8_type, cell, "value")?;
                let cond = builder.build_int_compare(inkwell::IntPredicate::NE, value.into_int_value(), i8_type.const_int(0, false), "cond")?;
                builder.build_conditional_branch(cond, loop_start, loop_end)?;

                builder.position_at_end(loop_end);
            },
            "." => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = unsafe { builder.build_gep(i8_type, memory, &[i], "ptr")? };
                let value = builder.build_load(i8_type, cell, "value")?;
                builder.build_call(putchar_func, &[value.into()], "putchar")?;
            },
            "," => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = unsafe { builder.build_gep(i8_type, memory, &[i], "ptr")? };
                let value = builder.build_call(getchar_func, &[], "getchar")?.try_as_basic_value().left().unwrap();
                builder.build_store(cell, value)?;
            },
            s => unreachable!("unexpected token: {}", s),
        }
    }

    builder.build_free(memory)?;
    builder.build_return(Some(&i8_type.const_int(0, false)))?;

    Target::initialize_all(&inkwell::targets::InitializationConfig::default());
    
    let opt = match args.opt.as_deref() {
        Some("0") => inkwell::OptimizationLevel::None,
        Some("1") => inkwell::OptimizationLevel::Less,
        Some("2") => inkwell::OptimizationLevel::Default,
        Some("3") => inkwell::OptimizationLevel::Aggressive,
        _ => {
            println!("Optimization level invalid or not specified, defaulting to --opt 2");
            inkwell::OptimizationLevel::Default
        }
    };
    let reloc = inkwell::targets::RelocMode::Default;
    let code_model = inkwell::targets::CodeModel::Default;

    if let Some(triple_str) = args.target {
        println!("Compiling for target: {}", triple_str);
        let triple = TargetTriple::create(&triple_str);
        let target = Target::from_triple(&triple)
            .map_err(|e| format!("could not create target: {}", e))?; 

        let arch = triple_str
            .split('-')
            .next().unwrap()
            .replace("_", "-");

        let target_machine = target.create_target_machine(
            &triple,
            &arch,
            "",
            opt,
            reloc,
            code_model,
        ).ok_or(format!("could not create target machine for target: {}", triple_str))?;

        let output = Path::new(module_name).with_extension("o");
        target_machine.write_to_file(&module, inkwell::targets::FileType::Object, &output)
            .map_err(|e| format!("could not write object file: {}", e))?;

        println!("Wrote module to object file: {}", output.display());
        return Ok(());
    }

    let output = Path::new(module_name).with_extension("ll");
    module.print_to_file(&output)
        .map_err(|e| format!("could not write LLVM IR file: {}", e))?;

    println!("Wrote module to LLVM IR file: {}", output.display());

    Ok(())
}
