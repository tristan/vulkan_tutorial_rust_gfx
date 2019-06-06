use std::fs::File;
use std::io::prelude::Write;
use std::path::Path;
use std::env;
use shaderc;

fn main() {

    let src_path = Path::new("shaders");
    let out_dir_str = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);
    let out_file_path = out_dir.join(&["compiled_shaders.rs"].join(""));
    let mut outfile = File::create(out_file_path).unwrap();

    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();

    for entry in std::fs::read_dir(&src_path).unwrap() {
        let path = entry.as_ref().unwrap().path();
        let filename = path.file_name().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap().to_ascii_uppercase();
        let ext = path.extension().unwrap();

        let src = std::fs::read_to_string(&path).unwrap();
        let (kind, kind_str) = match ext.to_str() {
            Some("vert") => (shaderc::ShaderKind::Vertex, "VERTEX"),
            Some("frag") => (shaderc::ShaderKind::Fragment, "FRAGMENT"),
            _ => continue
        };

        let binary_result = compiler.compile_into_spirv(
            &src, kind, filename.to_str().unwrap(),
            "main", Some(&options)).unwrap();

        let binary: &[u8] = binary_result.as_binary_u8();
        write!(outfile,
               "const {}_{}_SHADER: [u8;{}] = {:?};\n",
               stem, kind_str, binary_result.len(), binary)
            .unwrap();
    }
}
