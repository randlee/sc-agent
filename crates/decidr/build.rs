use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from("integration-cases.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let records: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(&manifest)?)?;
    let mut ids = Vec::with_capacity(records.len());
    for record in records {
        let id = record
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("integration case record is missing its string id")?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        {
            return Err(format!("invalid integration case id: {id}").into());
        }
        if ids.iter().any(|previous| previous == id) {
            return Err(format!("duplicate integration case id: {id}").into());
        }
        ids.push(id.to_owned());
    }
    let generated = format!(
        "const REQUIRED_CASES: [&str; {}] = [\n{}\n];\n",
        ids.len(),
        ids.iter()
            .map(|id| format!("    {id:?},"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?)
        .join("integration_cases.rs");
    fs::write(out, generated)?;
    Ok(())
}
