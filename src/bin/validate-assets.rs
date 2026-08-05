use std::path::PathBuf;

use clap::Parser;
use minecraft_1_12_2_rust_vulkan::launcher::AssetRoot::AssetRoot;

#[derive(Debug, Parser)]
#[command(name = "validate-assets")]
struct Arguments {
    #[arg(long)]
    path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    let assets = AssetRoot::open(arguments.path)?;
    println!("validated asset root: {}", assets.root().display());
    println!("asset coverage: {:?}", assets.coverage());
    Ok(())
}
