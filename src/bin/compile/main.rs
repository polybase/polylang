use std::io::Read;

fn main() {
    let mut code = String::new();
    std::io::stdin().read_to_string(&mut code).unwrap();

    let mut contract_name = None;
    let mut function_name = "main".to_string();
    let mut args = Vec::<u32>::new();

    for arg in std::env::args().skip(1) {
        match arg.split_once(':') {
            Some((key, value)) => match key {
                "contract" => contract_name = Some(value.to_string()),
                "function" => function_name = value.to_string(),
                "arg" => args.push(value.parse().unwrap()),
                _ => panic!("unknown argument: {}", key),
            },
            None => panic!("invalid argument: {}", arg),
        }
    }

    let program = polylang::parse(&code).unwrap_or_else(|e| panic!("{}", e.message));
    let miden_code =
        polylang::compiler::compile(program, contract_name.as_deref(), &function_name, &args);
    println!("{}", miden_code);
}
