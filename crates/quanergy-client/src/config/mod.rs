mod device_info;
mod settings;
mod xml;

#[cfg(test)]
mod tests;

pub use device_info::{DeviceInfo, SensorModel};
pub use settings::{EncoderMode, PipelineConfig};
pub use xml::flatten_xml;
