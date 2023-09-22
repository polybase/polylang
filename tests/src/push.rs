use super::*;

fn run_push(
    arr: serde_json::Value,
    element: serde_json::Value,
) -> Result<(abi::Value, abi::Value), error::Error> {
    let code = r#"
        @public
        contract Account {
            id: string;
            arr: number[];
            result: number;

            push(element: number) {
                this.result = this.arr.push(element);
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "push",
        serde_json::json!({
            "id": "test",
            "arr": arr.clone(),
            "result": 0,
        }),
        vec![element],
        None,
        HashMap::new(),
    )?;

    let this = output.this(&abi)?;
    match this {
        abi::Value::StructValue(fields) => {
            let pushed_arr = fields.iter().find(|(k, _)| k == "arr").unwrap().1.clone();
            let result = fields
                .iter()
                .find(|(k, _)| k == "result")
                .unwrap()
                .1
                .clone();
            Ok((pushed_arr, result))
        }
        _ => panic!("unexpected value"),
    }
}

fn run_push17(
    arr: serde_json::Value,
    elements: [serde_json::Value; 17],
) -> Result<(abi::Value, abi::Value), error::Error> {
    let code = r#"
        @public
        contract Account {
            id: string;
            arr: number[];
            result: number;

            push17(e1: number, e2: number, e3: number, e4: number, e5: number, e6: number, e7: number, e8: number, e9: number, e10: number, e11: number, e12: number, e13: number, e14: number, e15: number, e16: number, e17: number) {
                this.arr.push(e1);
                this.arr.push(e2);
                this.arr.push(e3);
                this.arr.push(e4);
                this.arr.push(e5);
                this.arr.push(e6);
                this.arr.push(e7);
                this.arr.push(e8);
                this.arr.push(e9);
                this.arr.push(e10);
                this.arr.push(e11);
                this.arr.push(e12);
                this.arr.push(e13);
                this.arr.push(e14);
                this.arr.push(e15);
                this.arr.push(e16);
                this.result = this.arr.push(e17);
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "push17",
        serde_json::json!({
            "id": "test",
            "arr": arr.clone(),
            "result": 0,
        }),
        elements.to_vec(),
        None,
        HashMap::new(),
    )?;

    let this = output.this(&abi)?;
    match this {
        abi::Value::StructValue(fields) => {
            let pushed_arr = fields.iter().find(|(k, _)| k == "arr").unwrap().1.clone();
            let result = fields
                .iter()
                .find(|(k, _)| k == "result")
                .unwrap()
                .1
                .clone();
            Ok((pushed_arr, result))
        }
        _ => panic!("unexpected value"),
    }
}

#[test]
fn test_push() {
    // [1, 2, 3].push(4) = [1, 2, 3, 4]
    let (original, result) = run_push(serde_json::json!([1, 2, 3]), serde_json::json!(4)).unwrap();
    assert_eq!(
        original,
        abi::Value::Array(vec![
            abi::Value::Float32(1.),
            abi::Value::Float32(2.),
            abi::Value::Float32(3.),
            abi::Value::Float32(4.),
        ])
    );
    assert_eq!(result, abi::Value::Float32(4.));

    // [].push(1) = [1]
    let (original, result) = run_push(serde_json::json!([]), serde_json::json!(1)).unwrap();
    assert_eq!(original, abi::Value::Array(vec![abi::Value::Float32(1.)]));
    assert_eq!(result, abi::Value::Float32(1.));
}

#[test]
fn test_push17() {
    // An array with 0 length is allocated with capacity 16.
    // We test that pushing 17 elements will cause the array to be reallocated.

    // [].push(1, 2, 3, ..., 17) = [1, 2, 3, ..., 17]
    let elements = [
        serde_json::json!(1),
        serde_json::json!(2),
        serde_json::json!(3),
        serde_json::json!(4),
        serde_json::json!(5),
        serde_json::json!(6),
        serde_json::json!(7),
        serde_json::json!(8),
        serde_json::json!(9),
        serde_json::json!(10),
        serde_json::json!(11),
        serde_json::json!(12),
        serde_json::json!(13),
        serde_json::json!(14),
        serde_json::json!(15),
        serde_json::json!(16),
        serde_json::json!(17),
    ];
    let (original, result) = run_push17(serde_json::json!([]), elements).unwrap();
    assert_eq!(
        original,
        abi::Value::Array(vec![
            abi::Value::Float32(1.),
            abi::Value::Float32(2.),
            abi::Value::Float32(3.),
            abi::Value::Float32(4.),
            abi::Value::Float32(5.),
            abi::Value::Float32(6.),
            abi::Value::Float32(7.),
            abi::Value::Float32(8.),
            abi::Value::Float32(9.),
            abi::Value::Float32(10.),
            abi::Value::Float32(11.),
            abi::Value::Float32(12.),
            abi::Value::Float32(13.),
            abi::Value::Float32(14.),
            abi::Value::Float32(15.),
            abi::Value::Float32(16.),
            abi::Value::Float32(17.),
        ])
    );
    assert_eq!(result, abi::Value::Float32(17.));
}
