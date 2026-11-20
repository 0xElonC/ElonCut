// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Concat contributors

//! Embed every guide at compile time; reading one never opens a file.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=guides");
    let directory = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("guides");
    let mut files: Vec<_> = fs::read_dir(directory)
        .expect("guides directory")
        .map(|entry| entry.expect("guide entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    files.sort();
    let mut generated = String::from("const GUIDES: &[Guide] = &[\n");
    for path in files {
        let id = path
            .file_stem()
            .and_then(|id| id.to_str())
            .expect("topic id");
        assert!(
            !id.is_empty()
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
            "guide topic ids use lowercase letters, digits and hyphens: {id}"
        );
        let body = fs::read_to_string(&path).expect("UTF-8 guide");
        // A topic starts with a Markdown H1 followed by a one-line summary.
        // License and source headers before that heading are kept verbatim.
        let mut lines = body.lines().skip_while(|line| !line.starts_with("# "));
        lines.next().expect("guide H1 heading");
        let summary = lines
            .find(|line| !line.trim().is_empty())
            .expect("guide summary");
        generated.push_str(&format!(
            "Guide {{ id: {id:?}, summary: {summary:?}, body: include_str!({path:?}) }},\n"
        ));
    }
    generated.push_str("];");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"));
    fs::write(output.join("guides.rs"), generated).expect("embedded guide registry");
}
