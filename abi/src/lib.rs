pub mod publickey;

use std::str::FromStr;

use base64::Engine;
use serde::{Deserialize, Serialize};

const BOOLEAN_MIDEN_WIDTH: u32 = 1;
const UINT32_MIDEN_WIDTH: u32 = 1;
const UINT64_MIDEN_WIDTH: u32 = 2;
const INT32_MIDEN_WIDTH: u32 = 1;
const INT64_MIDEN_WIDTH: u32 = 2;
const FLOAT32_MIDEN_WIDTH: u32 = 1;
const FLOAT64_MIDEN_WIDTH: u32 = 2;
const STRING_MIDEN_WIDTH: u32 = 2;
const BYTES_MIDEN_WIDTH: u32 = 2;
const ARRAY_MIDEN_WIDTH: u32 = 3;
const MAP_MIDEN_WIDTH: u32 = ARRAY_MIDEN_WIDTH * 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StdVersion {
    #[serde(rename = "0.5.0")]
    V0_5_0,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Abi {
    pub this_addr: Option<u32>,
    pub this_type: Option<Type>,
    pub param_types: Vec<Type>,
    pub std_version: Option<StdVersion>,
}

impl Abi {
    pub fn default_this_value(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let Some(ref this_type) = self.this_type else {
            return Err("Missing this type".into());
        };

        Ok(this_type.default_value())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveType {
    Boolean,
    UInt32,
    UInt64,
    Int32,
    Int64,
    Float32,
    Float64,
}

impl PrimitiveType {
    pub const fn miden_width(&self) -> u32 {
        match self {
            PrimitiveType::Boolean => BOOLEAN_MIDEN_WIDTH,
            PrimitiveType::UInt32 => UINT32_MIDEN_WIDTH,
            PrimitiveType::UInt64 => UINT64_MIDEN_WIDTH,
            PrimitiveType::Int32 => INT32_MIDEN_WIDTH,
            PrimitiveType::Int64 => INT64_MIDEN_WIDTH,
            PrimitiveType::Float32 => FLOAT32_MIDEN_WIDTH,
            PrimitiveType::Float64 => FLOAT64_MIDEN_WIDTH,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Type {
    Nullable(Box<Type>),
    PrimitiveType(PrimitiveType),
    #[default]
    String,
    Bytes,
    CollectionReference {
        collection: String,
    },
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    /// A type that can contain a 4-field wide hash, such as one returned by `hmerge`
    Hash,
    PublicKey,
    Struct(Struct),
}

impl Type {
    pub fn miden_width(&self) -> u32 {
        match self {
            Type::Nullable(t) => 1 + t.miden_width(),
            Type::PrimitiveType(pt) => pt.miden_width(),
            Type::String => STRING_MIDEN_WIDTH,
            Type::Bytes => BYTES_MIDEN_WIDTH,
            Type::CollectionReference { .. } => BYTES_MIDEN_WIDTH,
            Type::Array(_) => ARRAY_MIDEN_WIDTH,
            Type::Map(_, _) => MAP_MIDEN_WIDTH,
            Type::Hash => 4,
            Type::PublicKey => publickey::WIDTH,
            Type::Struct(struct_) => struct_.fields.iter().map(|(_, t)| t.miden_width()).sum(),
        }
    }

    pub fn default_value(&self) -> Value {
        match &self {
            Type::Nullable(_) => Value::Nullable(None),
            Type::PrimitiveType(PrimitiveType::Boolean) => Value::Boolean(false),
            Type::PrimitiveType(PrimitiveType::UInt32) => Value::UInt32(0),
            Type::PrimitiveType(PrimitiveType::UInt64) => Value::UInt64(0),
            Type::PrimitiveType(PrimitiveType::Int32) => Value::Int32(0),
            Type::PrimitiveType(PrimitiveType::Int64) => Value::Int64(0),
            Type::PrimitiveType(PrimitiveType::Float32) => Value::Float32(0.0),
            Type::PrimitiveType(PrimitiveType::Float64) => Value::Float64(0.0),
            Type::String => Value::String("".to_owned()),
            Type::Bytes => Value::Bytes(vec![]),
            Type::CollectionReference { .. } => Value::CollectionReference(Vec::new()),
            Type::Array(_) => Value::Array(Vec::new()),
            Type::Map(_, _) => Value::Map(Vec::new()),
            Type::Hash => Value::Hash([0; 4]),
            Type::PublicKey => Value::PublicKey(publickey::Key::default()),
            Type::Struct(t) => Value::StructValue(
                t.fields
                    .iter()
                    .map(|(n, t)| (n.clone(), t.default_value()))
                    .collect(),
            ),
        }
    }
}

type MemoryReader<'a> = dyn Fn(u64) -> Option<[u64; 4]> + 'a;

pub trait TypeReader {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value, Box<dyn std::error::Error>>;
}

#[derive(Debug, PartialEq)]
pub enum Value {
    Nullable(Option<Box<Value>>),
    Boolean(bool),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Int32(i32),
    Int64(i64),
    Hash([u64; 4]),
    String(String),
    Bytes(Vec<u8>),
    CollectionReference(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    PublicKey(publickey::Key),
    StructValue(Vec<(String, Value)>),
}

impl Into<serde_json::Value> for Value {
    fn into(self) -> serde_json::Value {
        match self {
            Value::Nullable(opt) => match opt {
                None => serde_json::Value::Null,
                Some(v) => (*v).into(),
            },
            Value::Boolean(b) => serde_json::Value::Bool(b),
            Value::UInt32(x) => serde_json::Value::Number(x.into()),
            Value::UInt64(x) => serde_json::Value::Number(x.into()),
            Value::Int32(x) => serde_json::Value::Number(x.into()),
            Value::Int64(x) => serde_json::Value::Number(x.into()),
            Value::Float32(x) => {
                serde_json::Value::Number(serde_json::Number::from_str(&x.to_string()).unwrap())
            }
            Value::Float64(x) => {
                serde_json::Value::Number(serde_json::Number::from_str(&x.to_string()).unwrap())
            }
            Value::Hash(h) => {
                let mut s = String::new();
                for x in h.iter() {
                    s.push_str(&format!("{:016x}", x));
                }
                serde_json::Value::String(s)
            }
            Value::String(s) => serde_json::Value::String(s),
            Value::Bytes(b) => serde_json::Value::String(format!(
                "\"{}\"",
                base64::engine::general_purpose::STANDARD.encode(&b)
            )),
            Value::CollectionReference(cr) => {
                let cr = String::from_utf8(cr).unwrap();
                // let parts = cr.split('|');
                // let collection_id = parts.clone().next().unwrap();
                // let id = parts.clone().nth(1).unwrap();
                let id = cr;

                let mut map = serde_json::Map::new();
                // map.insert("collectionId".to_string(), collection_id.into());
                map.insert("id".to_string(), id.into());

                serde_json::Value::Object(map)
            }
            Value::Array(a) => {
                let mut array = Vec::new();
                for value in a {
                    array.push(value.into());
                }
                serde_json::Value::Array(array)
            }
            Value::Map(m) => {
                let mut map = serde_json::Map::new();
                for (key, value) in m {
                    let key = key.maybe_to_string().unwrap();
                    map.insert(key.into(), value.into());
                }
                serde_json::Value::Object(map)
            }
            Value::PublicKey(pk) => serde_json::to_value(pk).unwrap(),
            Value::StructValue(sv) => {
                let mut map = serde_json::Map::new();
                for (name, value) in sv {
                    map.insert(name, value.into());
                }
                serde_json::Value::Object(map)
            }
        }
    }
}

impl TypeReader for PrimitiveType {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(match self {
            PrimitiveType::Boolean => {
                let [b, _, _, _] = reader(addr).ok_or("invalid address for boolean")?;
                assert!(b == 0 || b == 1);
                Value::Boolean(b != 0)
            }
            PrimitiveType::UInt32 => {
                let [x, _, _, _] = reader(addr).ok_or("invalid address for uint32")?;
                Value::UInt32(u32::try_from(x).unwrap())
            }
            PrimitiveType::UInt64 => {
                let [high, _, _, _] = reader(addr).ok_or("invalid address for uint64")?;
                let [low, _, _, _] = reader(addr + 1).ok_or("invalid address for uint64")?;

                Value::UInt64((high << 32) | low)
            }
            PrimitiveType::Int32 => {
                let [x, _, _, _] = reader(addr).ok_or("invalid address for int32")?;
                Value::Int32(x as i32)
            }
            PrimitiveType::Int64 => {
                let [high, _, _, _] = reader(addr).ok_or("invalid address for int64")?;
                let [low, _, _, _] = reader(addr + 1).ok_or("invalid address for int64")?;

                Value::Int64(((high << 32) | low) as i64)
            }
            PrimitiveType::Float32 => {
                let [bits, _, _, _] = reader(addr).ok_or("invalid address for float32")?;
                Value::Float32(f32::from_bits(bits as u32))
            }
            PrimitiveType::Float64 => {
                let [high, _, _, _] = reader(addr).ok_or("invalid address for float32")?;
                let [low, _, _, _] = reader(addr + 1).ok_or("invalid address for float32")?;

                Value::Float64(f64::from_bits((high << 32) | low))
            }
        })
    }
}

impl TypeReader for Struct {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value, Box<dyn std::error::Error>> {
        let mut fields = Vec::new();
        let mut current_addr = addr;
        for (name, type_) in &self.fields {
            let value = type_.read(reader, current_addr)?;
            fields.push((name.clone(), value));
            current_addr += type_.miden_width() as u64;
        }
        Ok(Value::StructValue(fields))
    }
}

impl TypeReader for Type {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value, Box<dyn std::error::Error>> {
        match self {
            Type::Nullable(t) => {
                let [is_null, _, _, _] = reader(addr).ok_or("invalid address for nullable")?;
                if is_null == 0 {
                    Ok(Value::Nullable(None))
                } else {
                    Ok(Value::Nullable(Some(Box::new(t.read(reader, addr + 1)?))))
                }
            }
            Type::PrimitiveType(pt) => pt.read(reader, addr),
            Type::Struct(s) => s.read(reader, addr),
            Type::Hash => Ok(reader(addr)
                .ok_or("invalid address for hash")
                .map(Value::Hash)?),
            Type::String => {
                let mut bytes = vec![];

                let length = reader(addr).ok_or("invalid address for string length")?[0];
                let data_ptr = reader(addr + 1).ok_or("invalid address for string data ptr")?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i).ok_or("invalid address for string byte")?[0];
                    bytes.push(byte as u8);
                }

                let string = String::from_utf8(bytes)?;

                Ok(Value::String(string))
            }
            Type::Bytes => {
                let mut bytes = vec![];

                let length = reader(addr).ok_or("invalid address for bytes length")?[0];
                let data_ptr = reader(addr + 1).ok_or("invalid address for bytes data ptr")?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i).ok_or("invalid address for bytes byte")?[0];
                    bytes.push(byte as u8);
                }

                Ok(Value::Bytes(bytes))
            }
            Type::CollectionReference { .. } => {
                let mut bytes = vec![];

                let length =
                    reader(addr).ok_or("invalid address for collection reference length")?[0];
                let data_ptr =
                    reader(addr + 1).ok_or("invalid address for collection reference data ptr")?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i)
                        .ok_or("invalid address for collection reference byte")?[0];
                    bytes.push(byte as u8);
                }

                Ok(Value::CollectionReference(bytes))
            }
            Type::Array(t) => {
                let mut values = vec![];

                let length = reader(addr + 1).ok_or("invalid address for array length")?[0];
                let data_ptr = reader(addr + 2).ok_or("invalid address for array data ptr")?[0];
                for i in 0..length {
                    let value = t.read(reader, data_ptr + i * t.miden_width() as u64)?;
                    values.push(value);
                }

                Ok(Value::Array(values))
            }
            Type::Map(k, v) => {
                let mut key_values = Vec::new();

                let key_array_data_start_ptr = reader(addr + 2).unwrap()[0];
                let value_array_data_start_ptr =
                    reader(addr + ARRAY_MIDEN_WIDTH as u64 + 2).unwrap()[0];
                let length = reader(addr + 1).ok_or("invalid address for map keys length")?[0];

                for i in 0..length {
                    let key = k.read(
                        reader,
                        key_array_data_start_ptr + i * k.miden_width() as u64,
                    )?;
                    let value = v.read(
                        reader,
                        value_array_data_start_ptr + i * v.miden_width() as u64,
                    )?;

                    key_values.push((key, value));
                }

                Ok(Value::Map(key_values))
            }
            Type::PublicKey => {
                let kty = reader(addr)
                    .map(|x| x[0])
                    .ok_or("invalid address for public key kty")?;
                let crv = reader(addr + 1)
                    .map(|x| x[0])
                    .ok_or("invalid address for public key crv")?;
                let alg = reader(addr + 2)
                    .map(|x| x[0])
                    .ok_or("invalid address for public key alg")?;
                let use_ = reader(addr + 3)
                    .map(|x| x[0])
                    .ok_or("invalid address for public key use")?;
                let extra_ptr = reader(addr + 4)
                    .map(|x| x[0])
                    .ok_or("invalid address for public key extra ptr")?;

                let mut extra_bytes = vec![];
                for i in 0..64 {
                    let byte = reader(extra_ptr + i)
                        .map(|x| x[0])
                        .ok_or("invalid address for public key extra byte")?;
                    extra_bytes.push(byte as u8);
                }

                let x = extra_bytes[0..32].try_into()?;
                let y = extra_bytes[32..64].try_into()?;

                let key = publickey::Key {
                    kty: (kty as u8).into(),
                    crv: (crv as u8).into(),
                    alg: (alg as u8).into(),
                    use_: (use_ as u8).into(),
                    x,
                    y,
                };

                Ok(Value::PublicKey(key))
            }
        }
    }
}

