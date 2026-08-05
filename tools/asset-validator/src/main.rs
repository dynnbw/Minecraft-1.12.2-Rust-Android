use std::env;
use std::path::{Path, PathBuf};

const REQUIRED_VISUAL: [&str; 4] = [
    "minecraft/lang/en_us.lang",
    "minecraft/textures/gui/title/minecraft.png",
    "minecraft/textures/gui/widgets.png",
    "minecraft/textures/font/ascii.png",
];

fn parse_path() -> Result<PathBuf, String> {
    let mut arguments = env::args_os().skip(1);
    let mut path = None;
    while let Some(argument) = arguments.next() {
        if argument == std::ffi::OsStr::new("--path") {
            path = arguments.next().map(PathBuf::from);
        } else {
            return Err(format!("unknown argument: {}", argument.to_string_lossy()));
        }
    }
    path.ok_or_else(|| "usage: mc112-asset-validator --path <asset-root>".to_owned())
}

fn has_ogg(root: &Path) -> bool {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ogg"))
            {
                return true;
            }
        }
    }
    false
}

fn run() -> Result<(), String> {
    let root = parse_path()?;
    if !root.is_dir() {
        return Err(format!("asset root does not exist: {}", root.display()));
    }

    for relative in REQUIRED_VISUAL {
        let path = root.join(relative);
        if !path.is_file() {
            return Err(format!("missing required 1.12.2 asset: {}", path.display()));
        }
    }

    let namespace = root.join("minecraft");
    let sound_registry = namespace.join("sounds.json").is_file();
    let sound_objects = has_ogg(&namespace.join("sounds"));
    let optifine_assets = namespace.join("optifine").is_dir()
        || namespace.join("mcpatcher").is_dir();

    if !sound_registry || !sound_objects || !optifine_assets {
        return Err(format!(
            "asset coverage incomplete: visual_assets=true, sound_registry={sound_registry}, sound_objects={sound_objects}, optifine_assets={optifine_assets}"
        ));
    }

    println!("validated asset root: {}", root.display());
    println!(
        "asset coverage: visual_assets=true, sound_registry=true, sound_objects=true, optifine_assets=true"
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}
