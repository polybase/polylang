#![cfg(test)]
use abi::Parser;
use serde::de::Deserialize;
use std::collections::HashMap;

mod fixtures {
    use super::*;

    pub fn pk1() -> serde_json::Value {
        serde_json::json!({
            "kty": "EC",
            "crv": "secp256k1",
            "alg": "ES256K",
            "use": "sig",
            "x": "nnzHFO4bZ239bIuAo8t0wQwXH3fPwbKQnpWPzOptv0Q=",
            "y": "Z1-oY62A6q5kCRGfBuk6E3IrSUjPCK2F6_EwVhW22lY="
        })
    }

    pub fn pk1_key() -> abi::publickey::Key {
        abi::publickey::Key::deserialize(pk1()).unwrap()
    }

    pub fn pk2() -> serde_json::Value {
        serde_json::json!({
            "kty": "EC",
            "crv": "secp256k1",
            "alg": "ES256K",
            "use": "sig",
            "x": "nnzHFO4bZ239bIuAo8t0wQwXH3fPwbKQnpWPzOptv0Q=",
            // Z at the start was changed to Y
            "y": "Y1-oY62A6q5kCRGfBuk6E3IrSUjPCK2F6_EwVhW22lY="
        })
    }

    pub fn pk2_key() -> abi::publickey::Key {
        abi::publickey::Key::deserialize(pk2()).unwrap()
    }
}

fn run(
    polylang_code: &str,
    collection: &str,
    function: &str,
    this: serde_json::Value,
    args: Vec<serde_json::Value>,
    ctx_public_key: Option<abi::publickey::Key>,
    other_records: HashMap<String, Vec<serde_json::Value>>,
) -> Result<(abi::Abi, polylang_prover::RunOutput), error::Error> {
    let program = polylang::parse_program(polylang_code).unwrap();

    let (miden_code, abi) = polylang::compiler::compile(program, Some(collection), function)?;

    let program = polylang_prover::compile_program(&abi, &miden_code).unwrap();
    let this_abi_value = abi.this_type.clone().unwrap().parse(&this).unwrap();
    let this_hash =
        polylang_prover::hash_this(abi.this_type.clone().unwrap(), &this_abi_value).unwrap();
    let inputs = polylang_prover::Inputs {
        abi: abi.clone(),
        ctx_public_key,
        this,
        this_hash,
        args,
        other_records,
    };

    let (output, _) = polylang_prover::run(&program, &inputs)?;

    Ok((abi, output))
}

#[test]
fn call_public_collection() {
    let code = r#"
        @public
        collection Account {
            id: string;
            name: string;

            setName(name: string) {
                this.name = name;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "setName",
        serde_json::json!({
            "id": "",
            "name": "",
        }),
        vec![serde_json::json!("test")],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("".to_owned())),
            ("name".to_owned(), abi::Value::String("test".to_owned())),
        ])
    );
}

#[test]
fn call_any_call_collection() {
    let code = r#"
        @call
        collection Account {
            id: string;
            name: string;

            setName(name: string) {
                this.name = name;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "setName",
        serde_json::json!({
            "id": "",
            "name": "",
        }),
        vec![serde_json::json!("test")],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("".to_owned())),
            ("name".to_owned(), abi::Value::String("test".to_owned())),
        ])
    );
}

#[test]
fn call_constructor_no_auth() {
    let code = r#"
        collection Account {
            id: string;

            constructor (id: string) {
                this.id = id;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "constructor",
        serde_json::json!({
            "id": "",
        }),
        vec![serde_json::json!("id1")],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![(
            "id".to_owned(),
            abi::Value::String("id1".to_owned())
        )])
    );
}