pub trait Parser<T: ?Sized> {
    fn parse(&self, value: &T) -> Result<Value, Box<dyn std::error::Error>>;
}

impl Parser<str> for PrimitiveType {
    fn parse(&self, value: &str) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(match self {
            PrimitiveType::Boolean => Value::Boolean(value.parse()?),
            PrimitiveType::UInt32 => Value::UInt32(value.parse()?),
            PrimitiveType::UInt64 => Value::UInt64(value.parse()?),
            PrimitiveType::Int32 => Value::Int32(value.parse()?),
            PrimitiveType::Int64 => Value::Int64(value.parse()?),
            PrimitiveType::Float32 => Value::Float32(value.parse()?),
            PrimitiveType::Float64 => Value::Float64(value.parse()?),
        })
    }
}

impl Parser<serde_json::Value> for PrimitiveType {
    fn parse(&self, value: &serde_json::Value) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(match self {
            PrimitiveType::Boolean => {
                Value::Boolean(value.as_bool().ok_or("invalid boolean value")?)
            }
            PrimitiveType::UInt32 => {
                Value::UInt32(value.as_u64().ok_or("invalid uint32 value")? as u32)
            }
            PrimitiveType::UInt64 => Value::UInt64(value.as_u64().ok_or("invalid uint64 value")?),
            PrimitiveType::Int32 => {
                Value::Int32(value.as_i64().ok_or("invalid int32 value")? as i32)
            }
            PrimitiveType::Int64 => Value::Int64(value.as_i64().ok_or("invalid int64 value")?),
            PrimitiveType::Float32 => {
                Value::Float32(value.as_f64().ok_or("invalid float32 value")? as f32)
            }
            PrimitiveType::Float64 => {
                Value::Float64(value.as_f64().ok_or("invalid float64 value")?)
            }
        })
    }
}

