pub mod publickey;

use std::str::FromStr;

use base64::Engine;
use serde::{Deserialize, Serialize};

use error::prelude::*;

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
    #[serde(rename = "0.6.1")]
    V0_6_1,
    #[serde(rename = "0.7.0")]
    V0_7_0,
}

/// An array of record hashes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecordHashes {
    pub contract: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Abi {
    pub std_version: Option<StdVersion>,
    pub this_addr: Option<u32>,
    pub this_type: Option<Type>,
    pub result_addr: Option<u32>,
    pub result_type: Option<Type>,
    pub param_types: Vec<Type>,
    pub other_records: Vec<RecordHashes>,
    pub other_contract_types: Vec<Type>,
    pub dependent_fields: Vec<(String, Type)>,
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
    ContractReference {
        contract: String,
    },
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    /// A type that can contain a 4-field wide hash, such as one returned by `hmerge`
    Hash,
    /// A hash that holds 8 field elements. Returned by sha256's and blake3's hash function.
    Hash8,
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
            Type::ContractReference { .. } => BYTES_MIDEN_WIDTH,
            Type::Array(_) => ARRAY_MIDEN_WIDTH,
            Type::Map(_, _) => MAP_MIDEN_WIDTH,
            Type::Hash => 4,
            Type::Hash8 => 8,
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
            Type::ContractReference { .. } => Value::ContractReference(Vec::new()),
            Type::Array(_) => Value::Array(Vec::new()),
            Type::Map(_, _) => Value::Map(Vec::new()),
            Type::Hash => Value::Hash([0; 4]),
            Type::Hash8 => Value::Hash8([0; 8]),
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
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value>;
}

#[derive(Debug, PartialEq, Clone)]
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
    Hash8([u64; 8]),
    String(String),
    Bytes(Vec<u8>),
    ContractReference(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    PublicKey(publickey::Key),
    StructValue(Vec<(String, Value)>),
}

