//! Adds the platform link arguments a Python extension module needs (macOS: dynamic lookup).

fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
