use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask manifest dir has no parent")?;
    let dist_dir = repo_root.join("dist");
    let pkg_dir = dist_dir.join("pkg");

    if dist_dir.exists() {
        fs::remove_dir_all(&dist_dir)?;
    }

    fs::create_dir_all(&pkg_dir)?;

    run(Command::new("wasm-pack")
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg("web")
        .arg("--out-dir")
        .arg(&pkg_dir)
        .arg(repo_root))?;

    for generated_file in [
        pkg_dir.join(".gitignore"),
        pkg_dir.join("package.json"),
        pkg_dir.join("spektrogramilo.d.ts"),
        pkg_dir.join("spektrogramilo_bg.wasm.d.ts"),
    ] {
        if generated_file.exists() {
            fs::remove_file(generated_file)?;
        }
    }

    fs::copy(repo_root.join("index.html"), dist_dir.join("index.html"))?;

    Ok(())
}

fn run(command: &mut Command) -> Result<(), Box<dyn std::error::Error>> {
    let status = command.status()?;
    if status.success() {
        return Ok(());
    }

    Err(Box::new(io::Error::other(format!(
        "command exited with status {status}"
    ))))
}
