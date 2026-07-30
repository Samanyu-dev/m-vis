use crate::ui::commands::ScanResult;

pub fn load_heap_snapshot(path: &str) -> std::io::Result<ScanResult> {
    let data = std::fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