impl Parser<str> for Struct {
    fn parse(&self, value: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let mut fields = Vec::new();
        let mut value = value;
        for (name, type_) in &self.fields {
            let (field_value, rest) = value.split_once(',').ok_or("invalid value")?;
            fields.push((name.clone(), type_.parse(field_value)?));
            value = rest;
        }
        Ok(Value::StructValue(fields))
    }
}

impl Parser<serde_json::Value> for Struct {
    fn parse(&self, value: &serde_json::Value) -> Result<Value, Box<dyn std::error::Error>> {
        let mut fields = Vec::new();
        for (name, type_) in &self.fields {
            let field_value = value
                .get(name)
                .ok_or_else(|| format!("missing field {}", name))?;
            fields.push((name.clone(), type_.parse(field_value)?));
        }
        Ok(Value::StructValue(fields))
    }
}

impl Parser<str> for Type {
    fn parse(&self, value: &str) -> Result<Value, Box<dyn std::error::Error>> {
        match self {
            Type::Nullable(t) => {
                if value == "null" {
                    Ok(Value::Nullable(None))
                } else {
                    Ok(Value::Nullable(Some(Box::new(t.parse(value)?))))
                }
            }
            Type::PrimitiveType(pt) => pt.parse(value),
            Type::Struct(s) => s.parse(value),
            Type::Hash => {
                let mut bytes = vec![];
                if !value.is_empty() {
                    for byte in value.split(',') {
                        bytes.push(byte.parse()?);
                    }
                }
                let mut hash = [0; 4];
                hash.copy_from_slice(&bytes);
                Ok(Value::Hash(hash))
            }
            Type::String => Ok(Value::String(value.to_string())),
            Type::Bytes => {
                let mut bytes = vec![];
                if !value.is_empty() {
                    for byte in value.split(',') {
                        bytes.push(byte.parse()?);
                    }
                }
                Ok(Value::Bytes(bytes))
            }
            Type::CollectionReference { .. } => {
                let mut bytes = vec![];
                if !value.is_empty() {
                    for byte in value.split(',') {
                        bytes.push(byte.parse()?);
                    }
                }
                Ok(Value::CollectionReference(bytes))
            }
            Type::Array(t) => {
                let mut values = vec![];
                if !value.is_empty() {
                    for value in value.split(';') {
                        values.push(t.parse(value)?);
                    }
                }
                Ok(Value::Array(values))
            }
            Type::Map(k, v) => {
                let mut key_values = vec![];
                if !value.is_empty() {
                    let mut parts = value.split(';');
                    loop {
                        let Some(key) = parts.next() else {
                            break;
                        };

                        let value = parts.next().expect("Missing value in map");

                        key_values.push((k.parse(key)?, v.parse(value)?));
                    }
                }
                Ok(Value::Map(key_values))
            }
            Type::PublicKey => {
                let mut values = value.split(',');
                let kty = values.next().ok_or("missing kty")?;
                let crv = values.next().ok_or("missing crv")?;
                let alg = values.next().ok_or("missing alg")?;
                let use_ = values.next().ok_or("missing use")?;
                let x_base64 = values.next().ok_or("missing x")?;
                let y_base64 = values.next().ok_or("missing y")?;

                let x = base64::engine::general_purpose::URL_SAFE.decode(x_base64)?;
                let y = base64::engine::general_purpose::URL_SAFE.decode(y_base64)?;

                let mut extra_bytes = vec![];
                extra_bytes.extend_from_slice(&x);
                extra_bytes.extend_from_slice(&y);

                let key = publickey::Key {
                    kty: kty.parse().map_err(|_| "invalid kty")?,
                    crv: crv.parse().map_err(|_| "invalid crv")?,
                    alg: alg.parse().map_err(|_| "invalid alg")?,
                    use_: use_.parse().map_err(|_| "invalid use")?,
                    x: x.try_into().map_err(|_| "invalid x")?,
                    y: y.try_into().map_err(|_| "invalid y")?,
                };

                Ok(Value::PublicKey(key))
            }
        }
    }
}

