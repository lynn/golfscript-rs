use crate::coerce::{coerce, Coerced};
use num::BigInt;
use num::One;
use num::Zero;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Gval {
    Int(BigInt),
    Arr(Vec<Gval>),
    Str(Vec<u8>),
    Blk(Vec<u8>),
}

impl From<u8> for Gval {
    fn from(byte: u8) -> Self {
        Gval::Int(byte.into())
    }
}

impl From<usize> for Gval {
    fn from(byte: usize) -> Self {
        Gval::Int(byte.into())
    }
}
impl Gval {
    pub fn bool(value: bool) -> Self {
        Gval::Int(if value { BigInt::one() } else { BigInt::zero() })
    }

    pub fn falsey(&self) -> bool {
        match self {
            Gval::Int(a) => *a == BigInt::zero(),
            Gval::Arr(vs) => vs.len() == 0,
            Gval::Str(bs) | Gval::Blk(bs) => bs.len() == 0,
        }
    }

    pub fn to_gs(self) -> Vec<u8> {
        match self {
            Gval::Int(a) => a.to_str_radix(10).into_bytes(),
            Gval::Arr(vs) => {
                let mut bytes: Vec<u8> = vec![];
                for v in vs {
                    bytes.extend(v.to_gs());
                }
                bytes
            }
            Gval::Str(bs) => bs,
            Gval::Blk(bs) => {
                let mut bytes: Vec<u8> = vec!['{' as u8];
                bytes.extend(bs);
                bytes.push('}' as u8);
                bytes
            }
        }
    }

    pub fn inspect(self) -> Vec<u8> {
        match self {
            Gval::Arr(vs) => {
                let mut bytes: Vec<u8> = vec![];
                for v in vs {
                    bytes.push(b' ');
                    bytes.extend(v.inspect());
                }
                bytes[0] = b'[';
                bytes.push(b']');
                bytes
            }
            Gval::Str(bs) => {
                let mut bytes: Vec<u8> = vec![b'"'];
                for b in bs {
                    if b == b'\'' {
                        bytes.push(b)
                    } else {
                        bytes.extend(std::ascii::escape_default(b))
                    }
                }
                bytes.push(b'"');
                bytes
            }
            _ => self.to_gs(),
        }
    }

    pub fn plus(self, other: Gval) -> Gval {
        match coerce(self, other) {
            Coerced::Ints(x, y) => Gval::Int(x + y),
            Coerced::Arrs(mut x, y) => {
                x.extend(y);
                Gval::Arr(x)
            }
            Coerced::Strs(mut x, y) => {
                x.extend(y);
                Gval::Str(x)
            }
            Coerced::Blks(x, y) => {
                let mut joined = x.clone();
                joined.push(b' ');
                joined.extend(y);
                Gval::Blk(joined)
            }
        }
    }
}

pub fn join(a: Vec<Gval>, sep: Gval) -> Gval {
    let mut a = a.into_iter();
    match a.next() {
        None => match sep {
            Gval::Arr(_) => Gval::Arr(vec![]),
            _ => Gval::Str(vec![]),
        },
        Some(mut r) => {
            for i in a {
                r = r.plus(sep.clone()).plus(i);
            }
            r
        }
    }
}

pub fn split<T: Clone + Eq>(a: Vec<T>, sep: Vec<T>, clean: bool) -> Vec<Vec<T>> {
    let mut r: Vec<Vec<T>> = vec![];
    let mut i: Vec<T> = vec![];
    let mut j: usize = 0;

    while j < a.len() {
        if j + sep.len() <= a.len() && a[j..j + sep.len()].iter().eq(sep.iter()) {
            if !clean || i.len() > 0 {
                r.push(i);
            }
            i = vec![];
            j += sep.len();
        } else {
            i.push(a[j].clone());
            j += 1;
        }
    }
    if !clean || i.len() > 0 {
        r.push(i);
    }
    r
}
