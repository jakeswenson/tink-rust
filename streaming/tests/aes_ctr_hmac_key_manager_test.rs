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

use prost::Message;
use std::collections::HashSet;
use tink::{proto::HashType, TinkError};
use tink_streaming_aead::subtle;
use tink_testutil::proto_encode;

mod common;
use common::encrypt_decrypt;

const AES_CTR_HMAC_KEY_SIZES: [u32; 2] = [16, 32];

#[test]
fn test_aes_ctr_hmac_get_primitive_basic() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    for key_size in &AES_CTR_HMAC_KEY_SIZES {
        let key = tink_testutil::new_aes_ctr_hmac_key(
            tink_testutil::AES_CTR_HMAC_KEY_VERSION,
            *key_size,
            HashType::Sha256,
            *key_size,
            HashType::Sha256,
            16,
            4096,
        );
        let serialized_key = proto_encode(&key);
        let p = match key_manager.primitive(&serialized_key) {
            Ok(tink::Primitive::StreamingAead(p)) => p,
            _ => unreachable!(),
        };
        encrypt_decrypt(p.box_clone(), p, 32, 32).unwrap();
    }
}

#[test]
fn test_aes_ctr_hmac_get_primitive_with_invalid_input() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");

    let test_keys = gen_invalid_aes_ctr_hmac_keys();
    for (i, serialized_key) in test_keys.iter().enumerate() {
        assert!(
            key_manager.primitive(serialized_key).is_err(),
            "expect an error in test case {}",
            i
        );
    }

    assert!(
        key_manager.primitive(&[]).is_err(),
        "expect an error when input is empty"
    );
}

#[test]
fn test_aes_ctr_hmac_new_key_multiple_times() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    let format = tink_testutil::new_aes_ctr_hmac_key_format(
        32,
        HashType::Sha256,
        32,
        HashType::Sha256,
        16,
        4096,
    );
    let serialized_format = proto_encode(&format);
    let mut keys = HashSet::new();
    let n = 26;
    for _i in 0..n {
        let key = key_manager.new_key(&serialized_format).unwrap();
        let serialized_key = proto_encode(&key);
        keys.insert(serialized_key);

        let key_data = key_manager.new_key_data(&serialized_format).unwrap();
        let serialized_key = key_data.value;
        keys.insert(serialized_key);
    }
    assert_eq!(keys.len(), n * 2, "key is repeated");
}

#[test]
fn test_aes_ctr_hmac_new_key_basic() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    for key_size in &AES_CTR_HMAC_KEY_SIZES {
        let format = tink_testutil::new_aes_ctr_hmac_key_format(
            *key_size,
            HashType::Sha256,
            *key_size,
            HashType::Sha256,
            16,
            4096,
        );
        let serialized_format = proto_encode(&format);
        let serialized_key = key_manager.new_key(&serialized_format).unwrap();
        let key = tink::proto::AesCtrHmacStreamingKey::decode(serialized_key.as_ref()).unwrap();
        validate_aes_ctr_hmac_key(&key, &format).unwrap();
    }
}

#[test]
fn test_aes_ctr_hmac_new_key_with_invalid_input() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    // bad format
    let bad_formats = gen_invalid_aes_ctr_hmac_key_formats();
    for (i, serialized_format) in bad_formats.iter().enumerate() {
        assert!(
            key_manager.new_key(serialized_format).is_err(),
            "expect an error in test case {}",
            i
        );
    }
    // empty array
    assert!(
        key_manager.new_key(&[]).is_err(),
        "expect an error when input is empty"
    );
}

#[test]
fn test_aes_ctr_hmac_new_key_data_basic() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    for key_size in &AES_CTR_HMAC_KEY_SIZES {
        let format = tink_testutil::new_aes_ctr_hmac_key_format(
            *key_size,
            HashType::Sha256,
            *key_size,
            HashType::Sha256,
            16,
            4096,
        );
        let serialized_format = proto_encode(&format);
        let key_data = key_manager.new_key_data(&serialized_format).unwrap();
        assert_eq!(
            key_data.type_url,
            tink_testutil::AES_CTR_HMAC_TYPE_URL,
            "incorrect type url"
        );
        assert_eq!(
            key_data.key_material_type,
            tink::proto::key_data::KeyMaterialType::Symmetric as i32,
            "incorrect key material type"
        );
        let key = tink::proto::AesCtrHmacStreamingKey::decode(key_data.value.as_ref())
            .expect("incorrect key value");
        validate_aes_ctr_hmac_key(&key, &format).unwrap();
    }
}

#[test]
fn test_aes_ctr_hmac_new_key_data_with_invalid_input() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    let bad_formats = gen_invalid_aes_ctr_hmac_key_formats();
    for (i, serialized_format) in bad_formats.iter().enumerate() {
        assert!(
            key_manager.new_key_data(serialized_format).is_err(),
            "expect an error in test case {}",
            i
        );
    }
    // empty input
    assert!(
        key_manager.new_key_data(&[]).is_err(),
        "expect an error when input is empty"
    );
}

