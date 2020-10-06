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

//! Handle wrapper for keysets.

use crate::{proto::key_data::KeyMaterialType, utils::wrap_err, TinkError};
use prost::Message;
use std::sync::Arc;

/// `Handle` provides access to a [`Keyset`](crate::proto::Keyset) protobuf, to limit the exposure
/// of actual protocol buffers that hold sensitive key material.
pub struct Handle {
    pub(crate) ks: crate::proto::Keyset,
}

impl Handle {
    /// Create a keyset handle that contains a single fresh key generated according
    /// to the given [`KeyTemplate`](crate::proto::KeyTemplate).
    pub fn new(kt: &crate::proto::KeyTemplate) -> Result<Self, TinkError> {
        let mut ksm = super::Manager::new();
        ksm.rotate(kt)
            .map_err(|e| wrap_err("keyset::Handle: cannot generate new keyset", e))?;
        ksm.handle()
            .map_err(|e| wrap_err("keyset::Handle: cannot get keyset handle", e))
    }

    /// Create a new instance of [`Handle`] using the given [`Keyset`](crate::proto::Keyset) which
    /// does not contain any secret key material.
    pub fn new_with_no_secrets(ks: crate::proto::Keyset) -> Result<Self, TinkError> {
        let h = Handle { ks };
        if h.has_secrets() {
            // If you need to do this, you have to use `tink::keyset::insecure::read()` instead.
            return Err("importing unencrypted secret key material is forbidden".into());
        }
        Ok(h)
    }

    /// Attempt to create a [`Handle`] from an encrypted keyset obtained via a
    /// [`Reader`](crate::keyset::Reader).
    pub fn read<T, S>(reader: &mut T, master_key: &S) -> Result<Self, TinkError>
    where
        T: crate::keyset::Reader,
        S: crate::Aead,
    {
        let encrypted_keyset = reader.read_encrypted()?;
        let ks = decrypt(&encrypted_keyset, master_key)?;
        Ok(Handle { ks })
    }

    /// Attempt to create a [`Handle`] from a keyset obtained via a
    /// [`Reader`](crate::keyset::Reader).
    pub fn read_with_no_secrets<T>(reader: &mut T) -> Result<Self, TinkError>
    where
        T: crate::keyset::Reader,
    {
        let ks = reader.read()?;
        Handle::new_with_no_secrets(ks)
    }

    /// Return a [`Handle`] of the public keys if the managed keyset contains private keys.
    pub fn public(&self) -> Result<Self, TinkError> {
        let priv_keys = &self.ks.key;
        let mut pub_keys = Vec::with_capacity(priv_keys.len());
        for priv_key in priv_keys {
            let priv_key_data = match &priv_key.key_data {
                None => return Err("keyset::Handle: invalid keyset".into()),
                Some(kd) => kd,
            };
            let pub_key_data =
                public_key_data(priv_key_data).map_err(|e| wrap_err("keyset::Handle", e))?;
            pub_keys.push(crate::proto::keyset::Key {
                key_data: Some(pub_key_data),
                status: priv_key.status,
                key_id: priv_key.key_id,
                output_prefix_type: priv_key.output_prefix_type,
            });
        }
        let ks = crate::proto::Keyset {
            primary_key_id: self.ks.primary_key_id,
            key: pub_keys,
        };
        Ok(Handle { ks })
    }

    /// Encrypts and writes the enclosed [`Keyset`](crate::proto::Keyset).
    pub fn write<T, S>(&self, writer: &mut T, master_key: S) -> Result<(), TinkError>
    where
        T: super::Writer,
        S: crate::Aead,
    {
        let encrypted = encrypt(&self.ks, master_key)?;
        writer.write_encrypted(&encrypted)
    }

    /// Export the keyset in `h` to the given [`Writer`](super::Writer) returning an error if the
    /// keyset contains secret key material.
    pub fn write_with_no_secrets<T>(&self, w: &mut T) -> Result<(), TinkError>
    where
        T: super::Writer,
    {
        if self.has_secrets() {
            Err("exporting unencrypted secret key material is forbidden".into())
        } else {
            w.write(&self.ks)
        }
    }

    /// Create a set of primitives corresponding to the keys with status=ENABLED in the keyset of
    /// the given keyset [`Handle`], assuming all the corresponding key managers are present (keys
    /// with status!=ENABLED are skipped).
    ///
    /// The returned set is usually later "wrapped" into a class that implements the corresponding
    /// [`Primitive`](crate::Primitive) interface.
    pub fn primitives(&self) -> Result<crate::primitiveset::PrimitiveSet, TinkError> {
        self.primitives_with_key_manager(None)
    }