#[test]
fn call_constructor_with_auth() {
    let code = r#"
        collection Account {
            id: string;
            pk: PublicKey;

            constructor (id: string) {
                this.id = id;
                if (ctx.publicKey)
                    this.pk = ctx.publicKey;
                else error("missing public key");
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "constructor",
        serde_json::json!({
            "id": "",
            "pk": fixtures::pk2(),
        }),
        vec![serde_json::json!("id1")],
        Some(fixtures::pk1_key()),
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("id1".to_owned())),
            ("pk".to_owned(), abi::Value::PublicKey(fixtures::pk1_key())),
        ])
    );
}

fn call_auth_public_key(use_correct_pk: bool) -> Result<(), Box<dyn std::error::Error>> {
    let code = r#"
        collection Account {
            id: string;
            pk: PublicKey;

            constructor (id: string, pk: PublicKey) {
                this.id = id;
                this.pk = pk;
            }

            @call(pk)
            changePk(newPk: PublicKey) {
                this.pk = newPk;
            }
        }
    "#;

    let old_pk = fixtures::pk1();
    let old_pk_key = fixtures::pk1_key();
    let new_pk = fixtures::pk2();
    let new_pk_key = fixtures::pk2_key();

    let (abi, output) = run(
        code,
        "Account",
        "changePk",
        serde_json::json!({
            "id": "test",
            "pk": old_pk,
        }),
        vec![new_pk],
        Some(if use_correct_pk {
            old_pk_key
        } else {
            new_pk_key.clone()
        }),
        HashMap::new(),
    )?;

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("test".to_owned())),
            ("pk".to_owned(), abi::Value::PublicKey(new_pk_key)),
        ]),
    );

    Ok(())
}

#[test]
fn call_auth_public_key_correct_pk() {
    call_auth_public_key(true).unwrap();
}

#[test]
fn call_auth_public_key_wrong_pk() {
    let err = call_auth_public_key(false).unwrap_err();
    assert!(err
        .to_string()
        .contains("You are not authorized to call this function"));
}

#[test]
fn call_auth_public_key_no_pk() {
    let code = r#"
        collection Account {
            id: string;
            pk: PublicKey;

            constructor (id: string, pk: PublicKey) {
                this.id = id;
                this.pk = pk;
            }

            @call(pk)
            changePk(newPk: PublicKey) {
                this.pk = newPk;
            }
        }
    "#;

    let err = run(
        code,
        "Account",
        "changePk",
        serde_json::json!({
            "id": "test",
            "pk": fixtures::pk1(),
        }),
        vec![fixtures::pk2()],
        None,
        HashMap::new(),
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("You are not authorized to call this function"));
}

#[test]
fn call_auth_public_key_allow_all() {
    let code = r#"
        collection Account {
            id: string;
            pk: PublicKey;

            constructor (id: string, pk: PublicKey) {
                this.id = id;
                this.pk = pk;
            }

            @call
            changePk(newPk: PublicKey) {
                this.pk = newPk;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "changePk",
        serde_json::json!({
            "id": "test",
            "pk": fixtures::pk1(),
        }),
        vec![fixtures::pk2()],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("test".to_owned())),
            ("pk".to_owned(), abi::Value::PublicKey(fixtures::pk2_key())),
        ]),
    );
}

#[test]
fn call_auth_no_directive() {
    let code = r#"
        collection Account {
            id: string;
            pk: PublicKey;

            constructor (id: string, pk: PublicKey) {
                this.id = id;
                this.pk = pk;
            }

            changePk(newPk: PublicKey) {
                this.pk = newPk;
            }
        }
    "#;

    let err = run(
        code,
        "Account",
        "changePk",
        serde_json::json!({
            "id": "test",
            "pk": fixtures::pk1(),
        }),
        vec![fixtures::pk2()],
        None,
        HashMap::new(),
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("You are not authorized to call this function"));
}

#[test]
fn call_collection_auth_any() {
    let code = r#"
        @call
        collection Account {
            id: string;
            pk: PublicKey;

            changePk(newPk: PublicKey) {
                this.pk = newPk;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "changePk",
        serde_json::json!({
            "id": "test",
            "pk": fixtures::pk1(),
        }),
        vec![fixtures::pk2()],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("test".to_owned())),
            ("pk".to_owned(), abi::Value::PublicKey(fixtures::pk2_key())),
        ]),
    );
}

