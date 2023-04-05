use std::{collections::HashMap, io::Read};

use polylang::compiler::CompileTimeArg;

fn main() {
    let mut code = String::new();
    std::io::stdin().read_to_string(&mut code).unwrap();

    let mut collection_name = None;
    let mut function_name = "main".to_string();

    for arg in std::env::args().skip(1) {
        match arg.split_once(':') {
            Some((key, value)) => match key {
                "collection" => collection_name = Some(value.to_string()),
                "function" => function_name = value.to_string(),
                _ => panic!("unknown argument: {}", key),
            },
            None => panic!("invalid argument: {}", arg),
        }
    }

    let program = polylang_parser::parse(&code).unwrap();

    let (miden_code, abi) =
        polylang::compiler::compile(program, collection_name.as_deref(), &function_name);
    println!("{}", miden_code);
    eprintln!("{}", serde_json::to_string(&abi).unwrap());
}
