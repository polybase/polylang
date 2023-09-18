use snafu::Snafu;

#[derive(Snafu, Debug)]
#[snafu(visibility(pub))]
pub enum ErrorKind {
    #[snafu(display("{source}"))]
    Wrapped { source: Box<dyn std::error::Error> },
    #[snafu(display("invalid address 0x{addr:x} for {type_name}"))]
    InvalidAddress { addr: u64, type_name: &'static str },
    #[snafu(display("cannot parse {type_name} from {input} ({source})"))]
    Parse {
        type_name: &'static str,
        input: String,
        source: Box<dyn std::error::Error>,
    },
    #[snafu(display("{type_name} {item} not found"))]
    NotFound {
        type_name: &'static str,
        item: String,
    },
    #[snafu(display("type mismatch: {context}"))]
    TypeMismatch { context: String },
    #[snafu(display("incorrect number of arguments {found} but expected {expected}"))]
    ArgumentsCount { found: usize, expected: usize },
    #[snafu(display(
        "stack depth is too small found {stack_len}{}",
        if let Some(expected) = expected {
            format!(" but expected {expected}")
        } else {
            "".to_string()
        }
    ))]
    Stack {
        stack_len: usize,
        expected: Option<usize>,
    },
    #[snafu(display("{msg}"))]
    Simple { msg: String },
    #[snafu(display("{context} >> {source}"))]
    Nested {
        context: String,
        source: Box<super::Error>,
    },
    #[snafu(display("i/o error: {source}"))]
    Io { source: std::io::Error },
    #[snafu(display("{context} is not implemented yet"))]
    NotImplemented { context: String },
}
