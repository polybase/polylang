use super::*;

#[test]
fn assign_param_record_to_field_record() {
    let code = r#"
        contract User {
            id: string;
            @delegate
            pk: PublicKey;
        }

        contract Account {
            id: string;
            name: string;
            user: User;

            constructor (id: string, name: string, user: User) {
                this.id = id;
                this.name = name;
                this.user = user;
            }
        }
    "#;

    let (abi, output) = run(
        code,
        "Account",
        "constructor",
        serde_json::json!({
            "id": "",
            "name": "",
            "user": {
                "id": "",
                "pk": fixtures::pk1(),
            },
        }),
        vec![
            serde_json::json!("john1"),
            serde_json::json!("John"),
            serde_json::json!({
                "id": "user1",
            }),
        ],
        None,
        HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        output.this(&abi).unwrap(),
        abi::Value::StructValue(vec![
            ("id".to_owned(), abi::Value::String("john1".to_owned())),
            ("name".to_owned(), abi::Value::String("John".to_owned())),
            (
                "user".to_owned(),
                abi::Value::ContractReference("user1".bytes().collect()),
            ),
        ]),
    );
}
