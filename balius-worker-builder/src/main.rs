use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Result, bail};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use wit_component::ComponentEncoder;

fn compile(contract_path: &Path) -> Result<Vec<u8>> {
    let manifest_path = contract_path.join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()?;
    let Some(package) = metadata.root_package() else {
        bail!("couldn't find root package");
    };
    let name = package.name.to_string();

    println!("Compiling {name}...");
    Command::new("cargo")
        .arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .exec()?;

    let filename = format!("{}.wasm", name.replace("-", "_"));
    let path = metadata
        .target_directory
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(filename);

    println!("Bundling {name} as WASM component...");
    let module = wat::Parser::new().parse_file(path)?;
    ComponentEncoder::default()
        .validate(true)
        .module(&module)?
        .encode()
}

trait CommandExt {
    fn exec(&mut self) -> Result<()>;
}

impl CommandExt for Command {
    fn exec(&mut self) -> Result<()> {
        let output = self.output()?;
        if !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8(output.stderr)?);
        }
        if !output.status.success() {
            bail!("command failed: {}", output.status);
        }
        Ok(())
    }
}

#[derive(Parser)]
struct Args {
    #[clap(short, long)]
    source_dir: PathBuf,
    #[clap(short, long)]
    target_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::try_parse()?;
    let bytes = compile(&args.source_dir)?;
    if let Some(parent) = args.target_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.target_file, bytes)?;
    println!("compiled to {}", args.target_file.display());
    Ok(())
}
