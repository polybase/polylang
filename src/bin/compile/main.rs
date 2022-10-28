use std::{collections::HashMap, io::Read};

use polylang::compiler::CompileTimeArg;

fn main() {
    let mut code = String::new();
    std::io::stdin().read_to_string(&mut code).unwrap();

    let mut contract_name = None;
    let mut function_name = "main".to_string();
    let mut args = Vec::<CompileTimeArg>::new();
    let mut this = None;

    for arg in std::env::args().skip(1) {
        match arg.split_once(':') {
            Some((key, value)) => match key {
                "contract" => contract_name = Some(value.to_string()),
                "function" => function_name = value.to_string(),
                "arg" => args.push(CompileTimeArg::U32(value.parse().unwrap())),
                "struct_arg" => {
                    let mut arg = HashMap::new();

                    for value in value.split(',') {
                        match value.split_once('=') {
                            Some((key, value)) => {
                                arg.insert(key.to_string(), value.parse().unwrap());
                            }
                            None => panic!("Invalid struct arg: {}", value),
                        }
                    }

                    args.push(CompileTimeArg::Record(arg));
                }
                "this" => {
                    let mut this_map = HashMap::new();

                    for value in value.split(',') {
                        match value.split_once('=') {
                            Some((key, value)) => {
                                this_map.insert(key.to_string(), value.parse().unwrap());
                            }
                            None => panic!("Invalid this arg: {}", value),
                        }
                    }

                    this = Some(this_map);
                }
                _ => panic!("unknown argument: {}", key),
            },
            None => panic!("invalid argument: {}", arg),
        }
    }

    let program = polylang::parse(&code).unwrap_or_else(|e| panic!("{}", e.message));
    let miden_code = polylang::compiler::compile(
        program,
        contract_name.as_deref(),
        &function_name,
        &args,
        this,
    );
    println!("{}", miden_code);
}