impl Value {
    pub fn visit<E>(&self, visitor: &mut impl FnMut(&Value) -> Result<(), E>) -> Result<(), E> {
        visitor(self)?;
        match self {
            Value::Nullable(Some(v)) => {
                v.visit(visitor)?;
            }
            Value::Array(a) => {
                for value in a {
                    value.visit(visitor)?;
                }
            }
            Value::Map(m) => {
                for (key, value) in m {
                    key.visit(visitor)?;
                    value.visit(visitor)?;
                }
            }
            Value::StructValue(sv) => {
                for (_, value) in sv {
                    value.visit(visitor)?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl TryInto<serde_json::Value> for Value {
    type Error = Error;
    fn try_into(self) -> Result<serde_json::Value> {
        Ok(match self {
            Value::Nullable(opt) => match opt {
                None => serde_json::Value::Null,
                Some(v) => (*v).try_into()?,
            },
            Value::Boolean(b) => serde_json::Value::Bool(b),
            Value::UInt32(x) => serde_json::Value::Number(x.into()),
            Value::UInt64(x) => serde_json::Value::Number(x.into()),
            Value::Int32(x) => serde_json::Value::Number(x.into()),
            Value::Int64(x) => serde_json::Value::Number(x.into()),
            Value::Float32(x) => {
                serde_json::Value::Number(serde_json::Number::from_str(&x.to_string()).wrap_err()?)
            }
            Value::Float64(x) => {
                serde_json::Value::Number(serde_json::Number::from_str(&x.to_string()).wrap_err()?)
            }
            Value::Hash(h) => {
                let mut s = String::new();
                for x in h.iter() {
                    s.push_str(&format!("{:016x}", x));
                }
                serde_json::Value::String(s)
            }
            Value::Hash8(h) => {
                let mut s = String::new();
                for x in h.iter() {
                    s.push_str(&format!("{:016x}", x));
                }
                serde_json::Value::String(s)
            }
            Value::String(s) => serde_json::Value::String(s),
            Value::Bytes(b) => serde_json::Value::String(format!(
                "\"{}\"",
                base64::engine::general_purpose::STANDARD.encode(b)
            )),
            Value::ContractReference(cr) => {
                let cr = String::from_utf8(cr).wrap_err()?;
                // let parts = cr.split('|');
                // let contract_id = parts.clone().next().unwrap();
                // let id = parts.clone().nth(1).unwrap();
                let id = cr;

                let mut map = serde_json::Map::new();
                // map.insert("contractId".to_string(), contract_id.into());
                map.insert("id".to_string(), id.into());

                serde_json::Value::Object(map)
            }
            Value::Array(a) => {
                let mut array = Vec::new();
                for value in a {
                    array.push(value.try_into()?);
                }
                serde_json::Value::Array(array)
            }
            Value::Map(m) => {
                let mut map = serde_json::Map::new();
                for (key, value) in m
                    .into_iter()
                    .filter_map(|(k, v)| Some((k.maybe_to_string()?, v)))
                {
                    map.insert(key, value.try_into()?);
                }
                serde_json::Value::Object(map)
            }
            Value::PublicKey(pk) => serde_json::to_value(pk).wrap_err()?,
            Value::StructValue(sv) => {
                let mut map = serde_json::Map::new();
                for (name, value) in sv {
                    map.insert(name, value.try_into()?);
                }
                serde_json::Value::Object(map)
            }
        })
    }
}

impl TypeReader for PrimitiveType {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value> {
        Ok(match self {
            PrimitiveType::Boolean => {
                let [b, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "boolean",
                })?;
                snafu::ensure!(
                    b == 0 || b == 1,
                    TypeMismatchSnafu {
                        context: "tried to use boolean that is not 0, nor 1",
                    }
                );
                Value::Boolean(b != 0)
            }
            PrimitiveType::UInt32 => {
                let [x, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "uint32",
                })?;
                Value::UInt32(u32::try_from(x).wrap_err()?)
            }
            PrimitiveType::UInt64 => {
                let [high, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "uint64",
                })?;
                let [low, _, _, _] = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "uint64",
                })?;

                Value::UInt64((high << 32) | low)
            }
            PrimitiveType::Int32 => {
                let [x, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "int32",
                })?;
                Value::Int32(x as i32)
            }
            PrimitiveType::Int64 => {
                let [high, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "int64",
                })?;
                let [low, _, _, _] = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "int64",
                })?;
                Value::Int64(((high << 32) | low) as i64)
            }
            PrimitiveType::Float32 => {
                let [bits, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "float32",
                })?;
                Value::Float32(f32::from_bits(bits as u32))
            }
            PrimitiveType::Float64 => {
                let [high, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "float64",
                })?;
                let [low, _, _, _] = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "float64",
                })?;

                Value::Float64(f64::from_bits((high << 32) | low))
            }
        })
    }
}

impl TypeReader for Struct {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value> {
        let mut fields = Vec::new();
        let mut current_addr = addr;
        for (name, type_) in &self.fields {
            let value = type_
                .read(reader, current_addr)
                .nest_err(|| format!("invalid read of field {name}"))?;
            fields.push((name.clone(), value));
            current_addr += u64::from(type_.miden_width());
        }
        Ok(Value::StructValue(fields))
    }
}

