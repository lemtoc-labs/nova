use toml::Value;

fn manifest() -> Value {
    toml::from_str(include_str!("../Cargo.toml")).expect("Cargo.toml should be valid TOML")
}

#[test]
fn package_metadata_is_declared() {
    let manifest = manifest();
    let package = manifest
        .get("package")
        .and_then(Value::as_table)
        .expect("Cargo.toml should contain a package table");

    assert_eq!(
        package.get("rust-version").and_then(Value::as_str),
        Some("1.88")
    );
    assert_eq!(
        package.get("readme").and_then(Value::as_str),
        Some("README.md")
    );

    let keywords = package
        .get("keywords")
        .and_then(Value::as_array)
        .expect("package keywords should be an array")
        .iter()
        .map(|keyword| keyword.as_str().expect("keywords should be strings"))
        .collect::<Vec<_>>();
    assert_eq!(keywords, ["zsh", "prompt", "shell", "terminal", "async"]);

    let categories = package
        .get("categories")
        .and_then(Value::as_array)
        .expect("package categories should be an array")
        .iter()
        .map(|category| category.as_str().expect("categories should be strings"))
        .collect::<Vec<_>>();
    assert_eq!(categories, ["command-line-utilities"]);
}

#[test]
fn release_profiles_keep_unwinding_enabled() {
    let manifest = manifest();
    let profiles = manifest
        .get("profile")
        .and_then(Value::as_table)
        .expect("Cargo.toml should contain profile tables");
    let release = profiles
        .get("release")
        .and_then(Value::as_table)
        .expect("Cargo.toml should contain a release profile");

    assert_eq!(
        release.get("panic").and_then(Value::as_str),
        Some("unwind"),
        "worker job isolation relies on catch_unwind"
    );

    let dist = profiles
        .get("dist")
        .and_then(Value::as_table)
        .expect("Cargo.toml should contain a dist profile");
    assert_eq!(
        dist.get("inherits").and_then(Value::as_str),
        Some("release")
    );
    assert_ne!(
        dist.get("panic").and_then(Value::as_str),
        Some("abort"),
        "the dist profile must not disable panic unwinding"
    );
}
