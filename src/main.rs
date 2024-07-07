use crate::coerce::flatten;
use crate::parse::parse_code;
use crate::util::chunk;
use crate::util::every_nth;
use crate::util::index;
use crate::util::slice;
use crate::util::split;
use crate::util::string_index;
use crate::value::join;
use clap::Parser;
use num::BigInt;
use num::Integer;
use num::One;
use num::Signed;
use num::ToPrimitive;
use num::Zero;
use std::cmp::Ordering;
use std::io::Read;
use std::io::Write;

use std::collections::HashMap;

mod coerce;
mod parse;
mod unescape;
mod util;
mod value;

use crate::coerce::{coerce, Coerced};
use crate::parse::Gtoken;
use crate::unescape::unescape;
use crate::util::{repeat, set_and, set_or, set_subtract, set_xor};
use crate::value::Gval;

fn print(bytes: &[u8]) {
    std::io::stdout().write_all(bytes).unwrap();
}

struct Gs {
    pub stack: Vec<Gval>,
    vars: HashMap<Vec<u8>, Gval>,
    lb: Vec<usize>,
    rng_state: u64,
}

impl Gs {
    pub fn new() -> Gs {
        Gs {
            stack: vec![],
            vars: HashMap::new(),
            lb: vec![],
            rng_state: 123456789u64,
        }
    }

    pub fn run(&mut self, code: &[u8]) {
        let (rest, tokens) = parse_code(code).expect("parse error");
        if !rest.is_empty() {
            panic!("parse error: has remainder")
        }
        // println!("parse: {:?}", tokens);
        let mut tokens = tokens.into_iter();
        while let Some(token) = tokens.next() {
            match token {
                Gtoken::Symbol(b":") => {
                    let name = tokens.next().expect("parse error: assignment");
                    let t = self.top().clone();
                    self.vars.insert(name.lexeme().to_owned(), t);
                }
                t => {
                    self.run_token(t);
                }
            }
        }
    }

    fn push(&mut self, val: Gval) {
        self.stack.push(val)
    }

    fn top(&self) -> &Gval {
        self.stack.last().expect("stack underflow")
    }

    fn dup(&mut self) {
        let a = self.pop();
        self.push(a.clone());
        self.push(a);
    }

