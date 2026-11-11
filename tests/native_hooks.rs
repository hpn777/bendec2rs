use bendec2rs::{generate_string, generate_string_with_hooks, NativeHooks};
use serde_json::{json, Value};

#[test]
fn native_callbacks_preserve_context_metadata_and_original_id_comments() {
    struct Hooks {
        seen: bool,
    }
    impl NativeHooks for Hooks {
        fn union_discriminant(
            &mut self,
            context: &Value,
            member: &str,
            value: f64,
        ) -> Result<f64, String> {
            assert_eq!(context["kind"], 5);
            assert_eq!(member, "Item");
            assert_eq!(value, 3.0);
            Ok(0x30008 as f64)
        }
        fn for_each_type(
            &mut self,
            generated: String,
            context: &Value,
            metadata: &Value,
        ) -> Result<String, String> {
            if context["name"] == "Tag" {
                assert_eq!(context["kind"], 4);
                assert_eq!(context["variants"][0][1], 3.0);
                assert_eq!(metadata["opaque"], "retained");
                self.seen = true;
            }
            Ok(generated)
        }
    }
    let types = vec![
        json!({"name":"Tag","underlying":"u16","offset":1,"variants":[["Item","0x2"]]}),
        json!({"name":"Item","fields":[]}),
        json!({"name":"Any","members":["Item"]}),
    ];
    let options = json!({"meta":{"Any":{"union":{"discVariant":"Tag","underlying":"u32"}}}});
    let mut hooks = Hooks { seen: false };
    let output = generate_string_with_hooks(
        &types,
        &options,
        &json!({"Tag":{"opaque":"retained"}}),
        &mut hooks,
    )
    .unwrap();
    assert!(hooks.seen);
    assert!(output.contains("Item(Item) = 0x00030008, // 0x00000003"));
    assert_eq!(
        generate_string(&types, &options).unwrap(),
        generate_string_with_hooks(&types, &options, &Value::Null, &mut ()).unwrap()
    );
}

#[test]
fn native_callback_errors_are_not_ignored() {
    struct Hooks;
    impl NativeHooks for Hooks {
        fn for_each_type(&mut self, _: String, _: &Value, _: &Value) -> Result<String, String> {
            Err("native failure".into())
        }
    }
    assert_eq!(
        generate_string_with_hooks(
            &[json!({"name":"u8","size":1})],
            &json!({}),
            &Value::Null,
            &mut Hooks
        )
        .unwrap_err(),
        "native failure"
    );
}

#[test]
fn native_discriminant_errors_are_not_ignored() {
    struct Hooks;
    impl NativeHooks for Hooks {
        fn union_discriminant(&mut self, _: &Value, _: &str, _: f64) -> Result<f64, String> {
            Err("discriminant refused".into())
        }
    }
    let types = vec![
        json!({"name":"Tag","underlying":"u16","variants":[["Item",1]]}),
        json!({"name":"Item","fields":[]}),
        json!({"name":"Any","members":["Item"]}),
    ];
    let options = json!({"meta":{"Any":{"union":{"discVariant":"Tag","underlying":"u32"}}}});
    assert_eq!(
        generate_string_with_hooks(&types, &options, &Value::Null, &mut Hooks).unwrap_err(),
        "discriminant refused"
    );
}
