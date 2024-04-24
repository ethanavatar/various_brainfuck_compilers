
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: inkwell_bfc <file>");
        std::process::exit(1);
    }

    let file = std::fs::read_to_string(&args[1]).unwrap();

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

    let module = context.create_module("bfc");
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
                let cell = get_pointer_to_array_index(&context, &builder, memory, i)?;
                let value = builder.build_load(i8_type, cell, "value")?;
                let value = builder.build_int_add(value.into_int_value(), i8_type.const_int(s.len() as u64, false), "value")?;
                builder.build_store(cell, value)?;
            },
            s if s.starts_with('-') => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = get_pointer_to_array_index(&context, &builder, memory, i)?;
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

                let cell = get_pointer_to_array_index(&context, &builder, memory, i)?;
                let value = builder.build_load(i8_type, cell, "value")?;
                let cond = builder.build_int_compare(inkwell::IntPredicate::EQ, value.into_int_value(), i8_type.const_int(0, false), "cond")?;
                builder.build_conditional_branch(cond, loop_end, loop_start)?;

                builder.position_at_end(loop_start);
                loop_stack.push((loop_start, loop_end));
            },
            "]" => {
                let (loop_start, loop_end) = loop_stack.pop().unwrap();

                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();

                let cell = get_pointer_to_array_index(&context, &builder, memory, i)?;
                let value = builder.build_load(i8_type, cell, "value")?;
                let cond = builder.build_int_compare(inkwell::IntPredicate::NE, value.into_int_value(), i8_type.const_int(0, false), "cond")?;
                builder.build_conditional_branch(cond, loop_start, loop_end)?;

                builder.position_at_end(loop_end);
            },
            "." => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = get_pointer_to_array_index(&context, &builder, memory, i)?;
                let value = builder.build_load(i8_type, cell, "value")?;
                builder.build_call(putchar_func, &[value.into()], "putchar")?;
            },
            "," => {
                let i = builder.build_load(i64_type, tape_pointer, "i")?.into_int_value();
                let cell = get_pointer_to_array_index(&context, &builder, memory, i)?;
                let value = builder.build_call(getchar_func, &[], "getchar")?.try_as_basic_value().left().unwrap();
                builder.build_store(cell, value)?;
            },
            s => unreachable!("unexpected token: {}", s),
        }
    }

    builder.build_return(Some(&i8_type.const_int(0, false)))?;
    let s = module.print_to_string().to_string();

    println!("{}", s);

    Ok(())
}

fn get_pointer_to_array_index<'a>(
    context: &'a inkwell::context::Context,
    builder: &'a inkwell::builder::Builder,
    array: inkwell::values::PointerValue<'a>,
    index: inkwell::values::IntValue<'a>,
) -> Result<inkwell::values::PointerValue<'a>, inkwell::builder::BuilderError> {
    unsafe {
        builder.build_gep(context.i8_type(), array, &[index], "ptr")
    }
}
