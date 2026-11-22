use super::super::field::Field;
use super::super::math::{apply as math, clamp_value};
use super::{Func, Instr, Load, Program};
use crate::image_buf::{Image, Size};
use crate::recipe::MathOp;

pub struct Env<'a> {
    pub inputs: &'a [&'a Image],
    pub k: [Field<'a>; 4],
    pub size: Size,
}

pub fn paint(programs: &[Program], rgb: [usize; 3], alpha: Option<usize>, env: &Env) -> Image {
    let mut img = Image::new(env.size);
    let (w, h) = (env.size.width as usize, env.size.height as usize);
    let stack = programs.iter().map(|p| p.stack).max().unwrap_or(0);
    img.for_each_row(|y, row| {
        let mut values = Vec::with_capacity(stack);
        let mut out = vec![0.0; programs.len()];
        let v = (y as f32 + 0.5) / h as f32;
        for (x, p) in row.iter_mut().enumerate() {
            let at = Point {
                u: (x as f32 + 0.5) / w as f32,
                v,
                index: y * w + x,
            };
            for (o, prog) in out.iter_mut().zip(programs) {
                *o = prog.eval(&at, env, &mut values);
            }
            let alpha = alpha.map_or(1.0, |i| out[i]);
            *p = [out[rgb[0]], out[rgb[1]], out[rgb[2]], alpha];
        }
    });
    img
}

struct Point {
    u: f32,
    v: f32,
    index: usize,
}

impl Program {
    fn eval(&self, at: &Point, env: &Env, values: &mut Vec<f32>) -> f32 {
        values.clear();
        for instr in &self.code {
            match *instr {
                Instr::Load(l) => values.push(load(l, at, env)),
                Instr::Call(f) => {
                    let base = values.len().saturating_sub(f.arity());
                    let arg = |i: usize| values.get(base + i).copied().unwrap_or(0.0);
                    let r = call(f, arg(0), arg(1), arg(2));
                    values.truncate(base);
                    values.push(r);
                }
            }
        }
        values.pop().unwrap_or(0.0)
    }
}

fn load(l: Load, at: &Point, env: &Env) -> f32 {
    match l {
        Load::Num(n) => n,
        Load::U => at.u,
        Load::V => at.v,
        Load::Input { slot, channel } => env
            .inputs
            .get(usize::from(slot))
            .and_then(|img| img.pixels().get(at.index))
            .map_or(0.0, |p| p[usize::from(channel)]),
        Load::K(k) => env.k[usize::from(k)].at(at.index),
    }
}

impl Func {
    pub(super) fn arity(self) -> usize {
        use Func::*;
        match self {
            Neg | Abs | Floor | Ceil | Round | Fract | Sqrt | Exp | Log | Sin | Cos | Tan
            | Sign => 1,
            Clamp | Mix | Smoothstep => 3,
            _ => 2,
        }
    }
}

/// A function `math` also has is `math`'s, edges included: division by zero is 0,
/// `sqrt` or `^` of a negative base reads it as 0.
fn call(f: Func, x: f32, y: f32, z: f32) -> f32 {
    let truth = |b: bool| if b { 1.0 } else { 0.0 };
    match f {
        Func::Neg => -x,
        Func::Add => math(MathOp::Add, x, y),
        Func::Sub => math(MathOp::Subtract, x, y),
        Func::Mul => math(MathOp::Multiply, x, y),
        Func::Div => math(MathOp::Divide, x, y),
        Func::Pow => math(MathOp::Power, x, y),
        Func::Min => math(MathOp::Min, x, y),
        Func::Max => math(MathOp::Max, x, y),
        Func::Abs => math(MathOp::Abs, x, y),
        Func::Fract => math(MathOp::Fract, x, y),
        Func::Sqrt => math(MathOp::Sqrt, x, y),
        Func::Sin => math(MathOp::Sin, x, y),
        Func::Cos => math(MathOp::Cos, x, y),
        Func::Atan2 => math(MathOp::Atan2, x, y),
        Func::Lt => truth(x < y),
        Func::Le => truth(x <= y),
        Func::Gt => truth(x > y),
        Func::Ge => truth(x >= y),
        Func::Eq => truth(x == y),
        Func::Ne => truth(x != y),
        Func::Floor => x.floor(),
        Func::Ceil => x.ceil(),
        Func::Round => x.round(),
        Func::Exp => x.exp(),
        Func::Tan => x.tan(),
        // As `divide`'s zero: no -inf or NaN out of a non-positive argument.
        Func::Log if x > 0.0 => x.ln(),
        Func::Log => 0.0,
        Func::Mod => floored_mod(x, y),
        Func::Clamp => clamp_value(x, y, z),
        Func::Mix => x + (y - x) * z,
        Func::Step => truth(y >= x),
        Func::Smoothstep => smoothstep(x, y, z),
        Func::Sign if x > 0.0 => 1.0,
        Func::Sign if x < 0.0 => -1.0,
        Func::Sign => x,
    }
}

/// `x - y·floor(x / y)`, GLSL's `mod`: the result takes `y`'s sign. 0 when `y` is
/// within `divide`'s guard of zero.
fn floored_mod(x: f32, y: f32) -> f32 {
    if y.abs() < 1e-9 {
        return 0.0;
    }
    x - y * (x / y).floor()
}

/// GLSL's `smoothstep(e0, e1, x)`: 0 below `e0`, 1 above `e1`, a cubic between.
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if (e1 - e0).abs() < 1e-9 {
        return if x >= e0 { 1.0 } else { 0.0 };
    }
    let t = clamp_value((x - e0) / (e1 - e0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
