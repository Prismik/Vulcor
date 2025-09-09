use std::{env, error::Error, ffi::OsStr, fs, path::{Path, PathBuf}, process::{Command, Output}};

fn main() {
    compile_shaders();
}

fn compile_shaders() {
    let shaders_path = shader_source_path();
    let validator_path = glsl_script_path();

    fs::read_dir(&shaders_path)
        .unwrap()
        .map(Result::unwrap)
        .filter(|dir| dir.file_type().unwrap().is_file())
        .filter(|dir| dir.path().extension() != Some(OsStr::new("spv")))
        .for_each(|dir| {
            let path = dir.path();
            let name = path.file_name().unwrap().to_str().unwrap();
            let output = format!("{}.spv", &name);
            println!("Compiling {:?}", path.as_os_str());
            let result = dbg!(Command::new(validator_path.as_os_str())
                .current_dir(&shaders_path)
                .arg("-V")
                .arg(&path)
                .arg("-o")
                .arg(output))
            .output();

            handle_validation_result(result);
        });
}

fn shader_source_path() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
    println!("Shader source location => {:?}", root.as_os_str());
    root
}

fn glsl_script_path() -> PathBuf {
    let vulkan_sdk_dir = env!("VULKAN_SDK");
    let platform_location = match env::consts::OS {
        "macos" => "macOS/bin",
        "windows" => "Bin",
        "linux" => "bin",
        _ => panic!("Running on an unknown OS => {}", env::consts::OS),
    };
    let script = match env::consts::OS {
        "macos" => "glslangValidator",
        "windows" => "glslangValidator.exe",
        "linux" => "glslangValidator",
        _ => panic!("Running on an unknown OS => {}", env::consts::OS),
    };
    let path = Path::new(vulkan_sdk_dir)
        .join(platform_location)
        .join(script);
    println!("GlslangValidator path => {:?}", path.as_os_str());
    path
}

fn handle_validation_result(result: Result<Output, std::io::Error>) {
    match result {
        Ok(output) => {
            if output.status.success() {
                println!("Shader compilation succeeded.");
                print!("stdout => {}", String::from_utf8(output.stdout).unwrap_or("stdout failed".to_string()));
            } else {
                eprintln!("Shader compilation failed => {}", output.status);
                eprint!("stdout => {}", String::from_utf8(output.stdout).unwrap_or("stdout failed".to_string()));
                eprint!("stderr => {}", String::from_utf8(output.stderr).unwrap_or("stderr failed".to_string()));
                panic!("Failed to compile shaders. Status => {}", output.status)
            }
        }
        Err(error) => panic!("Failed to compile shaders => {}", error)
    }
}