impl TypeReader for Type {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value> {
        match self {
            Type::Nullable(t) => {
                let [is_null, _, _, _] = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "nullable",
                })?;
                if is_null == 0 {
                    Ok(Value::Nullable(None))
                } else {
                    Ok(Value::Nullable(Some(Box::new(t.read(reader, addr + 1)?))))
                }
            }
            Type::PrimitiveType(pt) => pt.read(reader, addr),
            Type::Struct(s) => s.read(reader, addr),
            Type::Hash => reader(addr)
                .map(Value::Hash)
                .context(InvalidAddressSnafu {
                    addr,
                    type_name: "hash",
                })
                .map_err(Into::into),
            Type::Hash8 => {
                let mut hash = [0u64; 8];
                for i in 0..2 {
                    let [a, b, c, d] = reader(addr + i * 4).context(InvalidAddressSnafu {
                        addr,
                        type_name: "hash8",
                    })?;

                    let offset = (i * 4) as usize;
                    hash[offset] = a;
                    hash[offset + 1] = b;
                    hash[offset + 2] = c;
                    hash[offset + 3] = d;
                }

                Ok(Value::Hash8(hash))
            }
            Type::String => {
                let mut bytes = vec![];

                let length = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "string length",
                })?[0];
                let data_ptr = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "string data ptr",
                })?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i).context(InvalidAddressSnafu {
                        addr,
                        type_name: "string byte",
                    })?[0];
                    bytes.push(byte as u8);
                }

                let string = String::from_utf8(bytes).wrap_err()?;

                Ok(Value::String(string))
            }
            Type::Bytes => {
                let mut bytes = vec![];

                let length = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "bytes length",
                })?[0];
                let data_ptr = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "bytes data ptr",
                })?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i).context(InvalidAddressSnafu {
                        addr,
                        type_name: "bytes byte",
                    })?[0];
                    bytes.push(byte as u8);
                }

                Ok(Value::Bytes(bytes))
            }
            Type::ContractReference { .. } => {
                let mut bytes = vec![];

                let length = reader(addr).context(InvalidAddressSnafu {
                    addr,
                    type_name: "contract reference length",
                })?[0];
                let data_ptr = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "contract reference data ptr",
                })?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i).context(InvalidAddressSnafu {
                        addr,
                        type_name: "contract reference byte",
                    })?[0];
                    bytes.push(byte as u8);
                }

                Ok(Value::ContractReference(bytes))
            }
            Type::Array(t) => {
                let mut values = vec![];

                let length = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "array length",
                })?[0];
                let data_ptr = reader(addr + 2).context(InvalidAddressSnafu {
                    addr,
                    type_name: "array data ptr",
                })?[0];
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
                let length = reader(addr + 1).context(InvalidAddressSnafu {
                    addr,
                    type_name: "map keys length",
                })?[0];

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
                let kty = reader(addr).map(|x| x[0]).context(InvalidAddressSnafu {
                    addr,
                    type_name: "public key kty",
                })?;
                let crv = reader(addr + 1)
                    .map(|x| x[0])
                    .context(InvalidAddressSnafu {
                        addr,
                        type_name: "public key crv",
                    })?;
                let alg = reader(addr + 2)
                    .map(|x| x[0])
                    .context(InvalidAddressSnafu {
                        addr,
                        type_name: "public key alg",
                    })?;
                let use_ = reader(addr + 3)
                    .map(|x| x[0])
                    .context(InvalidAddressSnafu {
                        addr,
                        type_name: "public key use",
                    })?;
                let extra_ptr = reader(addr + 4)
                    .map(|x| x[0])
                    .context(InvalidAddressSnafu {
                        addr,
                        type_name: "public key extra ptr",
                    })?;

                let mut extra_bytes = [0; 64];
                for i in 0..64 {
                    let byte =
                        reader(extra_ptr + i)
                            .map(|x| x[0])
                            .context(InvalidAddressSnafu {
                                addr,
                                type_name: "public key extra byte",
                            })?;
                    extra_bytes[i as usize] = byte as u8;
                }

                let x = extra_bytes[0..32].try_into().unwrap();
                let y = extra_bytes[32..64].try_into().unwrap();

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
    fn parse(&self, value: &T) -> Result<Value>;
}

impl Parser<str> for PrimitiveType {
    fn parse(&self, value: &str) -> Result<Value> {
        match self {
            PrimitiveType::Boolean => value
                .parse()
                .map(Value::Boolean)
                .parse_err("Boolean", value),
            PrimitiveType::UInt32 => value.parse().map(Value::UInt32).parse_err("UInt32", value),
            PrimitiveType::UInt64 => value.parse().map(Value::UInt64).parse_err("UInt64", value),
            PrimitiveType::Int32 => value.parse().map(Value::Int32).parse_err("Int32", value),
            PrimitiveType::Int64 => value.parse().map(Value::Int64).parse_err("Int64", value),
            PrimitiveType::Float32 => value
                .parse()
                .map(Value::Float32)
                .parse_err("Float32", value),
            PrimitiveType::Float64 => value
                .parse()
                .map(Value::Float64)
                .parse_err("Float64", value),
        }
    }
}

