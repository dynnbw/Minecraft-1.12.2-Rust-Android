//! Host tool: precompile the main crate's Vulkan GLSL shaders to SPIR-V so
//! Android builds can embed them without cross-compiling Google shaderc.
//! Run (from repo root): cargo run --manifest-path tools/spv-precompiler/Cargo.toml --release
//!
//! Writes src/vulkan/shaders/spv/*.spv, which build.rs copies into OUT_DIR
//! for Android targets.

use std::fs;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    // Run from the repository root; resolve relative to the manifest path so
    // the tool works regardless of the caller's working directory.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.join("..").join("..");
    let shader_dir = repo_root.join("src/vulkan/shaders");
    let out_dir = shader_dir.join("spv");
    fs::create_dir_all(&out_dir)?;
    let compiler = shaderc::Compiler::new().expect("failed to create shaderc compiler");

    let shaders = [
        ("world.vert", shaderc::ShaderKind::Vertex),
        ("world.frag", shaderc::ShaderKind::Fragment),
        ("gui.vert", shaderc::ShaderKind::Vertex),
        ("gui.frag", shaderc::ShaderKind::Fragment),
        ("panorama.vert", shaderc::ShaderKind::Vertex),
        ("panorama.frag", shaderc::ShaderKind::Fragment),
        ("panorama_blur.frag", shaderc::ShaderKind::Fragment),
    ];

    for (file, kind) in shaders {
        let source = fs::read_to_string(shader_dir.join(file))
            .unwrap_or_else(|e| panic!("failed reading {file}: {e}"));
        let artifact = compiler
            .compile_into_spirv(&source, kind, file, "main", None)
            .unwrap_or_else(|e| panic!("failed compiling {file}: {e}"));
        let output = out_dir.join(format!("{file}.spv"));
        fs::write(&output, artifact.as_binary_u8())?;
        println!("wrote {}", output.display());
    }
    Ok(())
}
