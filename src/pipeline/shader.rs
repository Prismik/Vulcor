use std::{env, error::Error, path::PathBuf};

use ash::{vk, Device};

pub struct Shader {
    pub instance: vk::ShaderModule
}
impl Shader {
    pub fn new<P: AsRef<std::path::Path>>(path: P, logical_device: &Device) -> Result<Self, Box<dyn Error>> {
        let code = Self::read_shader_file(path)?;
        let instance = Self::create_shader_module(logical_device, &code)?;
        Ok(Self{instance})
    }

    fn read_shader_file<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<u32>, Box<dyn Error>> {
        let current_dir = env::current_dir()?;
        let mut target = PathBuf::from(current_dir);
        target.push(path);
        log::debug!("Loading shader => {}", target.to_string_lossy());
        let mut file = std::fs::File::open(target)?;
        Ok(ash::util::read_spv(&mut file)?)
    }

    fn create_shader_module(logical_device: &Device, code: &[u32]) -> Result<vk::ShaderModule, Box<dyn Error>> {
        let create_info = vk::ShaderModuleCreateInfo::default().code(code);
        let module = unsafe { logical_device.create_shader_module(&create_info, None)? };
        Ok(module)
    }
}