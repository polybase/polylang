pub mod kind;
pub mod span;

use snafu::Snafu;

use kind::ErrorKind;

pub mod prelude {
    pub use super::kind::*;
    pub use super::{
        ensure_eq_type, maybe_start, Error, OptionExt2, Result, ResultExt2, ResultExt3,
    };
    pub use ::snafu::{ensure, whatever, OptionExt, ResultExt, Whatever};
}

pub type Result<T, E = Error> = ::core::result::Result<T, E>;

#[derive(Debug, Snafu, derive_more::Deref)]
#[snafu(display("{kind}{}", self.print_source()))]
pub struct Error {
    #[deref]
    kind: ErrorKind,
    source_code: Option<String>,
    span: Option<span::Span>,
}

impl Error {
    pub fn add_source(self, source: impl Into<String>) -> Self {
        Self {
            source_code: Some(source.into()),
            ..self
        }
    }

    fn print_source(&self) -> impl std::fmt::Display + '_ {
        if let Some((source, span)) = self.source_code.as_ref().zip(self.span.as_ref()) {
            assert!(source.len() >= span.end);
            assert!(span.end >= span.start);

            let mut lines = source.lines();
            let mut char_count = 0;
            let (start_line_no, start_line_sym) = lines
                .by_ref()
                .enumerate()
                .find(|(_, line)| {
                    char_count += line.len();
                    let found = char_count >= span.start;
                    if !found {
                        // new line char
                        char_count += 1;
                    }
                    found
                })
                // We want to show lines/symbols starting from one, therefore +1 everywhere.
                //
                // For the symbol position it's a little bit tricky:
                // 1. `char_count` is actually `line_position_char + line.len()`
                // 2. `span.start` is actually `line_position_char + sym_position_within_line`
                // Therefore `sym_position_within_line = span.start - char_count + line.len()`
                .map(|(no, line)| (no + 1, line.len() + span.start - char_count + 1))
                .unwrap();

            let (end_line_no, end_line_sym) = if char_count >= span.end {
                (start_line_no, start_line_sym + span.end - span.start)
            } else {
                lines
                    .enumerate()
                    .find(|(_, line)| {
                        char_count += line.len();
                        let found = char_count >= span.end;
                        if !found {
                            // new line char
                            char_count += 1;
                        }
                        found
                    })
                    .map(|(no, line)| (no + start_line_no, line.len() + span.end - char_count + 1))
                    .unwrap()
            };
            let line_fmt = if start_line_no == end_line_no {
                format!("{start_line_no}:{start_line_sym}..{end_line_sym}")
            } else {
                format!("{start_line_no}:{start_line_sym}..{end_line_no}:{end_line_sym}")
            };

            format!(
                "\n\tsource `{}` at line {line_fmt}",
                &source[span.start..=span.end]
            )
        } else {
            String::new()
        }
    }

    pub fn unimplemented(context: String) -> Self {
        ErrorKind::NotImplemented { context }.into()
    }

    pub fn simple(msg: impl Into<String>) -> Self {
        ErrorKind::Simple { msg: msg.into() }.into()
    }

    pub fn wrapped<E: std::error::Error + 'static>(source: Box<E>) -> Self {
        ErrorKind::Wrapped { source }.into()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            kind,
            source_code: None,
            span: span::get(),
        }
    }
}

#[macro_export]
macro_rules! ensure_eq_type {
    ($field:expr, $type_expected:pat) => {
        $crate::prelude::ensure!(
            matches!($field.type_, $type_expected),
            $crate::kind::TypeMismatchSnafu {
                context: format!(
                    "{} expected to be {:?} but found {:?}",
                    stringify!($field),
                    stringify!($type_expected),
                    $field.type_
                )
            }
        );
    };
    ($field:expr, @$type_expected:expr) => {
        $crate::prelude::ensure!(
            &$field.type_ == $type_expected,
            $crate::kind::TypeMismatchSnafu {
                context: format!(
                    "{} expected to be {:?} but found {:?}",
                    stringify!($field),
                    $type_expected,
                    $field.type_
                )
            }
        );
    };
    (@$type_got:expr, @$type_expected:expr) => {
        $crate::prelude::ensure!(
            $type_got == $type_expected,
            $crate::kind::TypeMismatchSnafu {
                context: format!(
                    "assertion of type {:?} equals to {:?}",
                    $type_got, $type_expected
                )
            }
        );
    };
    (@$type_got:expr, pat $type_expected:pat) => {
        $crate::prelude::ensure!(
            matches!($type_got, $type_expected),
            $crate::kind::TypeMismatchSnafu {
                context: format!(
                    "assertion of type {:?} equals to {:?}",
                    $type_got,
                    stringify!($type_expected)
                )
            }
        );
    };
}

