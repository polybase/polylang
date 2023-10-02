use super::*;
use test_case::test_case;

fn run_splice(
    arr: serde_json::Value,
    start: u32,
    delete_count: u32,
) -> Result<(abi::Value, abi::Value), error::Error> {
    let code = r#"
        contract Account {
            id: string;
            arr: number[];
            ret: number[];

            splice(start: u32, deleteCount: u32) {
                this.ret = this.arr.splice(start, deleteCount);
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "splice",
        serde_json::json!({
            "id": "test",
            "arr": arr,
            "ret": [],
        }),
        vec![serde_json::json!(start), serde_json::json!(delete_count)],
        None,
        HashMap::new(),
    )?;

    let this = output.this(&abi)?;
    let (arr, ret) = match this {
        abi::Value::StructValue(fields) => {
            let arr = fields.iter().find(|(k, _)| k == "arr").unwrap().1.clone();
            let ret = fields.iter().find(|(k, _)| k == "ret").unwrap().1.clone();
            (arr, ret)
        }
        _ => panic!("unexpected value"),
    };

    Ok((arr, ret))
}

#[test_case(
    serde_json::json!([1, 2, 3, 4, 5]),
    0,
    2,
    &[3., 4., 5.],
    &[1., 2.]
    ; "delete from start"
)]
#[test_case(
    serde_json::json!([1, 2, 3, 4, 5]),
    1,
    2,
    &[1., 4., 5.],
    &[2., 3.]
    ; "delete from middle"
)]
#[test_case(
    serde_json::json!([1, 2, 3, 4, 5]),
    0,
    0,
    &[1., 2., 3., 4., 5.],
    &[]
    ; "no delete"
)]
fn test_splice(
    arr: serde_json::Value,
    start: u32,
    delete_count: u32,
    expected_new_array: &[f32],
    expected_returned: &[f32],
) {
    let (arr, ret) = run_splice(arr, start, delete_count).unwrap();
    assert_eq!(
        arr,
        abi::Value::Array(
            expected_new_array
                .into_iter()
                .map(|n| abi::Value::Float32(*n))
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(
        ret,
        abi::Value::Array(
            expected_returned
                .into_iter()
                .map(|n| abi::Value::Float32(*n))
                .collect::<Vec<_>>()
        )
    );
}

#[test]
fn test_splice_start_out_of_bounds() {
    assert!(run_splice(serde_json::json!([1, 2, 3, 4, 5]), 6, 0).is_err());
}
