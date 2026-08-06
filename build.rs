use std::path::{Path, PathBuf};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        copy_prebuilt_spirv();
        return;
    }
    compile_spirv_with_shaderc();
}

/// Android builds embed precompiled SPIR-V (see tools/spv-precompiler) so that
/// Google shaderc never needs to be cross-compiled. Windows keeps the live
/// shaderc path so shader edits are picked up on rebuild.
fn copy_prebuilt_spirv() {
    let shader_dir = Path::new("src/vulkan/shaders");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let spv_dir = shader_dir.join("spv");
    let shaders = [
        "world.vert.spv",
        "world.frag.spv",
        "gui.vert.spv",
        "gui.frag.spv",
        "panorama.vert.spv",
        "panorama.frag.spv",
        "panorama_blur.frag.spv",
    ];
    for file in shaders {
        let source = spv_dir.join(file);
        let target = out_dir.join(file);
        std::fs::copy(&source, &target)
            .unwrap_or_else(|e| panic!("failed copying {} to {}: {e}", source.display(), target.display()));
        println!("cargo:rerun-if-changed={}", source.display());
    }
}

fn compile_spirv_with_shaderc() {
    let shader_dir = Path::new("src/vulkan/shaders");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let compiler = shaderc::Compiler::new().expect("failed to create shaderc compiler");

    for (file, kind) in [
        ("world.vert", shaderc::ShaderKind::Vertex),
        ("world.frag", shaderc::ShaderKind::Fragment),
        ("gui.vert", shaderc::ShaderKind::Vertex),
        ("gui.frag", shaderc::ShaderKind::Fragment),
        ("panorama.vert", shaderc::ShaderKind::Vertex),
        ("panorama.frag", shaderc::ShaderKind::Fragment),
        ("panorama_blur.frag", shaderc::ShaderKind::Fragment),
    ] {
        let source_path = shader_dir.join(file);
        let source = std::fs::read_to_string(&source_path)
            .unwrap_or_else(|error| panic!("failed reading {}: {error}", source_path.display()));
        let artifact = compiler
            .compile_into_spirv(&source, kind, file, "main", None)
            .unwrap_or_else(|error| panic!("failed compiling {file}: {error}"));
        let output_path = out_dir.join(format!("{file}.spv"));
        std::fs::write(&output_path, artifact.as_binary_u8())
            .unwrap_or_else(|error| panic!("failed writing {}: {error}", output_path.display()));
        println!("cargo:rerun-if-changed={}", source_path.display());
    }
}
