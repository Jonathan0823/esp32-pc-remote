use std::ptr;
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    },
    core::PCWSTR,
};

const ENTROPY: &[u8] = b"esp32-media-daemon/user-secrets/v1";

pub fn protect(plaintext: &[u8]) -> Result<Vec<u8>, SecretError> {
    transform(plaintext, true)
}

pub fn unprotect(ciphertext: &[u8]) -> Result<Vec<u8>, SecretError> {
    transform(ciphertext, false)
}

fn transform(input: &[u8], encrypt: bool) -> Result<Vec<u8>, SecretError> {
    let input_len = u32::try_from(input.len()).map_err(|_| SecretError)?;
    let entropy_len = u32::try_from(ENTROPY.len()).map_err(|_| SecretError)?;
    let input_blob = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: input.as_ptr().cast_mut(),
    };
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: entropy_len,
        pbData: ENTROPY.as_ptr().cast_mut(),
    };
    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // Safety: input/entropy blobs remain valid for the calls; DPAPI owns the returned allocation.
    unsafe {
        if encrypt {
            CryptProtectData(
                &input_blob,
                PCWSTR::null(),
                Some(&entropy_blob),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        } else {
            CryptUnprotectData(
                &input_blob,
                None,
                Some(&entropy_blob),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        }
    }
    .map_err(|_| SecretError)?;

    if output_blob.pbData.is_null() {
        return Err(SecretError);
    }
    if output_blob.cbData == 0 {
        unsafe { LocalFree(Some(HLOCAL(output_blob.pbData.cast()))) };
        return Err(SecretError);
    }
    // Safety: DPAPI returned cbData bytes at pbData; LocalFree releases that allocation.
    let result = unsafe {
        let length = output_blob.cbData as usize;
        let data = std::slice::from_raw_parts(output_blob.pbData, length);
        let copy = data.to_vec();
        ptr::write_bytes(output_blob.pbData, 0, length);
        LocalFree(Some(HLOCAL(output_blob.pbData.cast())));
        copy
    };
    Ok(result)
}

#[derive(Debug, thiserror::Error)]
#[error("Windows user-bound secret protection failed")]
pub struct SecretError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_round_trips_under_current_user() {
        let secret = b"disposable test secret";
        let protected = protect(secret).unwrap();
        assert_ne!(protected, secret);
        assert_eq!(unprotect(&protected).unwrap(), secret);
    }
}
