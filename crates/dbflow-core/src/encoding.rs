//! Character-encoding detection: BOM, UTF-16 without BOM, strict UTF-8, then Windows-1252.

const SNIFF_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Utf16Le => "utf-16le",
            Encoding::Utf16Be => "utf-16be",
            Encoding::Windows1252 => "windows-1252",
        }
    }
}

fn valid_utf8(bytes: &[u8]) -> bool {
    let mut trim = 0;
    while trim <= 3 && trim < bytes.len() {
        if std::str::from_utf8(&bytes[..bytes.len() - trim]).is_ok() {
            return true;
        }
        trim += 1;
    }
    bytes.is_empty()
}

pub fn detect_encoding(bytes: &[u8]) -> Encoding {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Encoding::Utf8;
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        return Encoding::Utf16Le;
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        return Encoding::Utf16Be;
    }
    let sample = &bytes[..bytes.len().min(SNIFF_BYTES)];
    if sample.len() >= 4 {
        let (mut even, mut odd) = (0usize, 0usize);
        let mut i = 0;
        while i + 1 < sample.len() {
            if sample[i] == 0 {
                even += 1;
            }
            if sample[i + 1] == 0 {
                odd += 1;
            }
            i += 2;
        }
        let pairs = (sample.len() / 2) as f64;
        if odd as f64 / pairs > 0.3 && (even as f64 / pairs) < 0.05 {
            return Encoding::Utf16Le;
        }
        if even as f64 / pairs > 0.3 && (odd as f64 / pairs) < 0.05 {
            return Encoding::Utf16Be;
        }
    }
    if valid_utf8(sample) {
        Encoding::Utf8
    } else {
        Encoding::Windows1252
    }
}

/// Decode a whole file (the BOM is consumed).
pub fn decode_bytes(bytes: &[u8]) -> (String, Encoding) {
    let enc = detect_encoding(bytes);
    let codec = match enc {
        Encoding::Utf8 => encoding_rs::UTF_8,
        Encoding::Utf16Le => encoding_rs::UTF_16LE,
        Encoding::Utf16Be => encoding_rs::UTF_16BE,
        Encoding::Windows1252 => encoding_rs::WINDOWS_1252,
    };
    let (text, _, _) = codec.decode(bytes);
    (text.into_owned(), enc)
}