impl Parser<serde_json::Value> for PrimitiveType {
    fn parse(&self, value: &serde_json::Value) -> Result<Value> {
        let reason = "invalid json value";
        Ok(match self {
            PrimitiveType::Boolean => Value::Boolean(value.as_bool().parse_err(
                reason,
                "boolean",
                format!("{value}").as_str(),
            )?),
            PrimitiveType::UInt32 => Value::UInt32(value.as_u64().parse_err(
                reason,
                "uint32",
                format!("{value}").as_str(),
            )? as u32),
            PrimitiveType::UInt64 => Value::UInt64(value.as_u64().parse_err(
                reason,
                "uint64",
                format!("{value}").as_str(),
            )?),
            PrimitiveType::Int32 => Value::Int32(value.as_i64().parse_err(
                reason,
                "int32",
                format!("{value}").as_str(),
            )? as i32),
            PrimitiveType::Int64 => Value::Int64(value.as_i64().parse_err(
                reason,
                "int64",
                format!("{value}").as_str(),
            )?),
            PrimitiveType::Float32 => Value::Float32(value.as_f64().parse_err(
                reason,
                "float32",
                format!("{value}").as_str(),
            )? as f32),
            PrimitiveType::Float64 => Value::Float64(value.as_f64().parse_err(
                reason,
                "float64",
                format!("{value}").as_str(),
            )?),
        })
    }
}

impl Parser<str> for Struct {
    fn parse(&self, value: &str) -> Result<Value> {
        let mut fields = Vec::new();
        let mut value = value;
        for (name, type_) in &self.fields {
            let (field_value, rest) = value
                .split_once(',')
                .parse_err("missing field comma", "struct", value)
                .nest_err(|| format!("field {name}"))?;
            fields.push((
                name.clone(),
                type_
                    .parse(field_value)
                    .nest_err(|| format!("field {name}"))?,
            ));
            value = rest;
        }
        Ok(Value::StructValue(fields))
    }
}

impl Parser<serde_json::Value> for Struct {
    fn parse(&self, value: &serde_json::Value) -> Result<Value> {
        let mut fields = Vec::new();
        for (name, type_) in &self.fields {
            let field_value = value.get(name).parse_err("missing", "field", name)?;
            fields.push((name.clone(), type_.parse(field_value)?));
        }
        Ok(Value::StructValue(fields))
    }
}