    /// Create a set of primitives corresponding to the keys with status=ENABLED in the keyset of
    /// the given keyset [`Handle`], using the given key manager (instead of registered key
    /// managers) for keys supported by it.  Keys not supported by the key manager are handled
    /// by matching registered key managers (if present), and keys with status!=ENABLED are
    /// skipped.
    ///
    /// This enables custom treatment of keys, for example providing extra context (e.g. credentials
    /// for accessing keys managed by a KMS), or gathering custom monitoring/profiling
    /// information.
    ///
    /// The returned set is usually later "wrapped" into a class that implements the corresponding
    /// [`Primitive`](crate::Primitive)-interface.
    pub fn primitives_with_key_manager(
        &self,
        km: Option<Arc<dyn crate::registry::KeyManager>>,
    ) -> Result<crate::primitiveset::PrimitiveSet, TinkError> {
        super::validate(&self.ks)
            .map_err(|e| wrap_err("primitives_with_key_manager: invalid keyset", e))?;
        let mut primitive_set = crate::primitiveset::PrimitiveSet::new();
        for key in &self.ks.key {
            if key.status != crate::proto::KeyStatusType::Enabled as i32 {
                continue;
            }
            let key_data = key
                .key_data
                .as_ref()
                .ok_or_else(|| TinkError::new("primitives_with_key_manager: no key_data"))?;
            let primitive = match &km {
                Some(km) if km.does_support(&key_data.type_url) => km.primitive(&key_data.value),
                Some(_) | None => crate::registry::primitive_from_key_data(&key_data),
            }
            .map_err(|e| {
                wrap_err(
                    "primitives_with_key_manager: cannot get primitive from key",
                    e,
                )
            })?;

            let entry = primitive_set
                .add(primitive, key)
                .map_err(|e| wrap_err("primitives_with_key_manager: cannot add primitive", e))?;
            if key.key_id == self.ks.primary_key_id {
                primitive_set.primary = Some(entry.clone());
            }
        }
        Ok(primitive_set)
    }

    /// Check if the keyset handle contains any key material considered secret.
    /// Both symmetric keys and the private key of an asymmetric crypto system are considered secret
    /// keys. Also returns true when encountering any errors.
    fn has_secrets(&self) -> bool {
        for k in &self.ks.key {
            match &k.key_data {
                None => continue,
                Some(kd) => match KeyMaterialType::from_i32(kd.key_material_type) {
                    Some(KeyMaterialType::UnknownKeymaterial) => return true,
                    Some(KeyMaterialType::Symmetric) => return true,
                    Some(KeyMaterialType::AsymmetricPrivate) => return true,
                    Some(KeyMaterialType::AsymmetricPublic) => continue,
                    Some(KeyMaterialType::Remote) => continue,
                    None => return true,
                },
            }
        }
        false
    }
}

/// Extract the public key data corresponding to private key data.
fn public_key_data(
    priv_key_data: &crate::proto::KeyData,
) -> Result<crate::proto::KeyData, TinkError> {
    if priv_key_data.key_material_type
        != crate::proto::key_data::KeyMaterialType::AsymmetricPrivate as i32
    {
        return Err("keyset::Handle: keyset contains a non-private key".into());
    }
    let km = crate::registry::get_key_manager(&priv_key_data.type_url)?;

    if !km.supports_private_keys() {
        return Err(format!(
            "keyset::Handle: {} does not belong to a KeyManager that handles private keys",
            priv_key_data.type_url
        )
        .into());
    }
    km.public_key_data(&priv_key_data.value)
}

/// Decrypt a keyset with a master key.
fn decrypt<T>(
    encrypted_keyset: &crate::proto::EncryptedKeyset,
    master_key: &T,
) -> Result<crate::proto::Keyset, TinkError>
where
    T: crate::Aead,
{
    let decrypted = master_key
        .decrypt(&encrypted_keyset.encrypted_keyset, &[])
        .map_err(|e| wrap_err("keyset::Handle: decryption failed", e))?;
    crate::proto::Keyset::decode(&decrypted[..])
        .map_err(|_| TinkError::new("keyset::Handle:: invalid keyset"))
}

/// Encrypt a keyset with a master key.
fn encrypt<T>(
    keyset: &crate::proto::Keyset,
    master_key: T,
) -> Result<crate::proto::EncryptedKeyset, TinkError>
where
    T: crate::Aead,
{
    let mut serialized_keyset = vec![];
    keyset
        .encode(&mut serialized_keyset)
        .map_err(|e| wrap_err("keyset::Handle: invalid keyset", e))?;
    let encrypted = master_key
        .encrypt(&serialized_keyset, &[])
        .map_err(|e| wrap_err("keyset::Handle: encrypted failed", e))?;
    Ok(crate::proto::EncryptedKeyset {
        encrypted_keyset: encrypted,
        keyset_info: Some(get_keyset_info(keyset)),
    })
}

/// Return a [`KeysetInfo`](crate::proto::KeysetInfo) from a [`Keyset`](crate::proto::Keyset)
/// protobuf.
fn get_keyset_info(keyset: &crate::proto::Keyset) -> crate::proto::KeysetInfo {
    let n_key = keyset.key.len();
    let mut key_infos = Vec::with_capacity(n_key);
    for key in &keyset.key {
        key_infos.push(get_key_info(key));
    }
    crate::proto::KeysetInfo {
        primary_key_id: keyset.primary_key_id,
        key_info: key_infos,
    }
}

/// Return a [`KeyInfo`](crate::proto::keyset_info::KeyInfo) from a
/// [`Key`](crate::proto::keyset::Key) protobuf.
fn get_key_info(key: &crate::proto::keyset::Key) -> crate::proto::keyset_info::KeyInfo {
    crate::proto::keyset_info::KeyInfo {
        type_url: key
            .key_data
            .as_ref()
            .expect("key with no key_data")
            .type_url
            .clone(),
        status: key.status,
        key_id: key.key_id,
        output_prefix_type: key.output_prefix_type,
    }
}

impl std::fmt::Debug for Handle {
    /// Return a string representation of the managed keyset.
    /// The result does not contain any sensitive key material.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", get_keyset_info(&self.ks))
    }
}