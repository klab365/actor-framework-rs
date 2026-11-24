#![cfg(feature = "tokio")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn invalid_actor_routes_do_not_compile() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ui_dir = manifest_dir.join("tests/ui");
    let temp_dir = std::env::temp_dir().join(format!("actor-framework-compile-fail-{}", std::process::id()));
    let target_dir = manifest_dir.join("../../target/compile-fail");

    fs::create_dir_all(&temp_dir).expect("create compile-fail fixture directory");

    for entry in fs::read_dir(&ui_dir).expect("read UI fixtures") {
        let source = entry.expect("read UI fixture entry").path();
        if source.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }

        let case_dir = temp_dir.join(source.file_stem().expect("fixture has a name"));
        fs::create_dir_all(case_dir.join("src")).expect("create fixture source directory");
        fs::copy(&source, case_dir.join("src/main.rs")).expect("copy fixture source");
        fs::write(
            case_dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"compile-fail-case\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nactor-framework-rs = {{ path = {:?}, features = [\"tokio\"] }}\n",
                manifest_dir
            ),
        )
        .expect("write fixture manifest");

        let output = Command::new("cargo")
            .args(["check", "--quiet", "--manifest-path"])
            .arg(case_dir.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target_dir)
            .output()
            .expect("run cargo check for fixture");

        assert!(
            !output.status.success(),
            "{} unexpectedly compiled successfully",
            source.display()
        );
    }

    fs::remove_dir_all(temp_dir).expect("remove compile-fail fixtures");
}
