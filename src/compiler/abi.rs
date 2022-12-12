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
    Int32(i32),
    String(String),
    StructValue(Vec<(String, Value)>),
}

impl TypeReader for PrimitiveType {
    fn read(&self, reader: &MemoryReader, addr: u64) -> Result<Value, Box<dyn std::error::Error>> {
        Ok(match self {
            PrimitiveType::Boolean => {
                let [b, _, _, _] = reader(addr).ok_or("invalid address")?;
                assert!(b == 0 || b == 1);
                Value::Boolean(b != 0)
            }
            PrimitiveType::UInt32 => {
                let [x, _, _, _] = reader(addr).ok_or("invalid address")?;
                Value::UInt32(u32::try_from(x).unwrap())
            }
            PrimitiveType::Int32 => {
                let [x, _, _, _] = reader(addr).ok_or("invalid address")?;
                Value::Int32(i32::try_from(x).unwrap())
            }
            PrimitiveType::UInt64 => {
                let [high, low, _, _] = reader(addr).ok_or("invalid address")?;
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
            Type::String => {
                let mut bytes = vec![];

                let length = reader(addr).ok_or("invalid address")?[0];
                let data_ptr = reader(addr + 1).ok_or("invalid address")?[0];
                for i in 0..length {
                    let byte = reader(data_ptr + i).ok_or("invalid address")?[0];
                    bytes.push(byte as u8);
                }

                let string = String::from_utf8(bytes)?;

                Ok(Value::String(string))
            }
        }
    }
}