    fn pop(&mut self) -> Gval {
        let mut i = self.lb.len();
        while i > 0 && self.lb[i - 1] >= self.stack.len() {
            i -= 1;
            if self.lb[i] > 0 {
                self.lb[i] -= 1;
            }
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
        self.push(Gval::Str(bs));
    }

    fn bang(&mut self) {
        let f = self.pop().falsey();
        self.push(Gval::bool(f));
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
            Coerced::Ints(x, y) => self.push(Gval::Int(x - y)),
            Coerced::Arrs(x, y) => self.push(Gval::Arr(set_subtract(x, y))),
            Coerced::Strs(x, y) => self.push(Gval::Str(set_subtract(x, y))),
            Coerced::Blks(x, y) => self.push(Gval::Blk(set_subtract(x, y))),
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
            (Arr(a), Str(sep)) | (Str(sep), Arr(a)) => self.push(join(a, Str(sep))),
            (Str(a), Str(sep)) => {
                let a: Vec<Gval> = a.into_iter().map(|x| Gval::Str(vec![x])).collect();
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
                    n -= 1;
                }
            }
        }
    }

    fn slash(&mut self) {
        let b = self.pop();
        let a = self.pop();
        use Gval::*;
        match (a, b) {
            // divide
            (Int(a), Int(b)) => self.push(Int(a.div_floor(&b))),
            // split
            (Arr(a), Arr(sep)) => {
                let s = split(a, sep, false);
                self.push(Arr(s.into_iter().map(Arr).collect()));
            }
            (Str(a), Str(sep)) => {
                let s = split(a, sep, false);
                self.push(Arr(s.into_iter().map(Str).collect()));
            }
            (Arr(a), Str(sep)) | (Str(sep), Arr(a)) => {
                let s = split(a, sep.into_iter().map(|x| x.into()).collect(), false);
                self.push(Arr(s.into_iter().map(Arr).collect()));
            }

            // each
            (Str(a), Blk(code)) | (Blk(code), Str(a)) => self.each(code, a),
            (Arr(a), Blk(code)) | (Blk(code), Arr(a)) => self.each(code, a),

            // chunk
            (Int(n), Arr(mut a)) | (Arr(mut a), Int(n)) => {
                let c = chunk(&mut a, n);
                self.push(Arr(c.into_iter().map(|x| Arr(x.to_owned())).collect()));
            }
            (Int(n), Str(mut a)) | (Str(mut a), Int(n)) => {
                let c = chunk(&mut a, n);
                self.push(Arr(c.into_iter().map(|x| Str(x.to_owned())).collect()));
            }

            // unfold
            (Blk(cond), Blk(step)) => {
                let mut r = vec![];
                loop {
                    self.push(self.top().clone());
                    self.run(&cond);
                    if self.pop().falsey() {
                        break;
                    }
                    r.push(self.top().clone());
                    self.run(&step);
                }
                self.pop();
                self.push(Gval::Arr(r));
            }

            (Blk(_), Int(_)) | (Int(_), Blk(_)) => {
                panic!("int-block /")
            }
        }
    }

    fn percent(&mut self) {
        let b = self.pop();
        let a = self.pop();
        use Gval::*;
        match (a, b) {
            // modulo
            (Int(a), Int(b)) => self.push(Int(a.mod_floor(&b))),
            // clean split
            (Arr(a), Arr(sep)) => {
                let s = split(a, sep, true);
                self.push(Arr(s.into_iter().map(Arr).collect()));
            }
            (Str(a), Str(sep)) => {
                let s = split(a, sep, true);
                self.push(Arr(s.into_iter().map(Str).collect()));
            }
            (Arr(a), Str(sep)) | (Str(sep), Arr(a)) => {
                let s = split(a, sep.into_iter().map(|x| x.into()).collect(), true);
                self.push(Arr(s.into_iter().map(Arr).collect()));
            }

            // map
            (Arr(a), Blk(code)) | (Blk(code), Arr(a)) => {
                let r = self.gs_map(code, a);
                self.push(Arr(r))
            }
            (Str(a), Blk(code)) | (Blk(code), Str(a)) => {
                let r = self.gs_map(code, a);
                self.push(Str(flatten(r)))
            }

            // every nth
            (Int(n), Arr(a)) | (Arr(a), Int(n)) => self.push(Arr(every_nth(a, n))),
            (Int(n), Str(a)) | (Str(a), Int(n)) => self.push(Str(every_nth(a, n))),

            // unimplemented
            (Int(_), Blk(_)) | (Blk(_), Int(_)) | (Blk(_), Blk(_)) => panic!("%"),
        }
    }

    fn vertical_bar(&mut self) {
        let b = self.pop();
        let a = self.pop();
        self.push(match coerce(a, b) {
            Coerced::Ints(x, y) => Gval::Int(x | y),
            Coerced::Arrs(x, y) => Gval::Arr(set_or(x, y)),
            Coerced::Strs(x, y) => Gval::Str(set_or(x, y)),
            Coerced::Blks(x, y) => Gval::Blk(set_or(x, y)),
        })
    }

    fn ampersand(&mut self) {
        let b = self.pop();
        let a = self.pop();
        self.push(match coerce(a, b) {
            Coerced::Ints(x, y) => Gval::Int(x & y),
            Coerced::Arrs(x, y) => Gval::Arr(set_and(x, y)),
            Coerced::Strs(x, y) => Gval::Str(set_and(x, y)),
            Coerced::Blks(x, y) => Gval::Blk(set_and(x, y)),
        })
    }

    fn caret(&mut self) {
        let b = self.pop();
        let a = self.pop();
        self.push(match coerce(a, b) {
            Coerced::Ints(x, y) => Gval::Int(x ^ y),
            Coerced::Arrs(x, y) => Gval::Arr(set_xor(x, y)),
            Coerced::Strs(x, y) => Gval::Str(set_xor(x, y)),
            Coerced::Blks(x, y) => Gval::Blk(set_xor(x, y)),
        })
    }

    fn lteqgt(&mut self, ordering: Ordering) {
        let b = self.pop();
        let a = self.pop();
        use Gval::*;
        use Ordering::*;
        match (ordering, a, b) {
            (Equal, Int(i), Arr(a)) | (Equal, Arr(a), Int(i)) => {
                if let Some(x) = index(&a, i) {
                    self.push(x.clone())
                }
            }
            (Equal, Int(i), Str(a))
            | (Equal, Str(a), Int(i))
            | (Equal, Int(i), Blk(a))
            | (Equal, Blk(a), Int(i)) => {
                if let Some(x) = index(&a, i) {
                    self.push((*x).into())
                }
            }
            (o, Int(i), Arr(a)) | (o, Arr(a), Int(i)) => self.push(Arr(slice(o, a, i))),
            (o, Int(i), Str(a)) | (o, Str(a), Int(i)) => self.push(Str(slice(o, a, i))),
            (o, Int(i), Blk(a)) | (o, Blk(a), Int(i)) => self.push(Blk(slice(o, a, i))),
            (o, x, y) => self.push(Gval::bool(x.cmp(&y) == o)),
        }
    }

    fn comma(&mut self) {
        use Gval::*;
        match self.pop() {
            Int(n) => {
                let mut r = vec![];
                let mut i = BigInt::zero();
                while i < n {
                    r.push(Int(i.clone()));
                    i += 1i32;
                }
                self.push(Arr(r));
            }
            Arr(a) => self.push(a.len().into()),
            Str(a) => self.push(a.len().into()),
            Blk(code) => match self.pop() {
                Int(_) => panic!("select on integer"),
                Arr(a) => {
                    let r = self.select(code, a);
                    self.push(Arr(r))
                }
                Str(a) => {
                    let r = self.select(code, a);
                    self.push(Str(r))
                }
                Blk(a) => {
                    let r = self.select(code, a);
                    self.push(Blk(r))
                }
            },
        }
    }

    fn question(&mut self) {
        let b = self.pop();
        let a = self.pop();
        use Gval::*;
        match (a, b) {
            // power
            (Int(a), Int(b)) => self.push(Int(match b.to_u32() {
                Some(e) => a.pow(e),
                None => BigInt::zero(),
            })),

            // indexof
            (Arr(h), n @ Int(_))
            | (n @ Int(_), Arr(h))
            | (Arr(h), n @ Str(_))
            | (n @ Str(_), Arr(h))
            | (Arr(h), n @ Arr(_)) => self.push(Gval::Int(
                h.iter()
                    .position(|x| *x == n)
                    .map_or(-BigInt::one(), BigInt::from),
            )),
            (Str(h), Int(n)) | (Int(n), Str(h)) => self.push(Gval::Int(match n.to_u8() {
                None => -BigInt::one(),
                Some(b) => h
                    .iter()
                    .position(|x| *x == b)
                    .map_or(-BigInt::one(), BigInt::from),
            })),
            (Str(h), Str(n)) => self.push(Gval::Int(string_index(&h, &n))),

            // find
            (Int(_), Blk(_)) | (Blk(_), Int(_)) => panic!(),
            (Blk(code), Blk(a)) | (Blk(code), Str(a)) | (Str(a), Blk(code)) => self.find(code, a),
            (Blk(code), Arr(a)) | (Arr(a), Blk(code)) => self.find(code, a),
        }
    }

    fn left_paren(&mut self) {
        use Gval::*;
        match self.pop() {
            Int(n) => self.push(Int(n - 1i32)),
            Arr(a) => {
                self.push(Arr(a[1..].to_vec()));
                self.push(a[0].clone());
            }
            Str(a) => {
                self.push(Str(a[1..].to_vec()));
                self.push(a[0].into());
            }
            Blk(a) => {
                self.push(Blk(a[1..].to_vec()));
                self.push(a[0].into());
            }
        }
    }

    fn right_paren(&mut self) {
        use Gval::*;
        match self.pop() {
            Int(n) => self.push(Int(n + 1i32)),
            Arr(mut a) => {
                let l = a.pop().unwrap();
                self.push(Arr(a.to_vec()));
                self.push(l);
            }
            Str(mut a) => {
                let l = a.pop().unwrap();
                self.push(Str(a.to_vec()));
                self.push(l.into());
            }
            Blk(mut a) => {
                let l = a.pop().unwrap();
                self.push(Blk(a.to_vec()));
                self.push(l.into());
            }
        }
    }

    fn rng(&mut self) -> u64 {
        let (m, _) = self.rng_state.overflowing_mul(1664525);
        let (m, _) = m.overflowing_add(1013904223);
        self.rng_state = m;
        self.rng_state
    }

    fn rand(&mut self) {
        let r = match self.pop() {
            Gval::Int(n) if n.is_positive() => self.rng() % n,
            _ => BigInt::zero(),
        };
        self.push(Gval::Int(r));
    }

    fn do_loop(&mut self) {
        let a = self.pop();
        loop {
            self.go(a.clone());
            if self.pop().falsey() {
                break;
            }
        }
    }

    fn while_loop(&mut self, which: bool) {
        let b = self.pop();
        let a = self.pop();
        loop {
            self.go(a.clone());
            if self.pop().falsey() == which {
                break;
            }
            self.go(b.clone());
        }
    }

    fn zip(&mut self) {
        let a = self.pop().unwrap_arr();
        let mut r = vec![];
        let blank = a.first().map_or(Gval::Arr(vec![]), |x| x.factory());
        for row in a {
            for (y, elem) in row.into_arr().into_iter().enumerate() {
                while r.len() < y + 1 {
                    r.push(blank.clone())
                }
                r[y].push(elem.clone());
            }
        }
        self.push(Gval::Arr(r))
    }

    fn base(&mut self) {
        let b = self.pop().unwrap_int();
        match self.pop() {
            Gval::Int(n) => {
                let mut digits = vec![];
                let mut i = n.abs();
                while !i.is_zero() {
                    let (j, k) = i.div_mod_floor(&b);
                    i = j;
                    digits.push(Gval::Int(k));
                }
                digits.reverse();
                self.push(Gval::Arr(digits))
            }
            n => {
                let mut total = BigInt::zero();
                for digit in n.into_arr() {
                    total = total * b.clone() + digit.unwrap_int();
                }
                self.push(Gval::Int(total))
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

    fn each<T: Into<Gval>>(&mut self, code: Vec<u8>, vs: Vec<T>) {
        for v in vs {
            self.push(v.into());
            self.run(&code);
        }
    }

    fn gs_map<T: Into<Gval>>(&mut self, code: Vec<u8>, vs: Vec<T>) -> Vec<Gval> {
        let mut r: Vec<Gval> = vec![];
        for v in vs {
            let lb = self.stack.len();
            self.push(v.into());
            self.run(&code);
            r.extend(self.stack.drain(lb..));
        }
        r
    }

    fn select<T: Clone + Into<Gval>>(&mut self, code: Vec<u8>, vs: Vec<T>) -> Vec<T> {
        let mut r: Vec<T> = vec![];
        for v in vs {
            self.push(v.clone().into());
            self.run(&code);
            if self.pop().truthy() {
                r.push(v)
            }
        }
        r
    }

    fn find<T: Clone + Into<Gval>>(&mut self, code: Vec<u8>, vs: Vec<T>) {
        for v in vs {
            self.push(v.clone().into());
            self.run(&code);
            if self.pop().truthy() {
                self.push(v.into());
                break;
            }
        }
    }

    fn go(&mut self, val: Gval) {
        match val {
            Gval::Blk(s) => self.run(&s),
            _ => self.push(val),
        }
    }

    fn run_token(&mut self, token: Gtoken) {
        if let Some(v) = self.vars.get(token.lexeme()).cloned() {
            self.go(v);
            return;
        }
        match token {
            Gtoken::IntLiteral(bs) => {
                let n = BigInt::parse_bytes(bs, 10).unwrap();
                self.push(Gval::Int(n));
            }
            Gtoken::SingleQuotedString(bs) => self.push(Gval::Str(unescape(bs, true))),
            Gtoken::DoubleQuotedString(bs) => self.push(Gval::Str(unescape(bs, false))),
            Gtoken::Symbol(b"~") => self.tilde(),
            Gtoken::Symbol(b"`") => self.backtick(),
            Gtoken::Symbol(b"!") => self.bang(),
            Gtoken::Symbol(b"@") => self.at_sign(),
            Gtoken::Symbol(b"$") => self.dollar(),
            Gtoken::Symbol(b"+") => self.plus(),
            Gtoken::Symbol(b"-") => self.minus(),
            Gtoken::Symbol(b"*") => self.asterisk(),
            Gtoken::Symbol(b"/") => self.slash(),
            Gtoken::Symbol(b"%") => self.percent(),
            Gtoken::Symbol(b"|") => self.vertical_bar(),
            Gtoken::Symbol(b"&") => self.ampersand(),
            Gtoken::Symbol(b"^") => self.caret(),
            Gtoken::Symbol(b"[") => self.lb.push(self.stack.len()),
            Gtoken::Symbol(b"]") => {
                let vs = self.stack.drain(self.lb.pop().unwrap_or(0)..).collect();
                self.push(Gval::Arr(vs));
            }
            Gtoken::Symbol(b"\\") => {
                let b = self.pop();
                let a = self.pop();
                self.push(b);
                self.push(a);
            }
            Gtoken::Symbol(b";") => {
                let _ = self.pop();
            }
            Gtoken::Symbol(b"<") => self.lteqgt(Ordering::Less),
            Gtoken::Symbol(b"=") => self.lteqgt(Ordering::Equal),
            Gtoken::Symbol(b">") => self.lteqgt(Ordering::Greater),
            Gtoken::Symbol(b",") => self.comma(),
            Gtoken::Symbol(b".") => self.dup(),
            Gtoken::Symbol(b"?") => self.question(),
            Gtoken::Symbol(b"(") => self.left_paren(),
            Gtoken::Symbol(b")") => self.right_paren(),
            Gtoken::Symbol(b"and") => {
                let b = self.pop();
                let a = self.pop();
                self.go(if a.truthy() { b } else { a });
            }
            Gtoken::Symbol(b"or") => {
                let b = self.pop();
                let a = self.pop();
                self.go(if a.falsey() { b } else { a });
            }
            Gtoken::Symbol(b"xor") => {
                let b = self.pop();
                let a = self.pop();
                self.push(Gval::bool(a.truthy() ^ b.truthy()));
            }
            Gtoken::Symbol(b"n") => self.push(Gval::Str(b"\n".to_vec())),
            Gtoken::Symbol(b"print") => {
                let a = self.pop();
                print(&a.into_gs());
            }
            Gtoken::Symbol(b"p") => {
                let a = self.pop();
                print(&a.inspect());
                print(b"\n");
            }
            Gtoken::Symbol(b"puts") => {
                let a = self.pop();
                print(&a.into_gs());
                print(b"\n");
            }
            Gtoken::Symbol(b"rand") => self.rand(),
            Gtoken::Symbol(b"do") => self.do_loop(),
            Gtoken::Symbol(b"while") => self.while_loop(true),
            Gtoken::Symbol(b"until") => self.while_loop(false),
            Gtoken::Symbol(b"if") => {
                let c = self.pop();
                let b = self.pop();
                let a = self.pop();
                if a.truthy() {
                    self.go(b);
                } else {
                    self.go(c);
                }
            }
            Gtoken::Symbol(b"abs") => {
                let a = self.pop();
                self.push(Gval::Int(a.unwrap_int().abs()));
            }
            Gtoken::Symbol(b"zip") => self.zip(),
            Gtoken::Symbol(b"base") => self.base(),
            Gtoken::Block(_, src) => self.push(Gval::Blk(src.to_owned())),
            Gtoken::Symbol(_) => {}
            Gtoken::Comment(_) => {}
        }
    }
}

#[derive(clap::Parser, Debug)]
struct Cli {
    #[clap(long)]
    code_path: Option<String>,
    #[clap(short = 'e', long, allow_hyphen_values = true)]
    code: Option<String>,
    #[clap(long)]
    input_path: Option<String>,
    #[clap(short = 'i', long, allow_hyphen_values = true)]
    input: Option<String>,
    #[clap(short = 'q', long, takes_value = false)]
    no_implicit_output: bool,
    #[clap(short = 's', long, takes_value = false)]
    input_from_stdin: bool,
    #[clap(long, takes_value = false)]
    args: bool,
    args_vec: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let mut gs = Gs::new();
    let input = if cli.args {
        Gval::Arr(
            cli.args_vec
                .iter()
                .map(|x| Gval::Str(x.as_bytes().to_vec()))
                .collect(),
        )
    } else if cli.input_from_stdin {
        let mut bytes = vec![];
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        Gval::Str(bytes)
    } else if let Some(path) = cli.input_path {
        Gval::Str(std::fs::read(path).unwrap())
    } else if let Some(string) = cli.input {
        Gval::Str(string.as_bytes().to_vec())
    } else {
        Gval::Str(vec![])
    };
    let code = if let Some(path) = cli.code_path {
        std::fs::read(path).unwrap()
    } else if let Some(code) = cli.code {
        code.as_bytes().to_vec()
    } else {
        eprintln!(
            r"No code provided. Try:

    golfscript-rs --help
    golfscript-rs --code '~{{.@\%.}}do;'   --input '140 150'
    golfscript-rs --code 'n*~{{.@\%.}}do;' --args 140 150   # code.golf style
    golfscript-rs --code-path file.gs    --input-file input.txt
    golfscript-rs --code-path file.gs    --input-from-stdin
"
        );
        std::process::exit(1)
    };
    gs.stack.push(input);
    gs.run(&code);
    if !cli.no_implicit_output {
        gs.stack = vec![Gval::Arr(gs.stack)];
        gs.run(b"puts");
    }
}
