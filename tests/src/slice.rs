use super::*;

fn run_slice(
    arr: serde_json::Value,
    start: Option<u32>,
    end: Option<u32>,
) -> Result<abi::Value, error::Error> {
    let code = r#"
        @public
        collection Account {
            id: string;
            arr: number[];
            sliced: number[];

            slice2(start: u32, end: u32) {
                this.sliced = this.arr.slice(start, end);
            }

            slice1(start: u32) {
                this.sliced = this.arr.slice(start);
            }

            slice0() {
                this.sliced = this.arr.slice();
            }
        }
    "#;

    let (function_name, args) = match (start, end) {
        (Some(s), Some(e)) => ("slice2", vec![serde_json::json!(s), serde_json::json!(e)]),
        (Some(s), None) => ("slice1", vec![serde_json::json!(s)]),
        (None, None) => ("slice0", vec![]),
        _ => panic!("Unsupported argument combination"),
    };

    let (abi, output) = run(
        code,
        "Account",
        function_name,
        serde_json::json!({
            "id": "test",
            "arr": arr.clone(),
            "sliced": [],
        }),
        args,
        None,
        HashMap::new(),
    )?;

    let this = output.this(&abi)?;
    match this {
        abi::Value::StructValue(fields) => {
            let original_arr = fields.iter().find(|(k, _)| k == "arr").unwrap().1.clone();
            let sliced = fields
                .iter()
                .find(|(k, _)| k == "sliced")
                .unwrap()
                .1
                .clone();

            // Asserting the original array here
            assert_eq!(
                original_arr,
                abi::Value::Array(
                    arr.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| abi::Value::Float32(v.as_f64().unwrap() as f32))
                        .collect()
                )
            );

            Ok(sliced)
        }
        _ => panic!("unexpected value"),
    }
}

#[test]
fn slice_with_both_args() {
    // [1, 2, 3, 4, 5].slice(1, 3) = [2, 3]
    let sliced = run_slice(serde_json::json!([1, 2, 3, 4, 5]), Some(1), Some(3)).unwrap();
    assert_eq!(
        sliced,
        abi::Value::Array(vec![abi::Value::Float32(2.), abi::Value::Float32(3.),])
    );
}

#[test]
fn slice_with_only_start() {
    // [1, 2, 3, 4, 5].slice(2) = [3, 4, 5]
    let sliced = run_slice(serde_json::json!([1, 2, 3, 4, 5]), Some(2), None).unwrap();
    assert_eq!(
        sliced,
        abi::Value::Array(vec![
            abi::Value::Float32(3.),
            abi::Value::Float32(4.),
            abi::Value::Float32(5.),
        ])
    );
}

#[test]
fn slice_with_no_args() {
    // [1, 2, 3, 4, 5].slice() = [1, 2, 3, 4, 5]
    let sliced = run_slice(serde_json::json!([1, 2, 3, 4, 5]), None, None).unwrap();
    assert_eq!(
        sliced,
        abi::Value::Array(vec![
            abi::Value::Float32(1.),
            abi::Value::Float32(2.),
            abi::Value::Float32(3.),
            abi::Value::Float32(4.),
            abi::Value::Float32(5.),
        ])
    );
}
