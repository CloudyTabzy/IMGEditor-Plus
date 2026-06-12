use std::path::Path;

pub fn clamp_entry_name(name: &str, max_chars: usize) -> String {
    name.chars().take(max_chars).collect()
}

pub fn extension_type(file_name: &str) -> Option<&'static str> {
    Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| match extension.to_ascii_lowercase().as_str() {
            "dff" => "Model",
            "txd" => "Texture",
            "col" => "Collision",
            "ifp" => "Animation",
            "ipl" => "Placement",
            "ide" => "Definition",
            "dat" => "Data",
            _ => "file",
        })
}
