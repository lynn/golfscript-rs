use clap::Parser;
use nom::{
    bytes::complete::{tag, take_while_m_n},
    combinator::map_res,
    sequence::tuple,
    IResult,
};
use num::bigint::Sign;
use num::BigInt;
use num::FromPrimitive;
use num::Integer;
use num::One;
use num::Signed;
use num::ToPrimitive;
use num::Zero;

use std::collections::HashMap;
use std::str;

mod parse;
use parse::Gtoken;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Gval {
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

fn join(a: Vec<Gval>, sep: Gval) -> Gval {
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

fn repeat<T: Clone>(a: Vec<T>, mut n: BigInt) -> Vec<T> {
    let mut v = vec![];
    while n.is_positive() {
        v.extend(a.clone());
        n -= 1;
    }
    v
}

fn set_subtract<T: Eq>(a: Vec<T>, b: Vec<T>) -> Vec<T> {
    a.into_iter().filter(|x| !b.contains(&x)).collect()
}

impl Gval {
    fn falsey(self) -> bool {
        match self {
            Gval::Int(a) => a == BigInt::zero(),
            Gval::Arr(vs) => vs.len() == 0,
            Gval::Str(bs) | Gval::Blk(bs) => bs.len() == 0,
        }
    }
    fn to_gs(self) -> Vec<u8> {
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

    fn inspect(self) -> Vec<u8> {
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

    fn plus(self, other: Gval) -> Gval {
        match coerce(self, other) {
            Gco::Ints(x, y) => Gval::Int(x + y),
            Gco::Arrs(mut x, y) => {
                x.extend(y);
                Gval::Arr(x)
            }
            Gco::Strs(mut x, y) => {
                x.extend(y);
                Gval::Str(x)
            }
            Gco::Blks(x, y) => {
                let mut joined = x.clone();
                joined.push(b' ');
                joined.extend(y);
                Gval::Blk(joined)
            }
        }
    }
}

#[derive(Debug)]
enum Gco {
    Ints(BigInt, BigInt),
    Arrs(Vec<Gval>, Vec<Gval>),
    Strs(Vec<u8>, Vec<u8>),
    Blks(Vec<u8>, Vec<u8>),
}

fn flatten_append(bytes: &mut Vec<u8>, val: Gval) {
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

fn flatten(arr: Vec<Gval>) -> Vec<u8> {
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

fn coerce(a: Gval, b: Gval) -> Gco {
    use Gval::*;
    match (a, b) {
        // same type (or str + blk):
        (Int(a), Int(b)) => Gco::Ints(a, b),
        (Arr(a), Arr(b)) => Gco::Arrs(a, b),
        (Str(a), Str(b)) => Gco::Strs(a, b),
        (Blk(a), Blk(b)) => Gco::Blks(a, b),
        (Str(a), Blk(b)) => Gco::Blks(a, b),
        (Blk(a), Str(b)) => Gco::Blks(a, b),
        // int + arr: wrap the int
        (Int(a), Arr(b)) => Gco::Arrs(vec![Int(a)], b),
        (Arr(a), Int(b)) => Gco::Arrs(a, vec![Int(b)]),
        // int + str/blk: show the int
        (Int(a), Str(b)) => Gco::Strs(a.to_str_radix(10).into_bytes(), b),
        (Str(a), Int(b)) => Gco::Strs(a, b.to_str_radix(10).into_bytes()),
        (Int(a), Blk(b)) => Gco::Blks(a.to_str_radix(10).into_bytes(), b),
        (Blk(a), Int(b)) => Gco::Blks(a, b.to_str_radix(10).into_bytes()),
        // str + arr: flatten the arr
        (Arr(a), Str(b)) => Gco::Strs(flatten(a), b),
        (Str(a), Arr(b)) => Gco::Strs(a, flatten(b)),
        // arr + blk: show arr contents space-separated
        (Arr(a), Blk(b)) => Gco::Blks(show_words(a), b),
        (Blk(a), Arr(b)) => Gco::Blks(a, show_words(b)),
    }
}

struct Gs {
    pub stack: Vec<Gval>,
    vars: HashMap<Vec<u8>, Gval>,
    lb: Vec<usize>,
}

impl Gs {
    pub fn new() -> Gs {
        Gs {
            stack: vec![],
            vars: HashMap::new(),
            lb: vec![],
        }
    }

    pub fn run(&mut self, code: &[u8]) {
        let (rest, tokens) = parse::parse_code(code).expect("parse error");
        if rest != [] {
            panic!("parse error")
        }
        println!("parse: {:?}", tokens);
        let mut tokens = tokens.into_iter();
        while let Some(token) = tokens.next() {
            match token {
                Gtoken::Symbol(b":") => {
                    let name = tokens.next().expect("parse error: assignment");
                    let t = self.top().clone();
                    self.vars.insert(name.lexeme().to_owned(), t);
                }
                t => {
                    self.run_builtin(t);
                }
            }
        }
    }

    fn push(&mut self, val: Gval) {
        self.stack.push(val)
    }

    fn top(&mut self) -> &Gval {
        self.stack.last().expect("stack underflow")
    }

    fn pop(&mut self) -> Gval {
        let mut i = self.lb.len();
        while i > 0 && self.lb[i - 1] < self.stack.len() {
            i -= 1;
            self.lb[i] -= 1;
        }
        self.stack.pop().expect("stack underflow")
    }

    fn tilde(&mut self) {
        match self.pop() {
            Gval::Int(n) => self.push(Gval::Int(!n)),
            Gval::Arr(vs) => self.stack.extend(vs),
            Gval::Str(bs) => self.run(&bs),
            Gval::Blk(bs) => self.run(&bs),
        }
    }

    fn backtick(&mut self) {
        let bs = self.pop().inspect();
        self.push(Gval::Str(bs))
    }

    fn bang(&mut self) {
        let f = self.pop().falsey();
        self.push(Gval::Int(if f { BigInt::one() } else { BigInt::zero() }));
    }

    fn at_sign(&mut self) {
        let c = self.pop();
        let b = self.pop();
        let a = self.pop();
        self.push(b);
        self.push(c);
        self.push(a);
    }

    fn dollar(&mut self) {
        match self.pop() {
            Gval::Int(n) => {
                let len: BigInt = self.stack.len().into();
                if n < (-1i32).into() {
                    if let Some(i) = (-n - 2i32).to_usize() {
                        if i < self.stack.len() {
                            self.push(self.stack[i].clone());
                        }
                    }
                } else if n >= 0i32.into() && n < len {
                    if let Some(i) = (len - 1i32 - n).to_usize() {
                        self.push(self.stack[i].clone());
                    }
                }
            }
            Gval::Arr(mut vs) => {
                vs.sort();
                self.push(Gval::Arr(vs));
            }
            Gval::Str(mut bs) => {
                bs.sort();
                self.push(Gval::Str(bs));
            }
            Gval::Blk(code) => match self.pop() {
                Gval::Int(_) => panic!("can't sort an integer"),
                Gval::Arr(vs) => {
                    let sorted = self.sort_by(code, vs);
                    self.push(Gval::Arr(sorted));
                }
                Gval::Str(vs) => {
                    let sorted = self.sort_by(code, vs);
                    self.push(Gval::Str(sorted));
                }
                Gval::Blk(vs) => {
                    let sorted = self.sort_by(code, vs);
                    self.push(Gval::Blk(sorted));
                }
            },
        }
    }

    fn sort_by<T: Ord + Clone + Into<Gval>>(&mut self, code: Vec<u8>, vs: Vec<T>) -> Vec<T> {
        let mut results: Vec<(Gval, T)> = vec![];
        for v in vs {
            self.push(v.clone().into());
            self.run(&code);
            results.push((self.pop(), v));
        }
        results.sort_by(|a, b| a.0.cmp(&b.0));
        results.into_iter().map(|x| x.1).collect()
    }

    fn plus(&mut self) {
        let b = self.pop();
        let a = self.pop();
        self.push(a.plus(b));
    }

    fn minus(&mut self) {
        let b = self.pop();
        let a = self.pop();
        match coerce(a, b) {
            Gco::Ints(x, y) => self.push(Gval::Int(x - y)),
            Gco::Arrs(x, y) => self.push(Gval::Arr(set_subtract(x, y))),
            Gco::Strs(x, y) => self.push(Gval::Str(set_subtract(x, y))),
            Gco::Blks(x, y) => self.push(Gval::Blk(set_subtract(x, y))),
        }
    }

    fn asterisk(&mut self) {
        let b = self.pop();
        let a = self.pop();
        use Gval::*;
        match (a, b) {
            // multiply
            (Int(a), Int(b)) => self.push(Int(a * b)),
            // join
            (Arr(a), Arr(sep)) => self.push(join(a, Arr(sep))),
            (Str(a), Str(sep)) => {
                let a: Vec<Gval> = a.into_iter().map(|x| Gval::Str(vec![x.into()])).collect();
                self.push(join(a, Str(sep)));
            }
            (Arr(a), Str(sep)) | (Str(sep), Arr(a)) => {
                self.push(join(a, Str(sep)));
            }

            // fold
            (Blk(code), Blk(a)) | (Str(a), Blk(code)) | (Blk(code), Str(a)) => self.fold(code, a),
            (Arr(a), Blk(code)) | (Blk(code), Arr(a)) => self.fold(code, a),

            // repeat
            (Int(n), Arr(a)) | (Arr(a), Int(n)) => self.push(Arr(repeat(a, n))),
            (Int(n), Str(a)) | (Str(a), Int(n)) => self.push(Str(repeat(a, n))),

            // times
            (Int(mut n), Blk(f)) | (Blk(f), Int(mut n)) => {
                while n.is_positive() {
                    self.run(&f);
                    n -= BigInt::one();
                }
            }
        }
    }

    fn fold<T: Into<Gval>>(&mut self, code: Vec<u8>, vs: Vec<T>) {
        for (i, v) in vs.into_iter().enumerate() {
            self.push(v.into());
            if i >= 1 {
                self.run(&code);
            }
        }
    }

    fn go(&mut self, val: Gval) {
        match val {
            Gval::Blk(s) => self.run(&s),
            _ => self.push(val),
        }
    }

    fn run_builtin(&mut self, token: Gtoken) {
        if matches!(token, Gtoken::Symbol(s)) {
            if let Some(v) = self.vars.get(token.lexeme()) {
                let w = v.clone();
                self.go(w);
            }
        }
        match token {
            Gtoken::IntLiteral(bs) => {
                let n = BigInt::parse_bytes(bs, 10).unwrap();
                self.push(Gval::Int(n));
            }
            Gtoken::SingleQuotedString(bs) | Gtoken::DoubleQuotedString(bs) => {
                // TODO: string escapes
                self.push(Gval::Str(bs[1..bs.len() - 1].to_owned()))
            }
            Gtoken::Symbol(b"~") => self.tilde(),
            Gtoken::Symbol(b"`") => self.backtick(),
            Gtoken::Symbol(b"!") => self.bang(),
            Gtoken::Symbol(b"@") => self.at_sign(),
            Gtoken::Symbol(b"$") => self.dollar(),
            Gtoken::Symbol(b"+") => self.plus(),
            Gtoken::Symbol(b"-") => self.minus(),
            Gtoken::Symbol(b"*") => self.asterisk(),
            Gtoken::Block(_, src) => self.push(Gval::Blk(src.to_owned())),
            Gtoken::Symbol(_) => {}
            t => todo!("builtin {}", str::from_utf8(t.lexeme()).unwrap()),
        }
    }
}

#[derive(clap::Parser, Debug)]
struct Cli {
    code: String,
}

fn main() {
    let p = Cli::parse();
    let mut gs = Gs::new();
    gs.run(p.code.as_bytes());
    for g in gs.stack {
        print!("{} ", str::from_utf8(&g.inspect()).unwrap());
    }
    println!();
}
