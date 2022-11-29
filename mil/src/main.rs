use std::io::{Read, Write};

mod compiler;

fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let mut code = String::new();
    std::io::stdin().read_to_string(&mut code).unwrap();

    match arg.as_str() {
        "compile" => {
            let (code, graph) = compiler::compile(&code);
            println!("{}", code);

            let mut file = std::fs::File::create("graph.dot").unwrap();
            file.write_all(graph.as_bytes()).unwrap();
        }
        "graph" => {
            // compiler::graph(code);
        }
        _ => panic!("Unknown command"),
    }
}
