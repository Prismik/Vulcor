use ash::{Entry, Instance, ext::debug_utils, vk};
use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_void},
};

pub const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
pub const VALIDATION_LAYERS: [&'static CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];

pub fn validation_layers_supported(entry: &Entry) -> bool {
    let mut found: bool = true;
    for required in VALIDATION_LAYERS.iter() {
        found = unsafe {
            entry.enumerate_instance_layer_properties()
                .unwrap()
                .iter()
                .any(|layer| {
                    let name = CStr::from_ptr(layer.layer_name.as_ptr());
                    required == &name
                })
        };

        if !found {
            break;
        }
    }

    return found
}

pub fn setup_debug_messenger(entry: &Entry, instance: &Instance) -> Option<(debug_utils::Instance, vk::DebugUtilsMessengerEXT)> {
    if !VALIDATION_ENABLED { return None; }

    let create_info = create_debug_info();
    let debug_utils = debug_utils::Instance::new(entry, instance);
    let debug_utils_messenger = unsafe {
        debug_utils
            .create_debug_utils_messenger(&create_info, None)
            .unwrap()
    };

    Some((debug_utils, debug_utils_messenger))
}

pub fn create_debug_info() -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
    vk::DebugUtilsMessengerCreateInfoEXT::default()
        .flags(vk::DebugUtilsMessengerCreateFlagsEXT::empty())
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(debug_callback))
}

extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    types: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _: *mut c_void,
) -> vk::Bool32 {
    let data = unsafe { *data };
    let message = unsafe { CStr::from_ptr(data.p_message) }.to_string_lossy();
    if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::ERROR {
        log::error!("({:?}) {}", types, message);
    } else if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::WARNING {
        log::warn!("({:?}) {}", types, message);
    } else if severity >= vk::DebugUtilsMessageSeverityFlagsEXT::INFO {
        log::debug!("({:?}) {}", types, message);
    } else {
        log::trace!("({:?}) {}", types, message);
    }

    vk::FALSE
}