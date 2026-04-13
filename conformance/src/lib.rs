#[cfg(test)]
mod tests {
    use phig::Value;

    fn assert_passes(name: &str, phig_src: &str, json_src: &str) {
        let actual: Value = phig_src
            .parse()
            .unwrap_or_else(|e| panic!("{name}.phig should parse successfully, got error: {e}"));

        let expected: Value = serde_json::from_str(json_src)
            .unwrap_or_else(|e| panic!("failed to parse {name}.json: {e}"));

        assert_eq!(actual, expected);
    }

    fn assert_fails(name: &str, phig_src: &str) {
        let result: Result<Value, _> = phig_src.parse();
        assert!(
            result.is_err(),
            "{name}.phig should fail to parse, but got: {:#?}",
            result.unwrap()
        );
    }

    include!(concat!(env!("OUT_DIR"), "/tests.rs"));
}
