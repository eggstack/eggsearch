use crate::core::security_applicability::DependencyParseReport;

pub(crate) fn structured_input_error(
    filename: &str,
    content: &str,
) -> Option<DependencyParseReport> {
    let toml_file = matches!(
        filename,
        "Cargo.lock" | "Cargo.toml" | "poetry.lock" | "uv.lock"
    );
    let json_file = matches!(
        filename,
        "composer.lock"
            | "Pipfile.lock"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "packages.lock.json"
    );
    if toml_file && content.parse::<toml::Value>().is_err() {
        return Some(DependencyParseReport::malformed(format!(
            "{filename} contains invalid TOML"
        )));
    }
    if json_file && serde_json::from_str::<serde_json::Value>(content).is_err() {
        return Some(DependencyParseReport::malformed(format!(
            "{filename} contains invalid JSON"
        )));
    }
    let shape_ok = match filename {
        "Cargo.lock" => content
            .parse::<toml::Value>()
            .ok()
            .and_then(|v| v.get("package").and_then(toml::Value::as_array).map(|_| ()))
            .is_some(),
        "Cargo.toml" => content
            .parse::<toml::Value>()
            .ok()
            .and_then(|v| {
                v.as_table().and_then(|table| {
                    [
                        "package",
                        "workspace",
                        "dependencies",
                        "dev-dependencies",
                        "build-dependencies",
                        "target",
                        "patch",
                        "replace",
                        "profile",
                        "lints",
                    ]
                    .iter()
                    .any(|key| table.contains_key(*key))
                    .then_some(())
                })
            })
            .is_some(),
        "poetry.lock" | "uv.lock" => content
            .parse::<toml::Value>()
            .ok()
            .and_then(|v| v.get("package").and_then(toml::Value::as_array).map(|_| ()))
            .is_some(),
        "composer.lock" => serde_json::from_str::<serde_json::Value>(content)
            .ok()
            .and_then(|v| {
                v.get("packages")
                    .and_then(serde_json::Value::as_array)
                    .map(|_| ())
            })
            .is_some(),
        "Pipfile.lock" => serde_json::from_str::<serde_json::Value>(content)
            .ok()
            .and_then(|v| {
                v.as_object().and_then(|object| {
                    (object.contains_key("default") || object.contains_key("develop")).then_some(())
                })
            })
            .is_some(),
        "package-lock.json" | "npm-shrinkwrap.json" | "packages.lock.json" => {
            serde_json::from_str::<serde_json::Value>(content)
                .ok()
                .and_then(|v| v.as_object().map(|_| ()))
                .is_some()
        }
        _ => true,
    };
    if !shape_ok {
        Some(DependencyParseReport::unsupported(format!(
            "{filename} has a valid syntax but an unsupported document shape"
        )))
    } else {
        None
    }
}
