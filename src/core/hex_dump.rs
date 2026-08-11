//! Platform-agnostic raw memory reading and hex-dump formatting for the
//! Memory Inspector view. OS backends only need to implement
//! `MemoryProvider::read_process_memory`; everything else lives here.

const HEX_ROW_WIDTH: usize = 8 * 3;

pub fn format_hex_dump(address: usize, bytes: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    if bytes.is_empty() {
        lines.push("  <no bytes read>".to_string());
        return lines;
    }
    for (chunk_idx, chunk) in bytes.chunks(16).enumerate() {
        let addr = address + chunk_idx * 16;
        let mut hex_spans = String::with_capacity(48);
        let mut ascii_spans = String::with_capacity(18);
        ascii_spans.push('|');
        for (i, &b) in chunk.iter().enumerate() {
            if i == 8 {
                hex_spans.push(' ');
            }
            hex_spans.push_str(&format!("{:02x} ", b));
            if b.is_ascii_graphic() || b == b' ' {
                ascii_spans.push(b as char);
            } else {
                ascii_spans.push('.');
            }
        }
        let pad_len = HEX_ROW_WIDTH.saturating_sub(hex_spans.len());
        for _ in 0..pad_len {
            hex_spans.push(' ');
        }
        ascii_spans.push('|');

        lines.push(format!("0x{:012x}:  {} {}", addr, hex_spans, ascii_spans));
    }
    lines
}