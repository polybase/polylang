use super::*;

fn run_unshift(
    arr: serde_json::Value,
    elems: Vec<serde_json::Value>,
) -> Result<(abi::Value, abi::Value), error::Error> {
    let code = r#"
        @public
        collection Account {
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

#[test]
fn unshift_single_element() {
    assert_eq!(
        run_unshift(serde_json::json!([2, 3, 4]), vec![serde_json::json!(1)]).unwrap(),
        (
            abi::Value::Array(vec![
                abi::Value::Float32(1.),
                abi::Value::Float32(2.),
                abi::Value::Float32(3.),
                abi::Value::Float32(4.),
            ]),
            abi::Value::UInt32(4),
        ),
    );
}

#[test]
fn unshift_two_elements() {
    assert_eq!(
        run_unshift(
            serde_json::json!([3, 4, 5]),
            vec![serde_json::json!(1), serde_json::json!(2)]
        )
        .unwrap(),
        (
            abi::Value::Array(vec![
                abi::Value::Float32(1.),
                abi::Value::Float32(2.),
                abi::Value::Float32(3.),
                abi::Value::Float32(4.),
                abi::Value::Float32(5.),
            ]),
            abi::Value::UInt32(5),
        ),
    );
}

#[test]
fn unshift_empty_array() {
    assert_eq!(
        run_unshift(serde_json::json!([]), vec![serde_json::json!(1)]).unwrap(),
        (
            abi::Value::Array(vec![abi::Value::Float32(1.)]),
            abi::Value::UInt32(1),
        ),
    );
}