fn call_auth_delegate(use_correct_pk: bool) -> Result<(), Box<dyn std::error::Error>> {
    let code = r#"
        collection User {
            id: string;
            @delegate
            pk: PublicKey;
        }

        collection Account {
            id: string;
            name: string;
            user: User;

            @call(user)
            changeName(name: string) {
                this.name = name;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "changeName",
        serde_json::json!({
            "id": "test",
            "name": "test",
            "user": {
                "id": "user1",
                "pk": fixtures::pk1(),
            },
        }),
        vec![serde_json::json!("test2")],
        Some(if use_correct_pk {
            fixtures::pk1_key()
        } else {
            fixtures::pk2_key()
        }),
        {
            let mut hm = HashMap::new();
            hm.insert(
                "User".to_owned(),
                vec![serde_json::json!({
                    "id": "user1",
                    "pk": fixtures::pk1(),
                })],
            );
            hm
        },
    )?;

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("test".to_owned())),
            ("name".to_owned(), abi::Value::String("test2".to_owned())),
            (
                "user".to_owned(),
                abi::Value::CollectionReference("user1".bytes().collect()),
            ),
        ]),
    );

    Ok(())
}

#[test]
fn call_auth_delegate_correct_pk() {
    call_auth_delegate(true).unwrap();
}

#[test]
fn call_auth_delegate_wrong_pk() {
    let err = call_auth_delegate(false).unwrap_err();
    assert!(err
        .to_string()
        .contains("You are not authorized to call this function"));
}

fn call_auth_literal_pk(use_correct_pk: bool) -> Result<(), Box<dyn std::error::Error>> {
    let key = fixtures::pk1_key().to_64_byte_hex();
    let code = format!(
        r#"
        collection Account {{
            id: string;
            name: string;

            @call(eth#{key})
            changeName(name: string) {{
                this.name = name;
            }}
        }}
    "#
    );

    let (abi, output) = run(
        &code,
        "Account",
        "changeName",
        serde_json::json!({
            "id": "test",
            "name": "test",
        }),
        vec![serde_json::json!("test2")],
        Some(if use_correct_pk {
            fixtures::pk1_key()
        } else {
            fixtures::pk2_key()
        }),
        HashMap::new(),
    )?;

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("test".to_owned())),
            ("name".to_owned(), abi::Value::String("test2".to_owned())),
        ]),
    );

    Ok(())
}

#[test]
fn call_auth_literal_pk_correct_pk() {
    call_auth_literal_pk(true).unwrap();
}

#[test]
fn call_auth_literal_pk_wrong_pk() {
    let err = call_auth_literal_pk(false).unwrap_err();
    assert!(err
        .to_string()
        .contains("You are not authorized to call this function"));
}

#[test]
fn call_auth_literal_compressed() {
    let key = fixtures::pk1_key().to_compressed_33_byte_hex();
    let code = format!(
        r#"
        collection Account {{
            id: string;
            name: string;

            @call(eth#{key})
            changeName(name: string) {{
                this.name = name;
            }}
        }}
    "#
    );

    let (abi, output) = run(
        &code,
        "Account",
        "changeName",
        serde_json::json!({
            "id": "test",
            "name": "test",
        }),
        vec![serde_json::json!("test2")],
        Some(fixtures::pk1_key()),
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("test".to_owned())),
            ("name".to_owned(), abi::Value::String("test2".to_owned())),
        ]),
    );
}

