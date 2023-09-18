use std::str::FromStr;

use base64::Engine;
use serde::{Deserialize, Serialize};

use error::prelude::{whatever, Whatever};
use snafu::ResultExt;

/// Layout: [key, crv, alg, use, extra_ptr]
/// `extra_ptr` in secp256k1 is pointer to 64 bytes of data,
/// the x and y coordinates of the public key.
pub const WIDTH: u32 = 5;

// {"alg":"ES256K","crv":"secp256k1","kty":"EC","use":"sig","x":"TOz1M-Y1MVF6i7duA-aWbNSzwgiRngrMFViHOjR3O0w=","y":"XqGeNTl4BoJMANDK160xXhGjpRqy0bHqK_Rn-jsco1o="}d
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kty {
    #[default]
    EC,
}

impl From<Kty> for u8 {
    fn from(value: Kty) -> Self {
        match value {
            Kty::EC => 1,
        }
    }
}

impl From<u8> for Kty {
    fn from(value: u8) -> Self {
        match value {
            1 => Kty::EC,
            _ => panic!("invalid kty: {}", value),
        }
    }
}

impl FromStr for Kty {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "EC" => Ok(Kty::EC),
            _ => whatever!("invalid kty: {s}"),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Crv {
    #[default]
    Secp256k1,
}

impl From<Crv> for u8 {
    fn from(value: Crv) -> Self {
        match value {
            Crv::Secp256k1 => 1,
        }
    }
}

impl From<u8> for Crv {
    fn from(value: u8) -> Self {
        match value {
            1 => Crv::Secp256k1,
            _ => panic!("invalid crv: {}", value),
        }
    }
}

impl FromStr for Crv {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "secp256k1" => Ok(Crv::Secp256k1),
            _ => whatever!("invalid crv: {s}"),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Alg {
    #[default]
    ES256K,
}

impl From<Alg> for u8 {
    fn from(value: Alg) -> Self {
        match value {
            Alg::ES256K => 1,
        }
    }
}

impl From<u8> for Alg {
    fn from(value: u8) -> Self {
        match value {
            1 => Alg::ES256K,
            _ => panic!("invalid alg: {}", value),
        }
    }
}

impl FromStr for Alg {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ES256K" => Ok(Alg::ES256K),
            _ => whatever!("invalid alg: {s}"),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Use {
    #[default]
    Sig,
}

impl From<Use> for u8 {
    fn from(value: Use) -> Self {
        match value {
            Use::Sig => 1,
        }
    }
}

impl From<u8> for Use {
    fn from(value: u8) -> Self {
        match value {
            1 => Use::Sig,
            _ => panic!("invalid use: {}", value),
        }
    }
}

impl FromStr for Use {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sig" => Ok(Use::Sig),
            _ => whatever!("invalid use: {s}"),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct Key {
    pub kty: Kty,
    pub crv: Crv,
    pub alg: Alg,
    #[serde(rename = "use")]
    pub use_: Use,
    #[serde(
        serialize_with = "to_url_safe_base64",
        deserialize_with = "from_url_safe_base64"
    )]
    pub x: [u8; 32],
    #[serde(
        serialize_with = "to_url_safe_base64",
        deserialize_with = "from_url_safe_base64"
    )]
    pub y: [u8; 32],
}

fn to_url_safe_base64<S>(bytes: &[u8; 32], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&base64::engine::general_purpose::URL_SAFE.encode(bytes))
}

fn from_url_safe_base64<'de, D>(deserializer: D) -> std::result::Result<[u8; 32], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    base64::engine::general_purpose::URL_SAFE
        .decode(s.as_bytes())
        .map_err(serde::de::Error::custom)?
        .try_into()
        .map_err(|_| serde::de::Error::custom("invalid base64"))
}

impl Key {
    pub fn from_secp256k1_bytes(bytes: &[u8]) -> Result<Self, Whatever> {
        let mut x = [0; 32];
        let mut y = [0; 32];

        match bytes.len() {
            65 => {
                x.copy_from_slice(&bytes[1..33]);
                y.copy_from_slice(&bytes[33..]);
            }
            64 => {
                x.copy_from_slice(&bytes[..32]);
                y.copy_from_slice(&bytes[32..]);
            }
            33 => {
                let pk = secp256k1::PublicKey::from_slice(bytes).with_whatever_context(|e| {
                    format!("invalid secp256k1 public key bytes: {e}")
                })?;

                let uncompressed = pk.serialize_uncompressed();
                x.copy_from_slice(&uncompressed[1..33]);
                y.copy_from_slice(&uncompressed[33..]);
            }
            20 => whatever!("you provided an address, where a public key is expected. See https://ethereum.stackexchange.com/questions/13778/get-public-key-of-any-ethereum-account"),
            _ => whatever!(
                "invalid secp256k1 xy bytes length: {}. A key should be 65, 64 or 33 bytes long.",
                bytes.len()
            ),
        }

        Ok(Key {
            kty: Kty::EC,
            crv: Crv::Secp256k1,
            alg: Alg::ES256K,
            use_: Use::Sig,
            x,
            y,
        })
    }

    pub fn to_64_byte_hex(&self) -> String {
        format!("0x{}{}", hex::encode(&self.x), hex::encode(&self.y))
    }

    pub fn to_compressed_33_byte_hex(&self) -> String {
        let mut bytes = [0; 33];
        bytes[0] = 0x02 | (self.y[31] & 0x01);
        bytes[1..].copy_from_slice(&self.x);
        format!("0x{}", hex::encode(bytes))
    }
}
