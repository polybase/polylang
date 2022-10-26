use std::io::Read;

fn main() {
    let mut code = String::new();
    std::io::stdin().read_to_string(&mut code).unwrap();

    let program = polylang::parse(&code).unwrap();
    let miden_code = polylang::compiler::compile(program, None, "main", &[]);
    println!("{}", miden_code);
}
