use std::ffi::OsStr;
use std::fmt::Write;
use std::path::Path;
use std::{env, fs};

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_dir = manifest_dir.join("testdata");
    println!("cargo:rerun-if-changed={}", test_dir.display());

    let mut names: Vec<String> = fs::read_dir(&test_dir)
        .expect(&format!(
            "failed to read {test_dir}",
            test_dir = test_dir.display()
        ))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension() == Some(OsStr::new("phig")) {
                Some(path.file_stem()?.to_str()?.to_string())
            } else {
                None
            }
        })
        .collect();
    names.sort();

    let mut out = String::new();
    for name in &names {
        let phig_path = test_dir.join(format!("{name}.phig"));
        write!(out, "#[allow(non_snake_case)] #[test] fn {name}() {{").unwrap();
        if name.ends_with("_FAIL") {
            write!(
                out,
                r#"assert_fails("{name}", include_str!({phig_path:?}));"#
            )
            .unwrap();
        } else {
            let json_path = test_dir.join(format!("{name}.json"));
            write!(
                out,
                r#"assert_passes("{name}", include_str!({phig_path:?}), include_str!({json_path:?}));"#
            )
            .unwrap();
        }
        write!(out, "}}").unwrap();
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join("tests.rs"), out).unwrap();
}
