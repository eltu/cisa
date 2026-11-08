//! Index key layout: `normalized ‖ 0x00 ‖ surface`.
//!
//! Prefix search runs over the normalized form, and every completion already
//! carries the text shown to the user, so no side table of strings is needed.
//! When both forms are equal only the normalized form is stored.

use std::str::Utf8Error;

const SURFACE_SEPARATOR: u8 = 0x00;

/// Builds the index key for a query; `normalized` must not contain NUL.
pub(crate) fn encode_key(normalized: &str, surface: &str) -> Vec<u8> {
    if normalized == surface {
        return normalized.as_bytes().to_vec();
    }
    let mut key: Vec<u8> = Vec::with_capacity(normalized.len() + 1 + surface.len());
    key.extend_from_slice(normalized.as_bytes());
    key.push(SURFACE_SEPARATOR);
    key.extend_from_slice(surface.as_bytes());
    key
}

/// The text to show for an index key.
pub(crate) fn decode_surface(key: &[u8]) -> Result<&str, Utf8Error> {
    let surface: &[u8] = match key.iter().position(|&b| b == SURFACE_SEPARATOR) {
        Some(separator) => &key[separator + 1..],
        None => key,
    };
    std::str::from_utf8(surface)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("rust", "rust")]
    #[case("previsão", "Previsão")]
    #[case("", "")]
    fn decode_surface_encoded_key_returns_surface(#[case] normalized: &str, #[case] surface: &str) {
        // Act
        let key = encode_key(normalized, surface);

        // Assert
        assert_eq!(decode_surface(&key), Ok(surface));
    }

    #[test]
    fn encode_key_equal_forms_stores_normalized_once() {
        assert_eq!(encode_key("rust", "rust"), b"rust");
    }

    #[test]
    fn decode_surface_invalid_utf8_returns_error() {
        assert!(decode_surface(&[b'a', SURFACE_SEPARATOR, 0xFF]).is_err());
    }
}
