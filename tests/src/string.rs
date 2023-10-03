use super::*;

fn run_fn(f: &str, result: &str, s1: &str, s2: &str) -> Result<abi::Value, error::Error> {
    let code = r#"
        contract Account {
            result_bool: boolean;
            result_i32: i32;

            startsWith(x: string, y: string) {
                this.result_bool = x.startsWith(y);
            }

            includes(x: string, y: string) {
                this.result_bool = x.includes(y);
            }

            indexOf(x: string, y: string) {
                this.result_i32 = x.indexOf(y);
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        f,
        serde_json::json!({
            "result_bool": false,
            "result_i32": 123,
        }),
        vec![
            serde_json::Value::String(s1.into()),
            serde_json::Value::String(s2.into()),
        ],
        None,
        HashMap::new(),
    )?;

    let this = output.this(&abi)?;
    match this {
        abi::Value::StructValue(fields) => {
            let result = fields.iter().find(|(k, _)| k == result).unwrap().1.clone();
            Ok(result)
        }
        _ => panic!("unexpected value"),
    }
}

fn run_starts_with(s1: &str, s2: &str) -> Result<abi::Value, error::Error> {
    run_fn("startsWith", "result_bool", s1, s2)
}

fn run_includes(s1: &str, s2: &str) -> Result<abi::Value, error::Error> {
    run_fn("includes", "result_bool", s1, s2)
}

fn run_index_of(s1: &str, s2: &str) -> Result<abi::Value, error::Error> {
    run_fn("indexOf", "result_i32", s1, s2)
}

#[test_case::test_case("qwe", "qwe", true; "exact match")]
#[test_case::test_case("qwe", "ewq", false; "same size mismatch")]
#[test_case::test_case("qweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqwe", "qweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqweqwe", true; "exact long match")]
#[test_case::test_case("𝔍К𝓛𝓜ƝȎ𝚸𝑄Ṛ𝓢ṮṺƲᏔꓫ𝚈𝚭𝜶Ꮟ", "𝔍К𝓛𝓜ƝȎ𝚸𝑄Ṛ𝓢ṮṺƲᏔꓫ𝚈𝚭𝜶Ꮟ", true; "unicode")]
#[test_case::test_case("qwer", "qwe", true; "substring match")]
#[test_case::test_case("qwe", "qwef", false; "second larger")]
#[test_case::test_case("qwert", "wer", false; "substring but not start")]
#[test_case::test_case("", "", true; "empty strings")]
fn test_starts_with(s1: &str, s2: &str, expected: bool) {
    let result = run_starts_with(s1, s2).unwrap();
    assert_eq!(result, abi::Value::Boolean(expected));
}

#[test_case::test_case("qwe", "qwe", true; "exact match")]
#[test_case::test_case("qwe", "ewq", false; "same size mismatch")]
#[test_case::test_case("qwerty", "qwert", true; "substring start")]
#[test_case::test_case("asdqwe", "dqwe", true; "substring end")]
#[test_case::test_case("asqwerty", "we", true; "substring middle")]
#[test_case::test_case("𝔍К𝓛𝓜ƝȎ𝚸𝑄Ṛ𝓢ṮṺƲᏔꓫ𝚈𝚭𝜶Ꮟ", "𝑄Ṛ𝓢ṮṺƲᏔꓫ𝚈", true; "unicode")]
#[test_case::test_case("qwe", "qwef", false; "second larger")]
#[test_case::test_case("", "", true; "empty strings")]
fn test_includes(s1: &str, s2: &str, expected: bool) {
    let result = run_includes(s1, s2).unwrap();
    assert_eq!(result, abi::Value::Boolean(expected));
}

#[test_case::test_case("qwe", "qwe", 0; "exact match")]
#[test_case::test_case("qwe", "ewq", -1; "same size mismatch")]
#[test_case::test_case("qwerty", "qwert", 0; "substring start")]
#[test_case::test_case("asdqwe", "dqwe", 2; "substring end")]
#[test_case::test_case("asqwerty", "we", 3; "substring middle")]
// TODO: now it returns byte index, not char codepoint.
// #[test_case::test_case("𝔍К𝓛𝓜ƝȎ𝚸𝑄Ṛ𝓢ṮṺƲᏔꓫ𝚈𝚭𝜶Ꮟ", "𝑄Ṛ𝓢ṮṺƲᏔꓫ𝚈", 7; "unicode")]
#[test_case::test_case("qwe", "qwef", -1; "second larger")]
#[test_case::test_case("", "", 0; "empty strings")]
fn test_index_of(s1: &str, s2: &str, expected: i32) {
    let result = run_index_of(s1, s2).unwrap();
    assert_eq!(result, abi::Value::Int32(expected));
}
