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

struct Args {
    advice_tape: Vec<u64>,
}

impl Args {
    fn parse(args: std::env::Args) -> Result<Self, String> {
        let mut args = args.skip(1);
        let mut advice_tape = Vec::new();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--advice-tape" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("missing value for argument {}", arg))?;

                    let values = value
                        .split(',')
                        .map(|s| s.parse::<u64>())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| format!("invalid value for argument {}: {}", arg, e))?;

                    advice_tape.extend(values);
                }
                _ => return Err(format!("unknown argument: {}", arg)),
            }
        }
        Ok(Self { advice_tape })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse(std::env::args())?;

    let mut masm_code = String::new();
    std::io::stdin().read_to_string(&mut masm_code)?;

    let assembler =
        miden::Assembler::new().with_module_provider(miden_stdlib::StdLibrary::default());
    let program = assembler
        .compile(&masm_code)
        .expect("Failed to compile miden assembly");

    let mut process = miden_processor::Process::new_debug(
        assembler.kernel(),
        miden::ProgramInputs::new(&[], &args.advice_tape, vec![]).unwrap(),
    );
    let execution_result = process.execute(&program);
    let (_system, _decoder, stack, _range_checker, chiplets) = process.to_components();

    let get_mem_value = |addr| {
        chiplets
            .get_mem_value(0, addr)
            .map(|word| mont_red_cst(word[0].inner() as _))
    };
    let read_string = |len: u64, data_ptr: u64| {
        let mut str_bytes = Vec::new();
        for i in 0..len {
            let c = get_mem_value(data_ptr + i).unwrap() as u8;
            str_bytes.push(c);
        }

        String::from_utf8(str_bytes).unwrap()
    };

    let mut log_messages = Vec::new();
    let (mut prev, mut str_ptr) = (get_mem_value(4), get_mem_value(5));
    loop {
        if str_ptr == Some(0) || str_ptr == None {
            break;
        }

        let len = get_mem_value(str_ptr.unwrap()).unwrap();
        let data_ptr = get_mem_value(str_ptr.unwrap() + 1).unwrap();
        let str = read_string(len, data_ptr);
        log_messages.push(str);

        str_ptr = get_mem_value(prev.unwrap() + 1);
        prev = get_mem_value(prev.unwrap());
    }
    log_messages.reverse();

    for msg in log_messages {
        println!("Log: {}", msg);
    }

    match execution_result {
        Ok(output) => {
            println!("Output: {:?}", output);
            Ok(())
        }
        Err(miden::ExecutionError::FailedAssertion(_)) => {
            println!("Output: {:?}", stack.get_outputs());

            // read the error string out from the memory
            let str_len = get_mem_value(1).ok_or_else(|| "Got an error, but no error string")?;
            let str_data_ptr = get_mem_value(2).unwrap();

            if str_data_ptr == 0 {
                Err("Foreign (not from our language) assertion failed".into())
            } else {
                let mut error_str_bytes = Vec::new();
                for i in 0..str_len {
                    let c = get_mem_value(str_data_ptr + i).unwrap() as u8;
                    error_str_bytes.push(c);
                }

                let error_str = String::from_utf8(error_str_bytes).unwrap();
                Err(format!("Assertion failed: {}", read_string(str_len, str_data_ptr)).into())
            }
        }
        Err(e) => Err(format!("Execution error: {:?}", e).into()),
    }
}
