// Copyright 2020 The Tink-Rust Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
////////////////////////////////////////////////////////////////////////////////

//! AES-GCM-HKDF based implementation of the [`tink::StreamingAead`] trait.

use super::noncebased;
use aes_gcm::aead::{generic_array::GenericArray, Aead, NewAead};
use std::convert::TryInto;
use tink::{proto::HashType, subtle::random::get_random_bytes, utils::wrap_err, TinkError};

/// The size of the nonces used for GCM.
pub const AES_GCM_HKDF_NONCE_SIZE_IN_BYTES: usize = 12;

/// The size of the randomly generated nonce prefix.
pub const AES_GCM_HKDF_NONCE_PREFIX_SIZE_IN_BYTES: usize = 7;

/// The size of the tags of each ciphertext segment.
pub const AES_GCM_HKDF_TAG_SIZE_IN_BYTES: usize = 16;

/// `AesGcmHkdf` implements streaming AEAD encryption using AES-GCM.
///
/// Each ciphertext uses a new AES-GCM key. These keys are derived using HKDF
/// and are derived from the key derivation key, a randomly chosen salt of the
/// same size as the key and a nonce prefix.
#[derive(Clone)]
pub struct AesGcmHkdf {
    pub main_key: Vec<u8>,
    hkdf_alg: HashType,
    key_size_in_bytes: usize,
    ciphertext_segment_size: usize,
    first_ciphertext_segment_offset: usize,
    plaintext_segment_size: usize,
}

#[derive(Clone)]
enum AesGcmKeyVariant {
    Aes128(Box<aes_gcm::Aes128Gcm>),
    Aes256(Box<aes_gcm::Aes256Gcm>),
}

impl AesGcmHkdf {
    /// Initialize a streaming primitive with a key derivation key
    /// and encryption parameters.
    ///
    /// `main_key` is an input keying material used to derive sub keys.
    /// `hkdf_alg` is a MAC algorithm hash type, used for the HKDF key derivation.
    /// `key_size_in_bytes argument` is a key size of the sub keys.
    /// `ciphertext_segment_size` argument is the size of ciphertext segments.
    /// `first_segment_offset` argument is the offset of the first ciphertext segment.
    pub fn new(
        main_key: &[u8],
        hkdf_alg: HashType,
        key_size_in_bytes: usize,
        ciphertext_segment_size: usize,
        first_segment_offset: usize,
    ) -> Result<AesGcmHkdf, TinkError> {
        if main_key.len() < 16 || main_key.len() < key_size_in_bytes {
            return Err("main_key too short".into());
        }
        super::validate_aes_key_size(key_size_in_bytes)?;
        let header_len = 1 + key_size_in_bytes + AES_GCM_HKDF_NONCE_PREFIX_SIZE_IN_BYTES;
        if ciphertext_segment_size
            <= first_segment_offset + header_len + AES_GCM_HKDF_TAG_SIZE_IN_BYTES
        {
            return Err("ciphertext_segment_size too small".into());
        }

        Ok(AesGcmHkdf {
            main_key: main_key.to_vec(),
            hkdf_alg,
            key_size_in_bytes,
            ciphertext_segment_size,
            first_ciphertext_segment_offset: first_segment_offset + header_len,
            plaintext_segment_size: ciphertext_segment_size - AES_GCM_HKDF_TAG_SIZE_IN_BYTES,
        })
    }

    /// Return the length of the encryption header.
    pub fn header_length(&self) -> usize {
        1 + self.key_size_in_bytes + AES_GCM_HKDF_NONCE_PREFIX_SIZE_IN_BYTES
    }

    /// Return a key derived from the given main key using `salt` and `aad` parameters.
    fn derive_key(&self, salt: &[u8], aad: &[u8]) -> Result<Vec<u8>, TinkError> {
        tink::subtle::compute_hkdf(
            self.hkdf_alg,
            &self.main_key,
            salt,
            aad,
            self.key_size_in_bytes,
        )
    }
}

