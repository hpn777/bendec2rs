use serde_json::json;

#[test]
fn invalid_definitions_return_errors_not_panics() {
    for invalid in [
        json!(null),
        json!({"name": "Incomplete"}),
        json!({"name": "Broken", "fields": "not an array"}),
        json!({"name": "Broken", "fields": [{"name": "missingType"}]}),
        json!({"name": "Broken", "variants": [["X", {}]], "underlying": "u8"}),
    ] {
        assert!(
            bendec2rs::generate_string(std::slice::from_ref(&invalid), &json!({})).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn javascript_callbacks_are_not_silently_ignored() {
    assert!(bendec2rs::generate_string(&[], &json!({"forEachType": "function() {}"})).is_err());
    assert!(
        bendec2rs::generate_string(&[], &json!({"forEachType": "(generated) => generated"}))
            .is_err()
    );
}

#[test]
fn declarative_templates_can_contain_rust_match_arms() {
    let output = bendec2rs::generate_string(
        &[json!({"name": "Id", "alias": "u64"})],
        &json!({"forEachType": "{{ generated }}\nfn check() { match 1 { _ => () } }"}),
    )
    .unwrap();
    assert!(output.contains("pub type Id = u64;"));
    assert!(output.contains("_ => ()"));
}

#[test]
fn concurrent_calls_have_no_shared_emitter_state() {
    let expected =
        bendec2rs::generate_string(&[json!({"name": "Id", "alias": "u64"})], &json!({})).unwrap();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..100 {
                    let output = bendec2rs::generate_string(
                        &[json!({"name": "Id", "alias": "u64"})],
                        &json!({}),
                    )
                    .unwrap();
                    assert!(output.contains("pub type Id = u64;"));
                }
                bendec2rs::generate_string(&[json!({"name": "Id", "alias": "u64"})], &json!({}))
                    .unwrap()
            })
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), expected);
    }
}
