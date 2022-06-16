use crate::coerce::flatten_append;
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

    pub fn truthy(&self) -> bool {
        !self.falsey()
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
                let mut bytes: Vec<u8> = vec![b'['];
                let mut s = false;
                for v in vs {
                    if s {
                        bytes.push(b' ');
                    }
                    s = true;
                    bytes.extend(v.inspect());
                }
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

    pub fn factory(&self) -> Gval {
        match self {
            Gval::Int(_) => Gval::Int(BigInt::zero()),
            Gval::Arr(_) => Gval::Arr(vec![]),
            Gval::Str(_) => Gval::Str(vec![]),
            Gval::Blk(_) => Gval::Blk(vec![]),
        }
    }

    pub fn push(&mut self, other: Gval) {
        match self {
            Gval::Int(_) => panic!("push"),
            Gval::Arr(vs) => vs.push(other),
            Gval::Str(vs) => flatten_append(vs, other),
            Gval::Blk(vs) => flatten_append(vs, other),
        }
    }

    pub fn unwrap_int(self) -> BigInt {
        match self {
            Gval::Int(n) => n,
            _ => panic!("expected int"),
        }
    }

    pub fn unwrap_arr(self) -> Vec<Gval> {
        match self {
            Gval::Arr(a) => a,
            _ => panic!("expected array"),
        }
    }

    pub fn as_arr(self) -> Vec<Gval> {
        match self {
            Gval::Int(_) => panic!("as_arr"),
            Gval::Arr(a) => a,
            Gval::Str(a) | Gval::Blk(a) => a.into_iter().map(|b| b.into()).collect(),
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
            r = coerce(r, sep.clone()).left();
            for i in a {
                r = r.plus(sep.clone()).plus(i);
            }
            r
        }
    }
}
