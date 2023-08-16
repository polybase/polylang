use super::*;

fn run_splice(
    arr: serde_json::Value,
    start: u32,
    delete_count: u32,
) -> Result<(abi::Value, abi::Value), error::Error> {
    let code = r#"
        @public
        collection Account {
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

#[test]
fn test_splice_basic() {
    assert_eq!(
        run_splice(serde_json::json!([1, 2, 3, 4, 5]), 1, 2).unwrap(),
        (
            abi::Value::Array(vec![
                abi::Value::Float32(1.),
                abi::Value::Float32(4.),
                abi::Value::Float32(5.),
            ]),
            abi::Value::Array(vec![abi::Value::Float32(2.), abi::Value::Float32(3.),]),
        ),
    );
}

#[test]
fn test_splice_no_deletion() {
    assert_eq!(
        run_splice(serde_json::json!([1, 2, 3, 4, 5]), 1, 0).unwrap(),
        (
            abi::Value::Array(vec![
                abi::Value::Float32(1.),
                abi::Value::Float32(2.),
                abi::Value::Float32(3.),
                abi::Value::Float32(4.),
                abi::Value::Float32(5.),
            ]),
            abi::Value::Array(vec![]),
        ),
    );
}

#[test]
fn test_splice_start_out_of_bounds() {
    assert!(run_splice(serde_json::json!([1, 2, 3, 4, 5]), 6, 0).is_err());
}
