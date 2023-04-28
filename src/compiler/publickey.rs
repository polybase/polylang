use std::str::FromStr;

use super::{encoder::Instruction, *};

/// Layout: [key, crv, alg, use, extra_ptr]
/// `extra_ptr` in secp256k1 is pointer to 64 bytes of data,
/// the x and y coordinates of the public key.
pub(crate) const WIDTH: u32 = 5;

// {"alg":"ES256K","crv":"secp256k1","kty":"EC","use":"sig","x":"TOz1M-Y1MVF6i7duA-aWbNSzwgiRngrMFViHOjR3O0w=","y":"XqGeNTl4BoJMANDK160xXhGjpRqy0bHqK_Rn-jsco1o="}d
#[derive(Debug, Copy, Clone)]
pub(crate) enum Kty {
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "EC" => Ok(Kty::EC),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Crv {
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "secp256k1" => Ok(Crv::Secp256k1),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Alg {
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ES256K" => Ok(Alg::ES256K),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Use {
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sig" => Ok(Use::Sig),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct Key {
    pub(crate) kty: Kty,
    pub(crate) crv: Crv,
    pub(crate) alg: Alg,
    pub(crate) use_: Use,
    pub(crate) x: [u8; 32],
    pub(crate) y: [u8; 32],
}

pub(crate) fn new(compiler: &mut Compiler, key: Key) -> Symbol {
    let symbol = compiler.memory.allocate_symbol(Type::PublicKey);
    let symbol_xy = compiler.memory.allocate(64);

    compiler.memory.write(
        &mut compiler.instructions,
        symbol.memory_addr,
        &[
            ValueSource::Immediate(u8::from(key.kty) as u32),
            ValueSource::Immediate(u8::from(key.crv) as u32),
            ValueSource::Immediate(u8::from(key.alg) as u32),
            ValueSource::Immediate(u8::from(key.use_) as u32),
            ValueSource::Immediate(symbol_xy),
        ],
    );

    compiler.memory.write(
        &mut compiler.instructions,
        symbol_xy,
        &key.x
            .iter()
            .chain(key.y.iter())
            .map(|c| ValueSource::Immediate(*c as u32))
            .collect::<Vec<_>>(),
    );

    symbol
}

pub(crate) fn kty(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        ..Default::default()
    }
}

pub(crate) fn crv(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 1,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        ..Default::default()
    }
}

pub(crate) fn alg(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 2,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        ..Default::default()
    }
}

pub(crate) fn use_(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 3,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        ..Default::default()
    }
}

pub(crate) fn extra_ptr(symbol: &Symbol) -> Symbol {
    Symbol {
        memory_addr: symbol.memory_addr + 4,
        type_: Type::PrimitiveType(PrimitiveType::UInt32),
        ..Default::default()
    }
}

pub(crate) fn eq(compiler: &mut Compiler, a: &Symbol, b: &Symbol) -> Symbol {
    let symbol = compiler
        .memory
        .allocate_symbol(Type::PrimitiveType(PrimitiveType::Boolean));

    compiler.instructions.push(Instruction::Push(1));
    for i in 0..4 {
        compiler
            .memory
            .read(&mut compiler.instructions, a.memory_addr + i, 1);

        compiler
            .memory
            .read(&mut compiler.instructions, b.memory_addr + i, 1);

        compiler.instructions.push(Instruction::Eq);
        compiler.instructions.push(Instruction::And);
    }

    for i in 0..64 {
        compiler
            .memory
            .read(&mut compiler.instructions, a.memory_addr + 4, 1);
        compiler.instructions.push(Instruction::Push(i as u32));
        compiler.instructions.push(Instruction::Add);
        compiler.instructions.push(Instruction::MemLoad(None));

        compiler
            .memory
            .read(&mut compiler.instructions, b.memory_addr + 4, 1);
        compiler.instructions.push(Instruction::Push(i as u32));
        compiler.instructions.push(Instruction::Add);
        compiler.instructions.push(Instruction::MemLoad(None));

        compiler.instructions.push(Instruction::Eq);
        compiler.instructions.push(Instruction::And);
    }

    compiler.memory.write(
        &mut compiler.instructions,
        symbol.memory_addr,
        &[ValueSource::Stack],
    );

    symbol
}