impl Parser<str> for Type {
    fn parse(&self, value: &str) -> Result<Value> {
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
                        bytes.push(byte.parse().parse_err("hash", value)?);
                    }
                }
                let mut hash = [0; 4];
                hash.copy_from_slice(&bytes);
                Ok(Value::Hash(hash))
            }
            Type::Hash8 => {
                let mut bytes = vec![];
                if !value.is_empty() {
                    for byte in value.split(',') {
                        bytes.push(byte.parse().parse_err("hash8", value)?);
                    }
                }
                let mut hash = [0; 8];
                hash.copy_from_slice(&bytes);
                Ok(Value::Hash8(hash))
            }
            Type::String => Ok(Value::String(value.to_string())),
            Type::Bytes => {
                let mut bytes = vec![];
                if !value.is_empty() {
                    for byte in value.split(',') {
                        bytes.push(byte.parse().parse_err("bytes", value)?);
                    }
                }
                Ok(Value::Bytes(bytes))
            }
            Type::ContractReference { .. } => {
                let mut bytes = vec![];
                if !value.is_empty() {
                    for byte in value.split(',') {
                        bytes.push(byte.parse().parse_err("contract reference", value)?);
                    }
                }
                Ok(Value::ContractReference(bytes))
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

                        let value = parts
                            .next()
                            .ok_or_else(|| Error::simple("missing value in map"))?;

                        key_values.push((k.parse(key)?, v.parse(value)?));
                    }
                }
                Ok(Value::Map(key_values))
            }
            Type::PublicKey => {
                let mut values = value.split(',');
                let kty = values
                    .next()
                    .parse_err("missing field", "kty of public key", value)?;
                let crv = values
                    .next()
                    .parse_err("missing field", "crv of public key", value)?;
                let alg = values
                    .next()
                    .parse_err("missing field", "alg of public key", value)?;
                let use_ = values
                    .next()
                    .parse_err("missing field", "use of public key", value)?;
                let x_base64 =
                    values
                        .next()
                        .parse_err("missing field", "x of public key", value)?;
                let y_base64 =
                    values
                        .next()
                        .parse_err("missing field", "y of public key", value)?;

                let x = base64::engine::general_purpose::URL_SAFE
                    .decode(x_base64)
                    .wrap_err()?;
                let y = base64::engine::general_purpose::URL_SAFE
                    .decode(y_base64)
                    .wrap_err()?;

                let mut extra_bytes = vec![];
                extra_bytes.extend_from_slice(&x);
                extra_bytes.extend_from_slice(&y);

                let key = publickey::Key {
                    kty: kty.parse().parse_err("kty", kty)?,
                    crv: crv.parse().parse_err("crv", crv)?,
                    alg: alg.parse().parse_err("alg", alg)?,
                    use_: use_.parse().parse_err("use", use_)?,
                    x: x.try_into().ok().parse_err("invalid size", "x", x_base64)?,
                    y: y.try_into().ok().parse_err("invalid size", "y", y_base64)?,
                };

                Ok(Value::PublicKey(key))
            }
        }
    }
}

