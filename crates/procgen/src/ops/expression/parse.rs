//! Precedence, loosest first: comparisons, `+ -`, `* /`, unary minus, `^`. Left-associative but `^`, which binds right (`2 ^ 3 ^ 2` is `2 ^ 9`) and tighter than unary minus (`-a ^ 2` is `-(a ^ 2)`).

use super::fault::show;
use super::fault::{args_hint, func_names, unexpected, unknown_name};
use super::lex::{tokens, Tok};
use super::{Func, Instr, Load, Program, SyntaxError, FUNCS, INPUTS, MAX_DEPTH};

pub fn compile(src: &str) -> Result<Program, SyntaxError> {
    let toks = tokens(src)?;
    if toks.len() == 1 {
        return Err(SyntaxError {
            what: "the formula is empty".into(),
            column: None,
            fix: "write one, such as `u * v`".into(),
        });
    }
    let mut p = Parser {
        toks,
        pos: 0,
        code: Vec::new(),
        inputs: [false; 4],
    };
    p.comparison(0)?;
    let (tok, column) = p.peek();
    if tok != Tok::End {
        return Err(unexpected(tok, column, "an operator or the end"));
    }
    let stack = stack_depth(&p.code);
    Ok(Program {
        code: p.code,
        stack,
        inputs: p.inputs,
    })
}

fn stack_depth(code: &[Instr]) -> usize {
    let (mut now, mut most) = (0usize, 0usize);
    for i in code {
        match i {
            Instr::Load(_) => now += 1,
            Instr::Call(f) => now = now + 1 - f.arity(),
        }
        most = most.max(now);
    }
    most
}

struct Parser<'a> {
    toks: Vec<(Tok<'a>, usize)>,
    pos: usize,
    code: Vec<Instr>,
    inputs: [bool; 4],
}

