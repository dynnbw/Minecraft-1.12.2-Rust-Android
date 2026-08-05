use std::path::{Path, PathBuf};

fn main() {
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