#[test]
fn read_auth_field_correct_ctx() {
    let code = r#"
        collection Account {
            id: string;
            @read
            pk: PublicKey;
        }
    "#;

    let (_, output) = run(
        code,
        "Account",
        ".readAuth",
        serde_json::json!({
            "id": "",
            "pk": fixtures::pk1(),
        }),
        vec![],
        Some(fixtures::pk1_key()),
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(output.read_auth(), true);
}

#[test]
fn read_auth_field_wrong_ctx() {
    let code = r#"
        collection Account {
            id: string;
            @read
            pk: PublicKey;
        }
    "#;

    let (_, output) = run(
        code,
        "Account",
        ".readAuth",
        serde_json::json!({
            "id": "",
            "pk": fixtures::pk1(),
        }),
        vec![],
        Some(fixtures::pk2_key()),
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(output.read_auth(), false);
}

#[test]
fn read_auth_field_no_ctx() {
    let code = r#"
        collection Account {
            id: string;
            @read
            pk: PublicKey;
        }
    "#;

    let (_, output) = run(
        code,
        "Account",
        ".readAuth",
        serde_json::json!({
            "id": "",
            "pk": fixtures::pk1(),
        }),
        vec![],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(output.read_auth(), false);
}

#[test]
fn read_auth_collection_with_pk() {
    let code = r#"
        @read
        collection Account {
            id: string;
            pk: PublicKey;
        }
    "#;

    let (_, output) = run(
        code,
        "Account",
        ".readAuth",
        serde_json::json!({
            "id": "",
            "pk": fixtures::pk1(),
        }),
        vec![],
        Some(fixtures::pk1_key()),
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(output.read_auth(), true);
}

#[test]
fn read_auth_collection_without_pk() {
    let code = r#"
        @read
        collection Account {
            id: string;
            pk: PublicKey;
        }
    "#;

    let (_, output) = run(
        code,
        "Account",
        ".readAuth",
        serde_json::json!({
            "id": "",
            "pk": fixtures::pk1(),
        }),
        vec![],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(output.read_auth(), true);
}

#[test]
fn index_of() {
    fn run_index_of(
        element_type: &str,
        arr: Vec<serde_json::Value>,
        item: serde_json::Value,
    ) -> Result<abi::Value, error::Error> {
        let code = r#"
            @public
            collection Account {
                id: string;
                result: i32;

                indexOf(arr: $ELEMENT_TYPE[], item: $ELEMENT_TYPE) {
                    this.result = arr.indexOf(item);
                }
            }
        "#
        .replace("$ELEMENT_TYPE", element_type);

        let (abi, output) = run(
            &code,
            "Account",
            "indexOf",
            serde_json::json!({
                "id": "test",
                "result": 123456,
            }),
            vec![serde_json::json!(arr), serde_json::json!(item)],
            None,
            HashMap::new(),
        )?;

        let this = output.this(&abi).unwrap();
        Ok(match this {
            abi::Value::StructValue(fields) => fields
                .into_iter()
                .find_map(|(k, v)| if k == "result" { Some(v) } else { None })
                .unwrap(),
            _ => unreachable!(),
        })
    }

    assert_eq!(
        run_index_of(
            "string",
            vec![serde_json::json!("a"), serde_json::json!("b")],
            serde_json::json!("a")
        )
        .unwrap(),
        abi::Value::Int32(0),
    );

    assert_eq!(
        run_index_of(
            "string",
            vec![serde_json::json!("a"), serde_json::json!("b")],
            serde_json::json!("b")
        )
        .unwrap(),
        abi::Value::Int32(1),
    );

    assert_eq!(
        run_index_of(
            "string",
            vec![serde_json::json!("a"), serde_json::json!("b")],
            serde_json::json!("c")
        )
        .unwrap(),
        abi::Value::Int32(-1),
    );

    assert_eq!(
        run_index_of(
            "i32",
            vec![serde_json::json!(1), serde_json::json!(2)],
            serde_json::json!(2)
        )
        .unwrap(),
        abi::Value::Int32(1),
    );
}

#[test]
fn splice() {
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

    // Doesn't change the array if delete_count is 0
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

    // Errors if start > length(arr)
    assert!(run_splice(serde_json::json!([1, 2, 3, 4, 5]), 6, 0).is_err());
}

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