impl tink::StreamingAead for AesGcmHkdf {
    /// Return a wrapper around an underlying [`std::io::Write`], such that
    /// any write-operation via the wrapper results in AEAD-encryption of the
    /// written data, using aad as associated authenticated data. The associated
    /// data is not included in the ciphertext and has to be passed in as parameter
    /// for decryption.
    fn new_encrypting_writer(
        &self,
        mut w: Box<dyn std::io::Write>,
        aad: &[u8],
    ) -> Result<Box<dyn tink::EncryptingWrite>, TinkError> {
        let salt = get_random_bytes(self.key_size_in_bytes);
        let nonce_prefix = get_random_bytes(AES_GCM_HKDF_NONCE_PREFIX_SIZE_IN_BYTES);

        let dkey = self.derive_key(&salt, aad)?;
        let cipher_key = new_cipher_key(&dkey)?;

        let mut header = Vec::with_capacity(self.header_length());
        header.push(
            self.header_length()
                .try_into()
                .map_err(|e| wrap_err("header length too long", e))?,
        );
        header.extend_from_slice(&salt);
        header.extend_from_slice(&nonce_prefix);
        w.write(&header).map_err(|e| wrap_err("write failed", e))?;

        let nw = noncebased::Writer::new(noncebased::WriterParams {
            w,
            segment_encrypter: Box::new(AesGcmHkdfSegmentEncrypter { cipher_key }),
            nonce_size: AES_GCM_HKDF_NONCE_SIZE_IN_BYTES,
            nonce_prefix,
            plaintext_segment_size: self.plaintext_segment_size,
            first_ciphertext_segment_offset: self.first_ciphertext_segment_offset,
        })?;

        Ok(Box::new(nw))
    }

    /// Return a wrapper around an underlying [`std::io::Read`], such that
    /// any read-operation via the wrapper results in AEAD-decryption of the
    /// underlying ciphertext, using aad as associated authenticated data.
    fn new_decrypting_reader(
        &self,
        mut r: Box<dyn std::io::Read>,
        aad: &[u8],
    ) -> Result<Box<dyn std::io::Read>, TinkError> {
        let mut hlen = vec![0; 1];
        r.read_exact(&mut hlen)
            .map_err(|e| wrap_err("failed to reader header len", e))?;
        if hlen[0] as usize != self.header_length() {
            return Err("invalid header length".into());
        }

        let mut salt = vec![0; self.key_size_in_bytes];
        r.read_exact(&mut salt)
            .map_err(|e| wrap_err("cannot read salt", e))?;

        let mut nonce_prefix = vec![0; AES_GCM_HKDF_NONCE_PREFIX_SIZE_IN_BYTES];
        r.read_exact(&mut nonce_prefix)
            .map_err(|e| wrap_err("cannot read nonce_prefix", e))?;

        let dkey = self.derive_key(&salt, aad)?;
        let cipher_key = new_cipher_key(&dkey)?;

        let nr = noncebased::Reader::new(noncebased::ReaderParams {
            r,
            segment_decrypter: Box::new(AesGcmHkdfSegmentDecrypter { cipher_key }),
            nonce_size: AES_GCM_HKDF_NONCE_SIZE_IN_BYTES,
            nonce_prefix,
            ciphertext_segment_size: self.ciphertext_segment_size,
            first_ciphertext_segment_offset: self.first_ciphertext_segment_offset,
        })?;

        Ok(Box::new(nr))
    }
}

/// Create a new AES-GCM cipher key using the given key and the crypto library.
fn new_cipher_key(key: &[u8]) -> Result<AesGcmKeyVariant, TinkError> {
    match key.len() {
        16 => Ok(AesGcmKeyVariant::Aes128(Box::new(aes_gcm::Aes128Gcm::new(
            GenericArray::from_slice(key),
        )))),
        32 => Ok(AesGcmKeyVariant::Aes256(Box::new(aes_gcm::Aes256Gcm::new(
            GenericArray::from_slice(key),
        )))),
        l => Err(format!("AesGcmHmac: invalid AES key size {} (want 16, 32)", l).into()),
    }
}

/// A [`noncebased::SegmentEncrypter`] based on AES-GCM-HKDF.
struct AesGcmHkdfSegmentEncrypter {
    cipher_key: AesGcmKeyVariant,
}

impl noncebased::SegmentEncrypter for AesGcmHkdfSegmentEncrypter {
    fn encrypt_segment(&self, segment: &[u8], nonce: &[u8]) -> Result<Vec<u8>, TinkError> {
        let iv = GenericArray::from_slice(nonce);
        match &self.cipher_key {
            AesGcmKeyVariant::Aes128(key) => key.encrypt(iv, segment),
            AesGcmKeyVariant::Aes256(key) => key.encrypt(iv, segment),
        }
        .map_err(|e| wrap_err("AesGcmHkdf: encryption failed", e))
    }
}

/// A [`noncebased::SegmentDecrypter`] based on AES-GCM-HKDF.
struct AesGcmHkdfSegmentDecrypter {
    cipher_key: AesGcmKeyVariant,
}

impl noncebased::SegmentDecrypter for AesGcmHkdfSegmentDecrypter {
    fn decrypt_segment(&self, segment: &[u8], nonce: &[u8]) -> Result<Vec<u8>, TinkError> {
        let iv = GenericArray::from_slice(nonce);
        match &self.cipher_key {
            AesGcmKeyVariant::Aes128(key) => key.decrypt(iv, segment),
            AesGcmKeyVariant::Aes256(key) => key.decrypt(iv, segment),
        }
        .map_err(|e| wrap_err("AesGcmHkdf: decryption failed", e))
    }
}