#[test]
fn test_aes_ctr_hmac_does_support() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    assert!(
        key_manager.does_support(tink_testutil::AES_CTR_HMAC_TYPE_URL),
        "AesCtrHmacKeyManager must support {}",
        tink_testutil::AES_CTR_HMAC_TYPE_URL,
    );
    assert!(
        !key_manager.does_support("some bad type"),
        "AesCtrHmacKeyManager must support only {}",
        tink_testutil::AES_CTR_HMAC_TYPE_URL,
    );
}

#[test]
fn test_aes_ctr_hmac_type_url() {
    tink_streaming_aead::init();
    let key_manager = tink::registry::get_key_manager(tink_testutil::AES_CTR_HMAC_TYPE_URL)
        .expect("cannot obtain AES-CTR-HMAC key manager");
    assert_eq!(
        key_manager.type_url(),
        tink_testutil::AES_CTR_HMAC_TYPE_URL,
        "incorrect key type"
    );
}

fn gen_invalid_aes_ctr_hmac_keys() -> Vec<Vec<u8>> {
    vec![
        // not a AES_CTR_HMACKey
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key_format(
            32,
            HashType::Sha256,
            32,
            HashType::Sha256,
            16,
            4096,
        )),
        // bad key size
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key(
            tink_testutil::AES_CTR_HMAC_KEY_VERSION,
            17,
            HashType::Sha256,
            16,
            HashType::Sha256,
            16,
            4096,
        )),
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key(
            tink_testutil::AES_CTR_HMAC_KEY_VERSION,
            16,
            HashType::Sha256,
            17,
            HashType::Sha256,
            16,
            4096,
        )),
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key(
            tink_testutil::AES_CTR_HMAC_KEY_VERSION,
            33,
            HashType::Sha256,
            33,
            HashType::Sha256,
            16,
            4096,
        )),
        // bad version
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key(
            tink_testutil::AES_CTR_HMAC_KEY_VERSION + 1,
            16,
            HashType::Sha256,
            16,
            HashType::Sha256,
            16,
            4096,
        )),
    ]
}

fn gen_invalid_aes_ctr_hmac_key_formats() -> Vec<Vec<u8>> {
    vec![
        // not AesCtrHmacKeyFormat
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key(
            tink_testutil::AES_CTR_HMAC_KEY_VERSION,
            16,
            HashType::Sha256,
            16,
            HashType::Sha256,
            16,
            4096,
        )),
        // invalid key size
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key_format(
            17,
            HashType::Sha256,
            16,
            HashType::Sha256,
            16,
            4096,
        )),
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key_format(
            16,
            HashType::Sha256,
            17,
            HashType::Sha256,
            16,
            4096,
        )),
        proto_encode(&tink_testutil::new_aes_ctr_hmac_key_format(
            33,
            HashType::Sha256,
            33,
            HashType::Sha256,
            16,
            4096,
        )),
    ]
}

fn validate_aes_ctr_hmac_key(
    key: &tink::proto::AesCtrHmacStreamingKey,
    format: &tink::proto::AesCtrHmacStreamingKeyFormat,
) -> Result<(), TinkError> {
    if key.key_value.len() != (format.key_size as usize) {
        return Err("incorrect key size".into());
    }
    if key.version != tink_testutil::AES_CTR_HMAC_KEY_VERSION {
        return Err("incorrect key version".into());
    }
    let key_params = key
        .params
        .as_ref()
        .ok_or_else(|| TinkError::new("no params"))?;
    let format_params = format
        .params
        .as_ref()
        .ok_or_else(|| TinkError::new("no params"))?;
    if key_params.ciphertext_segment_size != format_params.ciphertext_segment_size {
        return Err("incorrect ciphertext segment size".into());
    }
    if key_params.derived_key_size != format_params.derived_key_size {
        return Err("incorrect derived key size".into());
    }
    if key_params.hkdf_hash_type != format_params.hkdf_hash_type {
        return Err("incorrect HKDF hash type".into());
    }
    let hkdf_hash_type = HashType::from_i32(key_params.hkdf_hash_type)
        .ok_or_else(|| TinkError::new("invalid HKDF hash"))?;
    let hmac_params = key_params
        .hmac_params
        .as_ref()
        .ok_or_else(|| TinkError::new("no params"))?;
    let hmac_hash =
        HashType::from_i32(hmac_params.hash).ok_or_else(|| TinkError::new("invalid HMAC hash"))?;
    // try to encrypt and decrypt
    let p = subtle::AesCtrHmac::new(
        &key.key_value,
        hkdf_hash_type,
        key_params.derived_key_size as usize,
        hmac_hash,
        hmac_params.tag_size as usize,
        key_params.ciphertext_segment_size as usize,
        0,
    )
    .expect("invalid key");
    validate_aes_ctr_hmac_primitive(p, key)
}

fn validate_aes_ctr_hmac_primitive(
    cipher: subtle::AesCtrHmac,
    key: &tink::proto::AesCtrHmacStreamingKey,
) -> Result<(), TinkError> {
    if cipher.main_key != key.key_value {
        return Err("main key and primitive don't match".into());
    }
    encrypt_decrypt(Box::new(cipher.clone()), Box::new(cipher), 32, 32)
}
