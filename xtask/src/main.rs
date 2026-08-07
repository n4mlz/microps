use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

const PACKAGE: &str = "ffi_c";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };
    if command != "build" {
        return Err(usage());
    }

    let Some(package) = args.next() else {
        return Err(usage());
    };
    if package != PACKAGE {
        return Err(format!("unsupported package: {package}"));
    }

    let mut target = None;
    let mut release = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--target" => {
                target = Some(
                    args.next()
                        .ok_or_else(|| "--target requires a value".to_owned())?,
                );
            }
            "--release" => release = true,
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    let target = target.ok_or_else(usage)?;
    build_ffi_c(&target, release).map_err(|error| error.to_string())
}

fn build_ffi_c(target: &str, release: bool) -> io::Result<()> {
    let workspace = workspace_root();
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(&workspace)
        .args(["build", "-p", PACKAGE, "--target", target]);
    if release {
        command.arg("--release");
    }
    let status = command.status()?;
    if !status.success() {
        return Err(io::Error::other("cargo build failed"));
    }

    let profile = if release { "release" } else { "debug" };
    let target_dir = cargo_target_dir(&workspace);
    let artifact = target_dir.join(target).join(profile).join("libffi_c.a");
    if !artifact.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("static library not found: {}", artifact.display()),
        ));
    }

    let output = workspace
        .join("dist")
        .join(PACKAGE)
        .join(target)
        .join(profile);
    fs::create_dir_all(&output)?;
    fs::copy(&artifact, output.join("libffi_c.a"))?;
    generate_header(&workspace, &output.join("microps.h"))?;
    println!("generated {}", output.display());
    Ok(())
}

fn generate_header(workspace: &Path, output: &Path) -> io::Result<()> {
    let crate_dir = workspace.join("crates/platform/ffi_c");
    let config =
        cbindgen::Config::from_file(crate_dir.join("cbindgen.toml")).map_err(io::Error::other)?;
    let bindings = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut file = File::create(output)?;
    bindings.write(&mut file);
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is located in the workspace root")
        .to_path_buf()
}

fn cargo_target_dir(workspace: &Path) -> PathBuf {
    match env::var_os("CARGO_TARGET_DIR") {
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                workspace.join(path)
            }
        }
        None => workspace.join("target"),
    }
}

fn usage() -> String {
    "usage: cargo xtask build ffi_c --target <target> [--release]".to_owned()
}
