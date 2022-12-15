use std::collections::HashMap;

use super::{PrimitiveType, Struct, Type};

type MemoryReader<'a> = dyn Fn(u64) -> Option<[u64; 4]> + 'a;

pub trait TypeReader {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value, Box<dyn std::error::Error>>;
}

#[derive(Debug)]
pub enum Value {
    Boolean(bool),
    UInt32(u32),
    UInt64(u64),
    Hash([u64; 4]),
    Int32(i32),
    String(String),
    StructValue(Vec<(String, Value)>),
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
            PrimitiveType::Int32 => {
                let [x, _, _, _] = reader(addr).ok_or("invalid address for int32")?;
                Value::Int32(i32::try_from(x).unwrap())
            }
            PrimitiveType::UInt64 => {
                let [high, low, _, _] = reader(addr).ok_or("invalid address for uint64")?;
                Value::UInt64((high << 32) | low)
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
        }
    }
}

pub trait Parser {
    fn parse(&self, value: &str) -> Result<Value, Box<dyn std::error::Error>>;
}

impl Parser for PrimitiveType {
    fn parse(&self, value: &str) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(match self {
            PrimitiveType::Boolean => Value::Boolean(value.parse()?),
            PrimitiveType::UInt32 => Value::UInt32(value.parse()?),
            PrimitiveType::Int32 => Value::Int32(value.parse()?),
            PrimitiveType::UInt64 => Value::UInt64(value.parse()?),
        })
    }
}

impl Parser for Struct {
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

impl Parser for Type {
    fn parse(&self, value: &str) -> Result<Value, Box<dyn std::error::Error>> {
        match self {
            Type::PrimitiveType(pt) => pt.parse(value),
            Type::Struct(s) => s.parse(value),
            Type::Hash => {
                let mut bytes = vec![];
                for byte in value.split(',') {
                    bytes.push(byte.parse()?);
                }
                let mut hash = [0; 4];
                hash.copy_from_slice(&bytes);
                Ok(Value::Hash(hash))
            }
            Type::String => Ok(Value::String(value.to_string())),
        }
    }
}

impl Value {
    pub fn serialize(&self) -> Vec<u64> {
        match self {
            Value::Boolean(b) => vec![*b as u64],
            Value::UInt32(x) => vec![u64::from(*x)],
            Value::UInt64(x) => vec![*x >> 32, *x & 0xffffffff],
            Value::Int32(x) => vec![*x as u64],
            Value::Hash(h) => h.to_vec(),
            Value::String(s) => [s.len() as u64]
                .into_iter()
                .chain(s.bytes().map(|b| b as u64))
                .collect(),
            Value::StructValue(sv) => sv
                .iter()
                .flat_map(|(_, v)| v.serialize())
                .collect::<Vec<_>>(),
        }
    }
}
