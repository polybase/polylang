use super::*;
use test_case::test_case;

fn run_unshift(
    arr: serde_json::Value,
    elems: Vec<serde_json::Value>,
) -> Result<(abi::Value, abi::Value), error::Error> {
    let code = r#"
        contract Account {
            id: string;
            arr: number[];
            len: u32;

            unshift1(elem1: number) {
                this.len = this.arr.unshift(elem1);
            }

            unshift2(elem1: number, elem2: number) {
                this.len = this.arr.unshift(elem1, elem2);
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        match elems.len() {
            1 => "unshift1",
            2 => "unshift2",
            _ => panic!("unexpected number of elements"),
        },
        serde_json::json!({
            "id": "test",
            "arr": arr,
            "len": 0,
        }),
        elems,
        None,
        HashMap::new(),
    )?;

    let this = output.this(&abi)?;
    let (arr, len) = match this {
        abi::Value::StructValue(fields) => {
            let arr = fields.iter().find(|(k, _)| k == "arr").unwrap().1.clone();
            let len = fields.iter().find(|(k, _)| k == "len").unwrap().1.clone();
            (arr, len)
        }
        _ => panic!("unexpected value"),
    };

    Ok((arr, len))
}

#[test_case(
    serde_json::json!([2, 3, 4]),
    vec![serde_json::json!(1)],
    &[1., 2., 3., 4.],
    4
    ; "unshift single element"
)]
#[test_case(
    serde_json::json!([3, 4, 5]),
    vec![serde_json::json!(1), serde_json::json!(2)],
    &[1., 2., 3., 4., 5.],
    5
    ; "unshift two elements"
)]
#[test_case(
    serde_json::json!([]),
    vec![serde_json::json!(1)],
    &[1.],
    1
    ; "unshift empty array"
)]
fn test_unshift(
    arr: serde_json::Value,
    elems: Vec<serde_json::Value>,
    expected_arr: &[f32],
    expected_len: u32,
) {
    let (arr, len) = run_unshift(arr, elems).unwrap();
    assert_eq!(
        arr,
        abi::Value::Array(
            expected_arr
                .into_iter()
                .map(|n| abi::Value::Float32(*n))
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(len, abi::Value::UInt32(expected_len));
}
