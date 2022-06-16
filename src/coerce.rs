use crate::value::Gval;
use num::BigInt;
use num::Integer;
use num::ToPrimitive;

#[derive(Debug)]
pub enum Coerced {
    Ints(BigInt, BigInt),
    Arrs(Vec<Gval>, Vec<Gval>),
    Strs(Vec<u8>, Vec<u8>),
    Blks(Vec<u8>, Vec<u8>),
}

impl Coerced {
    pub fn left(self) -> Gval {
        match self {
            Coerced::Ints(a, _) => Gval::Int(a),
            Coerced::Arrs(a, _) => Gval::Arr(a),
            Coerced::Strs(a, _) => Gval::Str(a),
            Coerced::Blks(a, _) => Gval::Blk(a),
        }
    }
}

pub fn flatten_append(bytes: &mut Vec<u8>, val: Gval) {
    match val {
        Gval::Int(a) => bytes.push(a.mod_floor(&256.into()).to_u8().unwrap()),
        Gval::Arr(vs) => {
            for v in vs {
                flatten_append(bytes, v);
            }
        }
        Gval::Str(bs) | Gval::Blk(bs) => bytes.extend(bs),
    }
}

pub fn flatten(arr: Vec<Gval>) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];
    flatten_append(&mut bytes, Gval::Arr(arr));
    bytes
}

fn show_words(arr: Vec<Gval>) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];
    for (i, v) in arr.into_iter().enumerate() {
        if i > 0 {
            bytes.push(' ' as u8)
        }
        bytes.extend(v.to_gs())
    }
    bytes
}

pub fn coerce(a: Gval, b: Gval) -> Coerced {
    use Gval::*;
    match (a, b) {
        // same type (or str + blk):
        (Int(a), Int(b)) => Coerced::Ints(a, b),
        (Arr(a), Arr(b)) => Coerced::Arrs(a, b),
        (Str(a), Str(b)) => Coerced::Strs(a, b),
        (Blk(a), Blk(b)) => Coerced::Blks(a, b),
        (Str(a), Blk(b)) => Coerced::Blks(a, b),
        (Blk(a), Str(b)) => Coerced::Blks(a, b),
        // int + arr: wrap the int
        (Int(a), Arr(b)) => Coerced::Arrs(vec![Int(a)], b),
        (Arr(a), Int(b)) => Coerced::Arrs(a, vec![Int(b)]),
        // int + str/blk: show the int
        (Int(a), Str(b)) => Coerced::Strs(a.to_str_radix(10).into_bytes(), b),
        (Str(a), Int(b)) => Coerced::Strs(a, b.to_str_radix(10).into_bytes()),
        (Int(a), Blk(b)) => Coerced::Blks(a.to_str_radix(10).into_bytes(), b),
        (Blk(a), Int(b)) => Coerced::Blks(a, b.to_str_radix(10).into_bytes()),
        // str + arr: flatten the arr
        (Arr(a), Str(b)) => Coerced::Strs(flatten(a), b),
        (Str(a), Arr(b)) => Coerced::Strs(a, flatten(b)),
        // arr + blk: show arr contents space-separated
        (Arr(a), Blk(b)) => Coerced::Blks(show_words(a), b),
        (Blk(a), Arr(b)) => Coerced::Blks(a, show_words(b)),
    }
}
