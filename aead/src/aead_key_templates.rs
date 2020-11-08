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

//! This module contains pre-generated [`KeyTemplate`]s for AEAD keys. One can use these templates
//! to generate new Keysets.

use prost::Message;
use tink::proto::{HashType, KeyTemplate, OutputPrefixType};

/// Return a [`KeyTemplate`] that generates an AES-GCM key with the following parameters:
///   - Key size: 16 bytes
///   - Output prefix type: TINK
pub fn aes128_gcm_key_template() -> KeyTemplate {
    create_aes_gcm_key_template(16, OutputPrefixType::Tink)
}

/// Return a [`KeyTemplate`] that generates an AES-GCM key with the following parameters:
///   - Key size: 32 bytes
///   - Output prefix type: TINK
pub fn aes256_gcm_key_template() -> KeyTemplate {
    create_aes_gcm_key_template(32, OutputPrefixType::Tink)
}

/// Return a [`KeyTemplate`] that generates an AES-GCM key with the following parameters:
///   - Key size: 32 bytes
///   - Output prefix type: RAW
pub fn aes256_gcm_no_prefix_key_template() -> KeyTemplate {
    create_aes_gcm_key_template(32, OutputPrefixType::Raw)
}

/// Return a [`KeyTemplate`] that generates an AES-GCM-SIV key with the following parameters:
///   - Key size: 16 bytes
///   - Output prefix type: TINK
pub fn aes128_gcm_siv_key_template() -> KeyTemplate {
    create_aes_gcm_siv_key_template(16, OutputPrefixType::Tink)
}

/// Return a [`KeyTemplate`] that generates an AES-GCM-SIV key with the following parameters:
///   - Key size: 32 bytes
///   - Output prefix type: TINK
pub fn aes256_gcm_siv_key_template() -> KeyTemplate {
    create_aes_gcm_siv_key_template(32, OutputPrefixType::Tink)
}

/// Return a [`KeyTemplate`] that generates an AES-GCM-SIV key with the following parameters:
///   - Key size: 32 bytes
///   - Output prefix type: RAW
pub fn aes256_gcm_siv_no_prefix_key_template() -> KeyTemplate {
    create_aes_gcm_siv_key_template(32, OutputPrefixType::Raw)
}

/// Return a [`KeyTemplate`] that generates an AES-CTR-HMAC-AEAD key with the following parameters:
///  - AES key size: 16 bytes
///  - AES CTR IV size: 16 bytes
///  - HMAC key size: 32 bytes
///  - HMAC tag size: 16 bytes
///  - HMAC hash function: SHA256
pub fn aes128_ctr_hmac_sha256_key_template() -> KeyTemplate {
    create_aes_ctr_hmac_aead_key_template(16, 16, 32, 16, HashType::Sha256)
}

/// Return a [`KeyTemplate`] that generates an AES-CTR-HMAC-AEAD key with the following parameters:
///  - AES key size: 32 bytes
///  - AES CTR IV size: 16 bytes
///  - HMAC key size: 32 bytes
///  - HMAC tag size: 32 bytes
///  - HMAC hash function: SHA256
pub fn aes256_ctr_hmac_sha256_key_template() -> KeyTemplate {
    create_aes_ctr_hmac_aead_key_template(32, 16, 32, 32, HashType::Sha256)
}

/// Return a [`KeyTemplate`] that generates a CHACHA20_POLY1305 key.
pub fn cha_cha20_poly1305_key_template() -> KeyTemplate {
    KeyTemplate {
        /// Don't set value because key_format is not required.
        value: vec![],
        type_url: crate::CHA_CHA20_POLY1305_TYPE_URL.to_string(),
        output_prefix_type: OutputPrefixType::Tink as i32,
    }
}

/// Return a [`KeyTemplate`] that generates a XCHACHA20_POLY1305 key.
pub fn x_cha_cha20_poly1305_key_template() -> KeyTemplate {
    KeyTemplate {
        /// Don't set value because key_format is not required.
        value: vec![],
        type_url: crate::X_CHA_CHA20_POLY1305_TYPE_URL.to_string(),
        output_prefix_type: OutputPrefixType::Tink as i32,
    }
}

/// Return a [`KeyTemplate`] that generates a KmsEnvelopeAead key for a given KEK in remote KMS
pub fn kms_envelope_aead_key_template(uri: &str, dek_t: KeyTemplate) -> KeyTemplate {
    let f = tink::proto::KmsEnvelopeAeadKeyFormat {
        kek_uri: uri.to_string(),
        dek_template: Some(dek_t),
    };
    let mut serialized_format = Vec::new();
    f.encode(&mut serialized_format).unwrap(); // safe: proto-encode
    KeyTemplate {
        value: serialized_format,
        type_url: crate::KMS_ENVELOPE_AEAD_TYPE_URL.to_string(),
        output_prefix_type: OutputPrefixType::Tink as i32,
    }
}

/// Return an AES-GCM key template with the given key size in bytes.
fn create_aes_gcm_key_template(key_size: u32, output_prefix_type: OutputPrefixType) -> KeyTemplate {
    let format = tink::proto::AesGcmKeyFormat {
        version: crate::AES_GCM_KEY_VERSION,
        key_size,
    };
    let mut serialized_format = Vec::new();
    format.encode(&mut serialized_format).unwrap(); // safe: proto-encode
    KeyTemplate {
        type_url: crate::AES_GCM_TYPE_URL.to_string(),
        value: serialized_format,
        output_prefix_type: output_prefix_type as i32,
    }
}

/// Return an AES-GCM-SIV key template with the given key size in bytes.
fn create_aes_gcm_siv_key_template(
    key_size: u32,
    output_prefix_type: OutputPrefixType,
) -> KeyTemplate {
    let format = tink::proto::AesGcmSivKeyFormat {
        version: crate::AES_GCM_SIV_KEY_VERSION,
        key_size,
    };
    let mut serialized_format = Vec::new();
    format.encode(&mut serialized_format).unwrap(); // safe: proto-encode
    KeyTemplate {
        type_url: crate::AES_GCM_SIV_TYPE_URL.to_string(),
        value: serialized_format,
        output_prefix_type: output_prefix_type as i32,
    }
}

/// Return an AES-CTR-HMAC key template with the given parameters.
fn create_aes_ctr_hmac_aead_key_template(
    aes_key_size: u32,
    iv_size: u32,
    hmac_key_size: u32,
    tag_size: u32,
    hash: HashType,
) -> KeyTemplate {
    let format = tink::proto::AesCtrHmacAeadKeyFormat {
        aes_ctr_key_format: Some(tink::proto::AesCtrKeyFormat {
            params: Some(tink::proto::AesCtrParams { iv_size }),
            key_size: aes_key_size,
        }),
        hmac_key_format: Some(tink::proto::HmacKeyFormat {
            version: crate::AES_CTR_HMAC_AEAD_KEY_VERSION,
            params: Some(tink::proto::HmacParams {
                hash: hash as i32,
                tag_size,
            }),
            key_size: hmac_key_size,
        }),
    };
    let mut serialized_format = Vec::new();
    format.encode(&mut serialized_format).unwrap(); // safe: proto-encode
    KeyTemplate {
        value: serialized_format,
        type_url: crate::AES_CTR_HMAC_AEAD_TYPE_URL.to_string(),
        output_prefix_type: OutputPrefixType::Tink as i32,
    }
}