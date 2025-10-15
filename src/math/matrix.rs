use cgmath::{point3, Deg};

pub type Mat4 = cgmath::Matrix4<f32>;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MVP {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}
