mod qraw;
mod sidecar;

#[cfg(test)]
mod tests;

pub use qraw::{QrawReader, QrawWriter};
pub use sidecar::SidecarMetadata;

pub fn current_time_string() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