impl Parser<serde_json::Value> for Type {
    fn parse(&self, value: &serde_json::Value) -> Result<Value, Box<dyn std::error::Error>> {
        match self {
            Type::Nullable(t) => {
                if value.is_null() {
                    Ok(Value::Nullable(None))
                } else {
                    Ok(Value::Nullable(Some(Box::new(t.parse(value)?))))
                }
            }
            Type::PrimitiveType(pt) => pt.parse(value),
            Type::Struct(s) => s.parse(value),
            Type::Hash => {
                let mut hash = [0u64; 4];
                if !value.is_null() {
                    let hex = value.as_str().ok_or("invalid hash")?;
                    let hex = hex.trim_start_matches("0x");

                    let mut bytes = vec![];
                    for byte in hex.as_bytes().chunks(16) {
                        let mut byte = byte.to_vec();
                        byte.resize(16, b'0');
                        bytes.push(byte);
                    }

                    for (i, byte) in bytes.iter().enumerate() {
                        hash[i] = u64::from_str_radix(std::str::from_utf8(byte)?, 16)?;
                    }

                    hash.reverse();
                }
                Ok(Value::Hash(hash))
            }
            Type::String => Ok(Value::String(
                value.as_str().ok_or("invalid string")?.to_string(),
            )),
            Type::Bytes => {
                let mut bytes = vec![];
                if !value.is_null() {
                    let bytes_str = value.as_str().ok_or("invalid bytes")?;
                    let bytes_str = bytes_str.trim_start_matches("0x");
                    let bytes_str = bytes_str.trim_start_matches("0X");
                    let bytes_str = bytes_str.trim_end_matches('"');
                    let bytes_str = bytes_str.trim_start_matches('"');
                    let bytes_str = bytes_str.trim_start_matches("0x");
                    let bytes_str = bytes_str.trim_start_matches("0X");
                    let bytes_str = bytes_str.trim();
                    let bytes_str = bytes_str.trim_start_matches("0x");
                    let bytes_str = bytes_str.trim_start_matches("0X");
                    let bytes_str = bytes_str.trim();
                    for byte_str in bytes_str.split(',') {
                        bytes.push(byte_str.parse()?);
                    }
                }
                Ok(Value::Bytes(bytes))
            }
            Type::CollectionReference { .. } => {
                let mut bytes = vec![];
                if !value.is_null() {
                    // let collection_id = value
                    //     .get("collectionId")
                    //     .ok_or("invalid collection reference")?
                    //     .as_str()
                    //     .ok_or("invalid collection reference")?;
                    let id = value
                        .get("id")
                        .ok_or("invalid collection reference")?
                        .as_str()
                        .ok_or("invalid collection reference")?;
                    // bytes.extend_from_slice(collection_id.as_bytes());
                    // bytes.extend_from_slice(b"|");
                    bytes.extend_from_slice(id.as_bytes());
                }
                Ok(Value::CollectionReference(bytes))
            }
            Type::Array(t) => {
                let mut values = vec![];
                if !value.is_null() {
                    for value in value.as_array().ok_or("invalid array")? {
                        values.push(t.parse(value)?);
                    }
                }
                Ok(Value::Array(values))
            }
            Type::Map(k, v) => {
                let mut key_values = vec![];
                if !value.is_null() {
                    for (key, value) in value.as_object().ok_or("invalid map")? {
                        key_values.push((k.parse(key.as_str())?, v.parse(value)?));
                    }
                }
                Ok(Value::Map(key_values))
            }
            Type::PublicKey => {
                let kty = value
                    .get("kty")
                    .ok_or("missing kty")?
                    .as_str()
                    .ok_or("invalid kty")?;
                let crv = value
                    .get("crv")
                    .ok_or("missing crv")?
                    .as_str()
                    .ok_or("invalid crv")?;
                let alg = value
                    .get("alg")
                    .ok_or("missing alg")?
                    .as_str()
                    .ok_or("invalid alg")?;
                let use_ = value
                    .get("use")
                    .ok_or("missing use")?
                    .as_str()
                    .ok_or("invalid use")?;
                let x_base64 = value
                    .get("x")
                    .ok_or("missing x")?
                    .as_str()
                    .ok_or("invalid x")?;
                let y_base64 = value
                    .get("y")
                    .ok_or("missing y")?
                    .as_str()
                    .ok_or("invalid y")?;

                let x = base64::engine::general_purpose::URL_SAFE.decode(x_base64)?;
                let y = base64::engine::general_purpose::URL_SAFE.decode(y_base64)?;

                let mut extra_bytes = vec![];
                extra_bytes.extend_from_slice(&x);
                extra_bytes.extend_from_slice(&y);

                let key = publickey::Key {
                    kty: kty.parse().map_err(|_| "invalid kty")?,
                    crv: crv.parse().map_err(|_| "invalid crv")?,
                    alg: alg.parse().map_err(|_| "invalid alg")?,
                    use_: use_.parse().map_err(|_| "invalid use")?,
                    x: x.try_into().map_err(|_| "invalid x")?,
                    y: y.try_into().map_err(|_| "invalid y")?,
                };

                Ok(Value::PublicKey(key))
            }
        }
    }
}