impl Parser<serde_json::Value> for Type {
    fn parse(&self, value: &serde_json::Value) -> Result<Value> {
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
                    let hex = value.as_str().parse_err("invalid", "hash", "json")?;
                    let hex = hex.trim_start_matches("0x");

                    let mut bytes = vec![];
                    for byte in hex.as_bytes().chunks(16) {
                        let mut byte = byte.to_vec();
                        byte.resize(16, b'0');
                        bytes.push(byte);
                    }

                    for (i, byte) in bytes.iter().enumerate() {
                        hash[i] = u64::from_str_radix(std::str::from_utf8(byte).wrap_err()?, 16)
                            .wrap_err()?;
                    }

                    hash.reverse();
                }
                Ok(Value::Hash(hash))
            }
            Type::Hash8 => {
                let mut hash = [0u64; 8];
                if !value.is_null() {
                    let hex = value.as_str().parse_err("invalid", "hash8", "json")?;
                    let hex = hex.trim_start_matches("0x");

                    let mut bytes = vec![];
                    for byte in hex.as_bytes().chunks(16) {
                        let mut byte = byte.to_vec();
                        byte.resize(16, b'0');
                        bytes.push(byte);
                    }

                    for (i, byte) in bytes.iter().enumerate() {
                        hash[i] = u64::from_str_radix(std::str::from_utf8(byte).wrap_err()?, 16)
                            .wrap_err()?;
                    }

                    hash.reverse();
                }
                Ok(Value::Hash8(hash))
            }
            Type::String => Ok(Value::String(
                value
                    .as_str()
                    .parse_err("invalid", "string", "json")?
                    .to_string(),
            )),
            Type::Bytes => {
                let mut bytes = vec![];
                if !value.is_null() {
                    let bytes_str = value.as_str().parse_err("invalid", "string", "json")?;
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
                        bytes.push(byte_str.parse().wrap_err()?);
                    }
                }
                Ok(Value::Bytes(bytes))
            }
            Type::ContractReference { .. } => {
                let mut bytes = vec![];
                if !value.is_null() {
                    // let contract_id = value
                    //     .get("contractId")
                    //     .ok_or("invalid contract reference")?
                    //     .as_str()
                    //     .ok_or("invalid contract reference")?;
                    let id = value
                        .get("id")
                        .parse_err("missing", "contract reference", "json")?
                        .as_str()
                        .parse_err("invalid", "contract reference", "json")?;
                    // bytes.extend_from_slice(contract_id.as_bytes());
                    // bytes.extend_from_slice(b"|");
                    bytes.extend_from_slice(id.as_bytes());
                }
                Ok(Value::ContractReference(bytes))
            }
            Type::Array(t) => {
                let mut values = vec![];
                if !value.is_null() {
                    for value in value.as_array().parse_err("invalid", "array", "json")? {
                        values.push(t.parse(value)?);
                    }
                }
                Ok(Value::Array(values))
            }
            Type::Map(k, v) => {
                let mut key_values = vec![];
                if !value.is_null() {
                    for (key, value) in value.as_object().parse_err("invalid", "object", "json")? {
                        key_values.push((k.parse(key.as_str())?, v.parse(value)?));
                    }
                }
                Ok(Value::Map(key_values))
            }
            Type::PublicKey => {
                let kty = value
                    .get("kty")
                    .parse_err("missing field", "kty of public key", "json")?
                    .as_str()
                    .parse_err("invalidi", "kty", "json as str")?;
                let crv = value
                    .get("crv")
                    .parse_err("missing field", "crv of public key", "json")?
                    .as_str()
                    .parse_err("invalidi", "crv", "json as str")?;
                let alg = value
                    .get("alg")
                    .parse_err("missing field", "alg of public key", "json")?
                    .as_str()
                    .parse_err("invalidi", "alg", "json as str")?;
                let use_ = value
                    .get("use")
                    .parse_err("missing field", "use of public key", "json")?
                    .as_str()
                    .parse_err("invalidi", "use", "json as str")?;
                let x_base64 = value
                    .get("x")
                    .parse_err("missing field", "x of public key", "json")?
                    .as_str()
                    .parse_err("invalidi", "x", "json as str")?;
                let y_base64 = value
                    .get("y")
                    .parse_err("missing field", "y of public key", "json")?
                    .as_str()
                    .parse_err("invalidi", "y", "json as str")?;

                let x = base64::engine::general_purpose::URL_SAFE
                    .decode(x_base64)
                    .wrap_err()?;
                let y = base64::engine::general_purpose::URL_SAFE
                    .decode(y_base64)
                    .wrap_err()?;

                let mut extra_bytes = vec![];
                extra_bytes.extend_from_slice(&x);
                extra_bytes.extend_from_slice(&y);

                let key = publickey::Key {
                    kty: kty.parse().parse_err("kty", kty)?,
                    crv: crv.parse().parse_err("crv", crv)?,
                    alg: alg.parse().parse_err("alg", alg)?,
                    use_: use_.parse().parse_err("use", use_)?,
                    x: x.try_into().ok().parse_err("invalid size", "x", x_base64)?,
                    y: y.try_into().ok().parse_err("invalid size", "y", y_base64)?,
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
                Some(v) => [1].into_iter().chain(v.serialize()).collect(),
            },
            Value::Boolean(b) => vec![*b as u64],
            Value::UInt32(x) => vec![u64::from(*x)],
            Value::UInt64(x) => vec![*x >> 32, *x & 0xffffffff],
            Value::Int32(x) => vec![*x as u32 as u64],
            Value::Int64(x) => vec![(*x >> 32) as u64, *x as u64],
            Value::Float32(x) => vec![x.to_bits() as u64],
            Value::Float64(x) => vec![(x.to_bits() >> 32), (x.to_bits() & 0xffffffff)],
            Value::Hash(h) => h.to_vec(),
            Value::Hash8(h) => h.to_vec(),
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
            Value::ContractReference(cr) => [cr.len() as u64]
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
            Value::Hash8(_) => None,
            Value::String(s) => Some(s),
            Value::Bytes(_) => None,
            Value::ContractReference(_) => None,
            Value::Array(_) => None,
            Value::Map(_) => None,
            Value::PublicKey(_) => None,
            Value::StructValue(_) => None,
        }
    }
}
