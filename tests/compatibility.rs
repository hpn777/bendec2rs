#[test]
fn matches_bendec_052_fixtures() {
    let cases: serde_json::Value = serde_json::from_str(include_str!("fixtures.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let request = &case["request"];
        let output =
            bendec2rs::generate_string(request["types"].as_array().unwrap(), &request["options"])
                .unwrap_or_else(|e| panic!("{}: {e}", case["name"]));
        assert_eq!(
            output,
            case["expected"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        let native = bendec2rs::generate_string_with_hooks(
            request["types"].as_array().unwrap(),
            &request["options"],
            &serde_json::Value::Null,
            &mut (),
        )
        .unwrap();
        assert_eq!(
            native, output,
            "no-op native callback drift: {}",
            case["name"]
        );
    }
}