impl<'a> Parser<'a> {
    fn peek(&self) -> (Tok<'a>, usize) {
        self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn next(&mut self) -> (Tok<'a>, usize) {
        let t = self.peek();
        self.pos += 1;
        t
    }

    fn eat(&mut self, sym: &str) -> bool {
        let hit = matches!(self.peek().0, Tok::Sym(s) if s == sym);
        self.pos += usize::from(hit);
        hit
    }

    fn eat_op(&mut self, ops: &[(&str, Func)]) -> Option<Func> {
        let Tok::Sym(s) = self.peek().0 else {
            return None;
        };
        let &(_, f) = ops.iter().find(|(o, _)| *o == s)?;
        self.pos += 1;
        Some(f)
    }

    fn emit(&mut self, f: Func) {
        self.code.push(Instr::Call(f));
    }

    fn deeper(&self, depth: usize) -> Result<usize, SyntaxError> {
        if depth < MAX_DEPTH {
            return Ok(depth + 1);
        }
        Err(SyntaxError {
            what: format!("the formula nests deeper than {MAX_DEPTH} levels"),
            column: Some(self.peek().1),
            fix: "split it across nodes".into(),
        })
    }

    fn comparison(&mut self, depth: usize) -> Result<(), SyntaxError> {
        const OPS: &[(&str, Func)] = &[
            ("<", Func::Lt),
            ("<=", Func::Le),
            (">", Func::Gt),
            (">=", Func::Ge),
            ("==", Func::Eq),
            ("!=", Func::Ne),
        ];
        self.sum(depth)?;
        while let Some(f) = self.eat_op(OPS) {
            self.sum(depth)?;
            self.emit(f);
        }
        Ok(())
    }

    fn sum(&mut self, depth: usize) -> Result<(), SyntaxError> {
        const OPS: &[(&str, Func)] = &[("+", Func::Add), ("-", Func::Sub)];
        self.product(depth)?;
        while let Some(f) = self.eat_op(OPS) {
            self.product(depth)?;
            self.emit(f);
        }
        Ok(())
    }

    fn product(&mut self, depth: usize) -> Result<(), SyntaxError> {
        const OPS: &[(&str, Func)] = &[("*", Func::Mul), ("/", Func::Div)];
        self.unary(depth)?;
        while let Some(f) = self.eat_op(OPS) {
            self.unary(depth)?;
            self.emit(f);
        }
        Ok(())
    }

    fn unary(&mut self, depth: usize) -> Result<(), SyntaxError> {
        if self.eat("-") {
            self.unary(self.deeper(depth)?)?;
            self.emit(Func::Neg);
            return Ok(());
        }
        if self.eat("+") {
            return self.unary(self.deeper(depth)?);
        }
        self.atom(depth)?;
        if self.eat("^") {
            self.unary(self.deeper(depth)?)?;
            self.emit(Func::Pow);
        }
        Ok(())
    }

    fn atom(&mut self, depth: usize) -> Result<(), SyntaxError> {
        match self.next() {
            (Tok::Num(n), _) => self.code.push(Instr::Load(Load::Num(n))),
            (Tok::Name(name), column) => self.name(name, column, depth)?,
            (Tok::Sym("("), _) => {
                self.comparison(self.deeper(depth)?)?;
                self.close(")")?;
            }
            (tok, column) => return Err(unexpected(tok, column, "a number, name or (")),
        }
        Ok(())
    }

    fn close(&mut self, sym: &str) -> Result<(), SyntaxError> {
        if self.eat(sym) {
            return Ok(());
        }
        let (tok, column) = self.peek();
        Err(unexpected(tok, column, &format!("{sym:?}")))
    }

    fn name(&mut self, name: &str, column: usize, depth: usize) -> Result<(), SyntaxError> {
        if let Some(&(_, f)) = FUNCS.iter().find(|(n, _)| *n == name) {
            return self.call(name, f, column, depth);
        }
        let load = match name {
            "u" => Load::U,
            "v" => Load::V,
            "pi" => Load::Num(std::f32::consts::PI),
            "tau" => Load::Num(std::f32::consts::TAU),
            "k1" | "k2" | "k3" | "k4" => Load::K(name.as_bytes()[1] - b'1'),
            _ => match INPUTS
                .iter()
                .position(|&c| name.len() == 1 && name.starts_with(c))
            {
                Some(slot) => self.input(slot)?,
                None if matches!(self.peek().0, Tok::Sym("(")) => {
                    return Err(SyntaxError {
                        what: format!("unknown function {name:?}"),
                        column: Some(column),
                        fix: format!("functions are {}", func_names()),
                    })
                }
                None => return Err(unknown_name(name, column)),
            },
        };
        if matches!(self.peek().0, Tok::Sym("(")) {
            return Err(SyntaxError {
                what: format!("{name:?} is not a function"),
                column: Some(column),
                fix: format!("functions are {}", func_names()),
            });
        }
        self.code.push(Instr::Load(load));
        Ok(())
    }

    fn input(&mut self, slot: usize) -> Result<Load, SyntaxError> {
        self.inputs[slot] = true;
        let mut channel = 0;
        if self.eat(".") {
            let (tok, at) = self.next();
            channel = match tok {
                Tok::Name("r") => 0,
                Tok::Name("g") => 1,
                Tok::Name("b") => 2,
                Tok::Name("a") => 3,
                _ => {
                    return Err(SyntaxError {
                        what: format!("input {} has no channel {}", INPUTS[slot], show(tok)),
                        column: Some(at),
                        fix: "channels are r, g, b, a".into(),
                    })
                }
            };
        }
        Ok(Load::Input {
            slot: slot as u8,
            channel,
        })
    }

    fn call(
        &mut self,
        name: &str,
        f: Func,
        column: usize,
        depth: usize,
    ) -> Result<(), SyntaxError> {
        if !self.eat("(") {
            return Err(SyntaxError {
                what: format!("function {name:?} is missing its arguments"),
                column: Some(column),
                fix: format!("call it as {name}({})", args_hint(f.arity())),
            });
        }
        let inner = self.deeper(depth)?;
        let mut given = 0;
        if !self.eat(")") {
            loop {
                self.comparison(inner)?;
                given += 1;
                if !self.eat(",") {
                    break;
                }
            }
            self.close(")")?;
        }
        if given != f.arity() {
            return Err(SyntaxError {
                what: format!(
                    "function {name:?} takes {} argument{}, but {given} given",
                    f.arity(),
                    if f.arity() == 1 { "" } else { "s" }
                ),
                column: Some(column),
                fix: format!("call it as {name}({})", args_hint(f.arity())),
            });
        }
        self.emit(f);
        Ok(())
    }
}
