use std::{env, path::PathBuf, process::Command};

pub fn main() {
    let mut css_out = PathBuf::from(env::var("OUT_DIR").unwrap());
    css_out.push("out.css");
    let _ = Command::new("npx")
        .arg("tailwindcss")
        .arg("-i")
        .arg("./main.css")
        .arg("-o")
        .arg(css_out)
        .arg("--minify")
        .spawn();
}