pub trait OptionExt2<T> {
    fn parse_err(self, reason: &'static str, type_name: &'static str, input: &str) -> Result<T>;

    fn not_found(self, type_name: &'static str, item: &str) -> Result<T>;
}

impl<T> OptionExt2<T> for Option<T> {
    fn parse_err(self, reason: &'static str, type_name: &'static str, input: &str) -> Result<T> {
        self.ok_or_else(|| ErrorKind::Parse {
            type_name,
            input: input.to_string(),
            source: reason.into(),
        })
        .map_err(Into::into)
    }

    fn not_found(self, type_name: &'static str, item: &str) -> Result<T> {
        self.ok_or_else(|| ErrorKind::NotFound {
            type_name,
            item: item.to_string(),
        })
        .map_err(Into::into)
    }
}

pub trait ResultExt2<T, E> {
    fn wrap_err(self) -> Result<T>;
    fn parse_err(self, type_name: &'static str, input: &str) -> Result<T>;
}

pub trait ResultExt3<T> {
    fn nest_err<F>(self, make_context: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E: std::error::Error + 'static> ResultExt2<T, E> for Result<T, E> {
    fn wrap_err(self) -> Result<T> {
        self.map_err(Box::new).map_err(Error::wrapped)
    }

    fn parse_err(self, type_name: &'static str, input: &str) -> Result<T> {
        self.map_err(Box::new)
            .map_err(|source| ErrorKind::Parse {
                type_name,
                input: input.to_string(),
                source,
            })
            .map_err(Into::into)
    }
}

impl<T> ResultExt3<T> for Result<T> {
    fn nest_err<F>(self, make_context: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        use snafu::ResultExt;
        self.map_err(Box::new)
            .with_context(|_| kind::NestedSnafu {
                context: make_context(),
            })
            .map_err(Into::into)
    }
}

#[cfg(never)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use test_case::test_case;

    fn compile_err(source: &str) -> Error {
        let program = polylang_parser::parse(&source).unwrap();

        crate::compiler::compile(program, None, "f")
            .map_err(|e| e.add_source(source))
            .unwrap_err()
    }

    static SPAN_LOCK: Mutex<()> = Mutex::new(());

    #[test_case(
        "function f(a: number) {assert(a);}",
        concat!("incorrect number of arguments 1 but expected 2",
        "\n\tsource `assert(a);` at line 1:24..33");
        "single line"
    )]
    #[test_case(
        "function f(a: number) {\nassert(a);\n}",
        concat!("incorrect number of arguments 1 but expected 2",
        "\n\tsource `assert(a);` at line 2:1..10");
        "single line at start"
    )]
    #[test_case(
        "function f(a: number) {\nassert(a)\n;}",
        concat!("incorrect number of arguments 1 but expected 2",
        "\n\tsource `assert(a)\n` at line 2:1..10");
        "single line whole"
    )]
    #[test_case(
        "function f(a: number) {\nassert(\na)\n;}",
        concat!("incorrect number of arguments 1 but expected 2",
        "\n\tsource `assert(\na)\n` at line 2:1..3:1");
        "two whole lines"
    )]
    fn fmt_error_span(invalid_source: &str, expected_msg: &str) {
        // Spans are set in global, if multithreaded needed
        // consider using thread_local/task_local.
        // Then this mutex should not be needed.
        let _span = SPAN_LOCK.lock();

        let err = compile_err(invalid_source);
        assert_eq!(&err.to_string(), expected_msg);
    }
}
