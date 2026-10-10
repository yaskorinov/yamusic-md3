//! Расшифровка файлов с transport=encraw (yandex_music/utils/decrypt.py, extra «crypto»):
//! AES-128-CTR, начальный счётчик — 16 нулевых байт, ключ — hex из downloadInfo.key.
use aes::Aes128;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockEncrypt, KeyInit};

use crate::error::{ApiError, Result};

/// Расшифровать (или зашифровать — CTR симметричен) содержимое файла.
pub fn decrypt_track(data: &[u8], key_hex: &str) -> Result<Vec<u8>> {
    let mut out = data.to_vec();
    apply_keystream(&mut out, key_hex, 0)?;
    Ok(out)
}

/// Наложить ключевой поток на кусок, начинающийся со смещения `offset` файла (для потоковой расшифровки).
pub fn apply_keystream(buf: &mut [u8], key_hex: &str, offset: u64) -> Result<()> {
    let key = hex::decode(key_hex.trim()).map_err(|e| ApiError::Decode(format!("ключ расшифровки: {e}")))?;
    let cipher = Aes128::new_from_slice(&key)
        .map_err(|_| ApiError::Decode(format!("ключ расшифровки: нужно 16 байт, а не {}", key.len())))?;
    let mut counter = u128::from(offset / 16);
    let mut skip = (offset % 16) as usize;
    let mut pos = 0;
    while pos < buf.len() {
        let mut block = GenericArray::from(counter.to_be_bytes());
        cipher.encrypt_block(&mut block);
        let take = (16 - skip).min(buf.len() - pos);
        for (b, k) in buf[pos..pos + take].iter_mut().zip(&block[skip..skip + take]) {
            *b ^= k;
        }
        pos += take;
        skip = 0;
        counter = counter.wrapping_add(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "000102030405060708090a0b0c0d0e0f";
    // эталон: cryptography Cipher(AES(key), CTR(bytes(16))).decryptor().update(bytes(range(40)))
    const EXPECTED: &str = "c6a03934838a5d8567468b69adc5d6766357018681d5a2095162a7f879e9331569f7a570bdbe80ab";

    #[test]
    fn matches_python() {
        let data: Vec<u8> = (0u8..40).collect();
        assert_eq!(hex::encode(decrypt_track(&data, KEY).unwrap()), EXPECTED);
    }

    #[test]
    fn streaming_offsets() {
        let data: Vec<u8> = (0u8..40).collect();
        let mut tail = data[21..].to_vec();
        apply_keystream(&mut tail, KEY, 21).unwrap();
        assert_eq!(hex::encode(tail), &EXPECTED[42..]);
    }

    #[test]
    fn bad_key() {
        assert!(decrypt_track(b"x", "zz").is_err());
        assert!(decrypt_track(b"x", "0011").is_err());
    }
}
