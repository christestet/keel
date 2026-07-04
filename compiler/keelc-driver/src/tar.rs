//! Minimal deterministic USTAR writer for OCI image layers and the
//! `oci-archive` layout form (spec ch19). Every entry uses a fixed mtime
//! (Unix epoch 0) and fixed uid/gid (0:0) regardless of build host, so the
//! same file set always produces byte-identical tar bytes (spec §19.5).
//!
//! ponytail: no long-name (GNU `@LongLink`) or PAX support — every path used
//! by the image writer fits in the 100-byte ustar name field. Add PAX headers
//! if a future entry needs a longer path.

const BLOCK: usize = 512;

pub struct TarBuilder {
    out: Vec<u8>,
}

impl TarBuilder {
    pub fn new() -> Self {
        TarBuilder { out: Vec::new() }
    }

    /// Appends one regular-file entry. `path` must be ASCII and under 100
    /// bytes (checked by the caller's fixed, short paths).
    pub fn add_file(&mut self, path: &str, mode: u32, data: &[u8]) {
        let mut header = [0u8; BLOCK];
        write_field(&mut header[0..100], path.as_bytes());
        write_octal(&mut header[100..108], mode as u64);
        write_octal(&mut header[108..116], 0); // uid
        write_octal(&mut header[116..124], 0); // gid
        write_octal(&mut header[124..136], data.len() as u64);
        write_octal(&mut header[136..148], 0); // mtime
        header[156] = b'0'; // typeflag: regular file
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        // Checksum is computed with the checksum field itself treated as
        // eight ASCII spaces, then written as a 6-digit octal + NUL + space.
        header[148..156].copy_from_slice(b"        ");
        let sum: u32 = header.iter().map(|&b| b as u32).sum();
        let sum_str = format!("{sum:06o}\0 ");
        header[148..156].copy_from_slice(sum_str.as_bytes());

        self.out.extend_from_slice(&header);
        self.out.extend_from_slice(data);
        let pad = (BLOCK - (data.len() % BLOCK)) % BLOCK;
        self.out.extend(std::iter::repeat_n(0u8, pad));
    }

    /// Two 512-byte zero blocks mark the archive's end (POSIX ustar).
    pub fn finish(mut self) -> Vec<u8> {
        self.out.extend(std::iter::repeat_n(0u8, BLOCK * 2));
        self.out
    }
}

fn write_field(dest: &mut [u8], bytes: &[u8]) {
    dest[..bytes.len()].copy_from_slice(bytes);
}

fn write_octal(dest: &mut [u8], value: u64) {
    // Field is N bytes: an octal number, NUL-terminated, space-padded on the left.
    let width = dest.len() - 1;
    let octal = format!("{value:0width$o}", width = width);
    let octal = &octal[octal.len() - width..];
    dest[..width].copy_from_slice(octal.as_bytes());
    dest[width] = 0;
}

#[cfg(test)]
mod tests {
    use super::TarBuilder;

    #[test]
    fn single_file_is_reproducible_and_parseable() {
        let mut a = TarBuilder::new();
        a.add_file("app/main", 0o755, b"binary bytes");
        let a = a.finish();

        let mut b = TarBuilder::new();
        b.add_file("app/main", 0o755, b"binary bytes");
        let b = b.finish();
        assert_eq!(a, b, "identical inputs must produce byte-identical tar");

        // Total size is a whole number of 512-byte blocks (header + data + pad + 2 trailer).
        assert_eq!(a.len() % 512, 0);
        assert_eq!(&a[0..8], b"app/main");
        assert_eq!(&a[257..263], b"ustar\0");
    }
}
