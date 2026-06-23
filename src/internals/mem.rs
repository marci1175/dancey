use windows::core::PCWSTR;

/// Creates a valid [`PCWSTR`] instance. The underlying pointer is pointing to the vec returned in the tuple. 
/// The caller must keep this vector alive for the length of the usage of the PCWSTR.
pub fn string_to_pcwstr(string: &str) -> (PCWSTR, Vec<u16>) {
    let wide: Vec<u16> = string.encode_utf16().chain(Some(0)).collect();
    let ptr = PCWSTR(wide.as_ptr());
    (ptr, wide) // caller must keep the Vec alive
}
