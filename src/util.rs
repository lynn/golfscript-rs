use core::cmp::Ordering;
use core::hash::Hash;
use num::BigInt;
use num::Integer;
use num::One;
use num::Signed;
use num::ToPrimitive;
use num::Zero;
use std::collections::HashSet;

pub fn to_byte(n: BigInt) -> u8 {
    n.mod_floor(&256.into()).to_u8().unwrap()
}

pub fn repeat<T: Clone>(a: Vec<T>, mut n: BigInt) -> Vec<T> {
    let mut v = vec![];
    while n.is_positive() {
        v.extend(a.clone());
        n -= 1;
    }
    v
}

pub fn chunk<'a, T: Clone>(a: &'a mut Vec<T>, n: BigInt) -> Vec<&'a [T]> {
    if a.len() == 0 {
        return vec![];
    }
    if n.is_zero() {
        panic!("chunk division by 0");
    }
    if n.is_negative() {
        a.reverse();
    }
    a.chunks(n.abs().to_usize().unwrap()).collect()
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

pub fn every_nth<T>(a: Vec<T>, n: BigInt) -> Vec<T> {
    let m = n.abs().to_usize().unwrap();
    if n.is_negative() {
        a.into_iter().rev().step_by(m).collect()
    } else {
        a.into_iter().step_by(m).collect()
    }
}

pub fn set_subtract<T: Eq>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    a.into_iter().filter(|x| !b.contains(&x)).collect()
}

pub fn set_or<T: Clone + Eq + Hash>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    let mut seen: HashSet<T> = HashSet::new();
    let mut result: Vec<T> = vec![];
    for v in a.into_iter().chain(b.into_iter()) {
        if seen.insert(v.clone()) {
            result.push(v)
        }
    }
    result
}

pub fn set_and<T: Clone + Eq + Hash>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    let mut in_a: HashSet<T> = HashSet::new();
    let mut result: Vec<T> = vec![];
    for v in a {
        in_a.insert(v);
    }
    let mut seen: HashSet<T> = HashSet::new();
    for v in b {
        if in_a.contains(&v) && seen.insert(v.clone()) {
            result.push(v)
        }
    }
    result
}

pub fn set_xor<T: Clone + Eq + Hash>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    let mut in_a: HashSet<T> = HashSet::new();
    let mut in_b: HashSet<T> = HashSet::new();
    let mut seen: HashSet<T> = HashSet::new();
    let mut result: Vec<T> = vec![];
    for v in &a {
        in_a.insert(v.clone());
    }
    for v in &b {
        in_b.insert(v.clone());
    }
    for v in a.into_iter().chain(b.into_iter()) {
        if !seen.contains(&v) && (in_a.contains(&v) ^ in_b.contains(&v)) {
            seen.insert(v.clone());
            result.push(v)
        }
    }
    result
}

pub fn index<T>(a: &Vec<T>, i: BigInt) -> Option<&T> {
    let l: BigInt = a.len().into();
    if i >= l {
        None
    } else if i >= BigInt::zero() && i < l {
        Some(&a[i.to_usize().unwrap()])
    } else if i >= -l.clone() {
        Some(&a[(i + l).to_usize().unwrap()])
    } else {
        None
    }
}

pub fn slice<T: Clone>(o: Ordering, a: Vec<T>, i: BigInt) -> Vec<T> {
    let l = a.len();
    let lb: BigInt = a.len().into();
    let ix = if i >= lb {
        l
    } else if i >= BigInt::zero() {
        i.to_usize().unwrap()
    } else if i >= -lb.clone() {
        (i + l).to_usize().unwrap()
    } else {
        0
    };
    match o {
        Ordering::Less => a[0..ix].to_vec(),
        Ordering::Greater => a[ix..].to_vec(),
        _ => panic!(),
    }
}

pub fn string_index(haystack: &[u8], needle: &[u8]) -> BigInt {
    let hl = haystack.len();
    let nl = needle.len();
    if nl <= hl {
        for i in 0..=hl - nl {
            if &haystack[i..i + nl] == needle {
                return i.into();
            }
        }
    }
    return -BigInt::one();
}