impl Value {
    pub fn serialize(&self) -> Vec<u64> {
        match self {
            Value::Nullable(opt) => match opt {
                None => vec![0],
                Some(v) => [1].into_iter().chain(v.serialize().into_iter()).collect(),
            },
            Value::Boolean(b) => vec![*b as u64],
            Value::UInt32(x) => vec![u64::from(*x)],
            Value::UInt64(x) => vec![*x >> 32, *x & 0xffffffff],
            Value::Int32(x) => vec![*x as u32 as u64],
            Value::Int64(x) => vec![(*x >> 32) as u64, *x as u64],
            Value::Float32(x) => vec![x.to_bits() as u64],
            Value::Float64(x) => vec![
                (x.to_bits() >> 32) as u64,
                (x.to_bits() & 0xffffffff) as u64,
            ],
            Value::Hash(h) => h.to_vec(),
            Value::String(s) => [s.len() as u64]
                .into_iter()
                .chain(s.bytes().map(|b| b as u64))
                .collect(),
            Value::Bytes(b) => [b.len() as u64]
                .into_iter()
                .chain(b.iter().map(|b| *b as u64))
                .collect(),
            Value::Array(values) => [values.len() as u64]
                .into_iter()
                .chain(values.iter().flat_map(|v| v.serialize()))
                .collect(),
            // Map is serialized as [keys_arr..., values_arr...] so that we can reuse read_advice_array
            Value::Map(key_values) => []
                .into_iter()
                .chain([key_values.len() as u64])
                .chain(key_values.iter().flat_map(|(k, _)| k.serialize()))
                .chain([key_values.len() as u64])
                .chain(key_values.iter().flat_map(|(_, v)| v.serialize()))
                .collect(),
            Value::CollectionReference(cr) => [cr.len() as u64]
                .into_iter()
                .chain(cr.iter().map(|b| *b as u64))
                .collect(),
            Value::PublicKey(k) => vec![
                u8::from(k.kty) as u64,
                u8::from(k.crv) as u64,
                u8::from(k.alg) as u64,
                u8::from(k.use_) as u64,
            ]
            .into_iter()
            .chain(k.x.iter().map(|b| *b as u64))
            .chain(k.y.iter().map(|b| *b as u64))
            .collect(),
            Value::StructValue(sv) => sv
                .iter()
                .flat_map(|(_, v)| v.serialize())
                .collect::<Vec<_>>(),
        }
    }

    fn maybe_to_string(self) -> Option<String> {
        match self {
            Value::Nullable(_) => None,
            Value::Boolean(true) => Some("true".to_owned()),
            Value::Boolean(false) => Some("false".to_owned()),
            Value::UInt32(x) => Some(x.to_string()),
            Value::UInt64(x) => Some(x.to_string()),
            Value::Float32(x) => Some(x.to_string()),
            Value::Float64(x) => Some(x.to_string()),
            Value::Int32(x) => Some(x.to_string()),
            Value::Int64(x) => Some(x.to_string()),
            Value::Hash(_) => None,
            Value::String(s) => Some(s),
            Value::Bytes(_) => None,
            Value::CollectionReference(_) => None,
            Value::Array(_) => None,
            Value::Map(_) => None,
            Value::PublicKey(_) => None,
            Value::StructValue(_) => None,
        }
    }
}
