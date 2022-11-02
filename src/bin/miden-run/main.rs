use std::io::Read;

// Copied from https://github.com/novifinancial/winterfell/blob/1a1815adb51757e57f8f3844c51ff538e6c17a32/math/src/field/f64/mod.rs#L572
const fn mont_red_cst(x: u128) -> u64 {
    // See reference above for a description of the following implementation.
    let xl = x as u64;
    let xh = (x >> 64) as u64;
    let (a, e) = xl.overflowing_add(xl << 32);

    let b = a.wrapping_sub(a >> 32).wrapping_sub(e as u64);

    let (r, c) = xh.overflowing_sub(b);
    r.wrapping_sub(0u32.wrapping_sub(c as u32) as u64)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut masm_code = String::new();
    std::io::stdin().read_to_string(&mut masm_code)?;

    let assembler =
        miden::Assembler::new().with_module_provider(miden_stdlib::StdLibrary::default());
    let program = assembler
        .compile(&masm_code)
        .expect("Failed to compile miden assembly");

    let mut process = miden_processor::Process::new_debug(miden::ProgramInputs::none());
    match process.execute(&program) {
        Ok(output) => {
            println!("Output: {:?}", output);
            Ok(())
        }
        Err(miden::ExecutionError::FailedAssertion(_)) => {
            let (_system, _decoder, stack, _range_checker, chiplets) = process.to_components();
            println!("Output: {:?}", stack.get_outputs());

            // read the error string out from the memory
            let get_mem_value = |addr| {
                chiplets
                    .get_mem_value(0, addr)
                    .map(|word| mont_red_cst(word[0].inner() as _))
            };

            let str_len = get_mem_value(1).ok_or_else(|| "Got an error, but no error string")?;
            let str_data_ptr = get_mem_value(2).unwrap();

            if str_data_ptr == 0 {
                return Err("Foreign (not from our language) assertion failed".into());
            } else {
                let mut error_str_bytes = Vec::new();
                for i in 0..str_len {
                    let c = get_mem_value(str_data_ptr + i).unwrap() as u8;
                    error_str_bytes.push(c);
                }

                let error_str = String::from_utf8(error_str_bytes).unwrap();
                return Err(format!("Assertion failed: {}", error_str).into());
            }
        }
        Err(e) => Err(format!("Execution error: {:?}", e).into()),
    }
}
