//! A small interpreter for PURE WGSL functions, so a lockstep test can RUN the
//! shipped shader code on the CPU and compare its answers with the Rust twin.
//!
//! Why this exists (2026-09-27, environment Layer 1): the older twin tests in
//! this repo (the ocean waves in `terrain::ocean_waves`, the region record in
//! `renderer::env_regions`) parse the shader TEXT for constants and struct
//! fields. That catches a retuned constant, but not a formula that drifted: a
//! sign flipped in one twin, a term dropped, two lanes swapped in a pack. The
//! only check that cannot be fooled by that is to evaluate both twins on the
//! same inputs and compare the numbers. No GPU is needed and nothing boots.
//!
//! How: naga (already in the tree through wgpu) parses the assembled
//! megashader into its IR, and this walks the IR of one function: expressions
//! evaluated at their `Emit`, `let`, `var`, `if`, `loop`, calls into other
//! functions, and the component-wise arithmetic and math builtins, all in f32
//! as a GPU would. Anything it does not support (textures, storage buffers,
//! matrices, derivatives, atomics) is an ERROR, never a silent zero, so a test
//! that reaches it fails loudly instead of comparing against nothing.
//!
//! What it does NOT prove: that a particular GPU's `sin` or `pow` rounds the
//! way Rust's f32 does. Transcendentals on real hardware can differ by a few
//! ulp. That is a precision question the twin's tolerances absorb; the thing
//! this guards is the formula.
//!
//! Test-only (declared under `#[cfg(test)]` in `shader_loader.rs`).

use wgpu::naga;
use naga::{
    ArraySize, BinaryOperator, Expression, Function, Handle, Literal, MathFunction, Module,
    RelationalFunction, ScalarKind, Statement, TypeInner, UnaryOperator,
};

/// A runtime value. Numbers are f32, i32, u32 or bool, alone or in a vector;
/// structs and arrays are `Composite`; `Pointer` is a function-local `var`
/// plus a path of member or element indices into it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    F32(f32),
    I32(i32),
    U32(u32),
    Bool(bool),
    Vector(Vec<Value>),
    Composite(Vec<Value>),
    Pointer(usize, Vec<usize>),
}

impl Value {
    /// A float vector from components.
    pub fn vec(components: &[f32]) -> Value {
        Value::Vector(components.iter().map(|c| Value::F32(*c)).collect())
    }

    /// The float components of a scalar or vector, in order.
    pub fn floats(&self) -> Result<Vec<f32>, String> {
        match self {
            Value::F32(v) => Ok(vec![*v]),
            Value::Vector(c) => c.iter().map(|v| v.as_f32()).collect(),
            other => Err(format!("expected a float or float vector, got {other:?}")),
        }
    }

    fn as_f32(&self) -> Result<f32, String> {
        match self {
            Value::F32(v) => Ok(*v),
            other => Err(format!("expected f32, got {other:?}")),
        }
    }

    fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(format!("expected bool, got {other:?}")),
        }
    }

    fn as_index(&self) -> Result<usize, String> {
        match self {
            Value::I32(v) if *v >= 0 => Ok(*v as usize),
            Value::U32(v) => Ok(*v as usize),
            other => Err(format!("expected a non-negative index, got {other:?}")),
        }
    }

    fn components(&self) -> Vec<Value> {
        match self {
            Value::Vector(c) => c.clone(),
            other => vec![other.clone()],
        }
    }
}

/// The interpreter over one parsed module.
pub struct Evaluator<'m> {
    module: &'m Module,
}

/// Why a block stopped.
enum Flow {
    Next,
    Break,
    Continue,
    Return(Option<Value>),
}

/// One function invocation's state.
struct Frame<'m> {
    func: &'m Function,
    args: Vec<Value>,
    cache: Vec<Option<Value>>,
    locals: Vec<Value>,
}

/// Iterations any one `loop` may run before the evaluator assumes it is stuck.
const LOOP_CAP: usize = 1_000_000;

impl<'m> Evaluator<'m> {
    pub fn new(module: &'m Module) -> Self {
        Self { module }
    }

    /// Parse WGSL source (no validation: the build's own shader tests do that).
    pub fn parse(source: &str) -> Result<Module, String> {
        naga::front::wgsl::parse_str(source).map_err(|e| format!("parse error: {e}"))
    }

    /// The handle of a named type (a struct), for `value_from_floats`.
    pub fn type_named(&self, name: &str) -> Result<Handle<naga::Type>, String> {
        self.module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some(name))
            .map(|(h, _)| h)
            .ok_or_else(|| format!("no type named {name} in the module"))
    }

    /// A struct type's size in bytes as the module lays it out (None if the
    /// type is not a struct).
    pub fn struct_span(&self, ty: Handle<naga::Type>) -> Option<u32> {
        match &self.module.types[ty].inner {
            TypeInner::Struct { span, .. } => Some(*span),
            _ => None,
        }
    }

    /// Build a value of type `ty` from a flat f32 buffer laid out as the GPU
    /// would read it, using the MODULE's own member offsets and array strides.
    /// This is what makes a lockstep test also a layout test: a Rust pack
    /// whose order differs from the shader struct produces the wrong value here.
    pub fn value_from_floats(&self, ty: Handle<naga::Type>, floats: &[f32]) -> Result<Value, String> {
        self.from_floats_at(ty, floats, 0)
    }

    fn from_floats_at(&self, ty: Handle<naga::Type>, floats: &[f32], byte_offset: u32) -> Result<Value, String> {
        let at = |i: u32| -> Result<f32, String> {
            let k = (byte_offset / 4 + i) as usize;
            floats
                .get(k)
                .copied()
                .ok_or_else(|| format!("buffer too short: float {k} of {}", floats.len()))
        };
        match &self.module.types[ty].inner {
            TypeInner::Scalar(s) if s.kind == ScalarKind::Float && s.width == 4 => Ok(Value::F32(at(0)?)),
            TypeInner::Vector { size, scalar } if scalar.kind == ScalarKind::Float && scalar.width == 4 => {
                let n = *size as u32;
                let mut c = Vec::with_capacity(n as usize);
                for i in 0..n {
                    c.push(Value::F32(at(i)?));
                }
                Ok(Value::Vector(c))
            }
            TypeInner::Struct { members, .. } => members
                .iter()
                .map(|m| self.from_floats_at(m.ty, floats, byte_offset + m.offset))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Composite),
            TypeInner::Array { base, size: ArraySize::Constant(n), stride } => (0..n.get())
                .map(|i| self.from_floats_at(*base, floats, byte_offset + i * stride))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Composite),
            other => Err(format!("value_from_floats: unsupported type {other:?}")),
        }
    }

    /// Call a function by name with argument values; its return value.
    pub fn call(&self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        let (h, _) = self
            .module
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some(name))
            .ok_or_else(|| format!("no function named {name}"))?;
        self.call_handle(h, args, 0)?
            .ok_or_else(|| format!("{name} returned nothing"))
    }

    fn call_handle(&self, h: Handle<Function>, args: Vec<Value>, depth: usize) -> Result<Option<Value>, String> {
        if depth > 64 {
            return Err("call depth over 64: recursion is not WGSL".into());
        }
        let func = &self.module.functions[h];
        if args.len() != func.arguments.len() {
            return Err(format!(
                "{:?} takes {} arguments, got {}",
                func.name,
                func.arguments.len(),
                args.len()
            ));
        }
        let mut fr = Frame {
            func,
            args,
            cache: vec![None; func.expressions.len()],
            locals: Vec::with_capacity(func.local_variables.len()),
        };
        // Locals start at their constant initialiser or zero (WGSL zero-fills).
        let inits: Vec<_> = func.local_variables.iter().map(|(_, lv)| (lv.ty, lv.init)).collect();
        for (ty, init) in inits {
            let v = match init {
                Some(e) => self.expr(&mut fr, e, depth)?,
                None => self.zero(ty)?,
            };
            fr.locals.push(v);
        }
        match self.block(&mut fr, &func.body, depth)? {
            Flow::Return(v) => Ok(v),
            Flow::Next => Ok(None),
            Flow::Break | Flow::Continue => Err("break/continue escaped a function".into()),
        }
    }

    fn block(&self, fr: &mut Frame<'m>, block: &naga::Block, depth: usize) -> Result<Flow, String> {
        for stmt in block.iter() {
            let flow = self.statement(fr, stmt, depth)?;
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }

    fn statement(&self, fr: &mut Frame<'m>, stmt: &Statement, depth: usize) -> Result<Flow, String> {
        match stmt {
            Statement::Emit(range) => {
                // Re-evaluate every time: inside a loop the same range is
                // emitted once per iteration with new operands.
                for h in range.clone() {
                    let v = self.compute(fr, h, depth)?;
                    fr.cache[h.index()] = Some(v);
                }
                Ok(Flow::Next)
            }
            Statement::Block(b) => self.block(fr, b, depth),
            Statement::If { condition, accept, reject } => {
                if self.expr(fr, *condition, depth)?.as_bool()? {
                    self.block(fr, accept, depth)
                } else {
                    self.block(fr, reject, depth)
                }
            }
            Statement::Loop { body, continuing, break_if } => {
                for _ in 0..LOOP_CAP {
                    match self.block(fr, body, depth)? {
                        Flow::Break => return Ok(Flow::Next),
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                    match self.block(fr, continuing, depth)? {
                        Flow::Break => return Ok(Flow::Next),
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Next | Flow::Continue => {}
                    }
                    if let Some(b) = break_if {
                        if self.expr(fr, *b, depth)?.as_bool()? {
                            return Ok(Flow::Next);
                        }
                    }
                }
                Err(format!("a loop ran {LOOP_CAP} iterations"))
            }
            Statement::Break => Ok(Flow::Break),
            Statement::Continue => Ok(Flow::Continue),
            Statement::Return { value } => {
                let v = match value {
                    Some(e) => Some(self.expr(fr, *e, depth)?),
                    None => None,
                };
                Ok(Flow::Return(v))
            }
            Statement::Store { pointer, value } => {
                // (The value first: WGSL evaluates the right-hand side before
                // the store, and a pointer expression cannot depend on it.)
                let v = self.expr(fr, *value, depth)?;
                match self.expr(fr, *pointer, depth)? {
                    Value::Pointer(local, path) => {
                        let slot = navigate_mut(&mut fr.locals[local], &path)?;
                        *slot = v;
                        Ok(Flow::Next)
                    }
                    other => Err(format!("store through a non-pointer {other:?}")),
                }
            }
            Statement::Call { function, arguments, result } => {
                let mut args = Vec::with_capacity(arguments.len());
                for a in arguments {
                    args.push(self.expr(fr, *a, depth)?);
                }
                let r = self.call_handle(*function, args, depth + 1)?;
                if let Some(h) = result {
                    fr.cache[h.index()] =
                        Some(r.ok_or_else(|| "a call's result was used but it returned nothing".to_string())?);
                }
                Ok(Flow::Next)
            }
            other => Err(format!("unsupported statement {other:?}")),
        }
    }

    /// A value, from the cache when it was already evaluated (an `Emit`ted
    /// expression, a call result), computed on demand otherwise (constants,
    /// arguments, local pointers, and the constant-folded expressions naga
    /// never emits).
    fn expr(&self, fr: &mut Frame<'m>, h: Handle<Expression>, depth: usize) -> Result<Value, String> {
        if let Some(v) = &fr.cache[h.index()] {
            return Ok(v.clone());
        }
        let v = self.compute(fr, h, depth)?;
        fr.cache[h.index()] = Some(v.clone());
        Ok(v)
    }

    fn compute(&self, fr: &mut Frame<'m>, h: Handle<Expression>, depth: usize) -> Result<Value, String> {
        // Copy the 'm reference out first, so the expression borrows the
        // module, not the frame the evaluation below needs mutably.
        let func: &'m Function = fr.func;
        let e = &func.expressions[h];
        match e {
            Expression::FunctionArgument(i) => Ok(fr.args[*i as usize].clone()),
            Expression::LocalVariable(lv) => Ok(Value::Pointer(lv.index(), Vec::new())),
            Expression::Load { pointer } => match self.expr(fr, *pointer, depth)? {
                Value::Pointer(local, path) => navigate(&fr.locals[local], &path).cloned(),
                other => Err(format!("load through a non-pointer {other:?}")),
            },
            Expression::CallResult(_) => Err("a call result was read before its call ran".into()),
            Expression::Access { base, index } => {
                let b = self.expr(fr, *base, depth)?;
                let i = self.expr(fr, *index, depth)?.as_index()?;
                access(b, i)
            }
            Expression::AccessIndex { base, index } => {
                let b = self.expr(fr, *base, depth)?;
                access(b, *index as usize)
            }
            _ => {
                // Everything else is pure in its operands: evaluate them, then
                // share the arithmetic with the module-scope constant path.
                let mut ops = Vec::new();
                for op in operands(e) {
                    ops.push(self.expr(fr, op, depth)?);
                }
                self.pure(e, ops)
            }
        }
    }

    /// A module-scope constant's value.
    fn constant(&self, h: Handle<naga::Constant>) -> Result<Value, String> {
        self.global(self.module.constants[h].init)
    }

    fn global(&self, h: Handle<Expression>) -> Result<Value, String> {
        let e = &self.module.global_expressions[h];
        let mut ops = Vec::new();
        for op in operands(e) {
            ops.push(self.global(op)?);
        }
        self.pure(e, ops)
    }

    fn zero(&self, ty: Handle<naga::Type>) -> Result<Value, String> {
        Ok(match &self.module.types[ty].inner {
            TypeInner::Scalar(s) => zero_scalar(s.kind)?,
            TypeInner::Vector { size, scalar } => Value::Vector(vec![zero_scalar(scalar.kind)?; *size as usize]),
            TypeInner::Struct { members, .. } => {
                Value::Composite(members.iter().map(|m| self.zero(m.ty)).collect::<Result<_, _>>()?)
            }
            TypeInner::Array { base, size: ArraySize::Constant(n), .. } => {
                Value::Composite(vec![self.zero(*base)?; n.get() as usize])
            }
            other => return Err(format!("zero value of unsupported type {other:?}")),
        })
    }

    /// The expressions whose value depends only on already-evaluated operands.
    fn pure(&self, e: &Expression, ops: Vec<Value>) -> Result<Value, String> {
        match e {
            Expression::Literal(l) => literal(l),
            Expression::Constant(c) => self.constant(*c),
            Expression::ZeroValue(ty) => self.zero(*ty),
            Expression::Compose { ty, .. } => match &self.module.types[*ty].inner {
                TypeInner::Vector { .. } => Ok(Value::Vector(ops.iter().flat_map(|v| v.components()).collect())),
                TypeInner::Struct { .. } | TypeInner::Array { .. } => Ok(Value::Composite(ops)),
                other => Err(format!("compose of unsupported type {other:?}")),
            },
            Expression::Splat { size, .. } => Ok(Value::Vector(vec![ops[0].clone(); *size as usize])),
            Expression::Swizzle { size, pattern, .. } => {
                let c = ops[0].components();
                Ok(Value::Vector(pattern[..*size as usize].iter().map(|p| c[*p as usize].clone()).collect()))
            }
            Expression::Unary { op, .. } => map1(&ops[0], |v| unary(*op, v)),
            Expression::Binary { op, .. } => map2(&ops[0], &ops[1], |a, b| binary(*op, a, b)),
            Expression::Select { .. } => {
                // ops: condition, accept, reject
                match &ops[0] {
                    Value::Bool(c) => Ok(if *c { ops[1].clone() } else { ops[2].clone() }),
                    Value::Vector(cs) => {
                        let (a, r) = (ops[1].components(), ops[2].components());
                        cs.iter()
                            .enumerate()
                            .map(|(i, c)| Ok(if c.as_bool()? { a[i].clone() } else { r[i].clone() }))
                            .collect::<Result<Vec<_>, String>>()
                            .map(Value::Vector)
                    }
                    other => Err(format!("select condition {other:?}")),
                }
            }
            Expression::Relational { fun, .. } => {
                let c = ops[0].components();
                match fun {
                    RelationalFunction::All => Ok(Value::Bool(c.iter().all(|v| *v == Value::Bool(true)))),
                    RelationalFunction::Any => Ok(Value::Bool(c.iter().any(|v| *v == Value::Bool(true)))),
                    RelationalFunction::IsNan => map1(&ops[0], |v| Ok(Value::Bool(v.as_f32()?.is_nan()))),
                    RelationalFunction::IsInf => map1(&ops[0], |v| Ok(Value::Bool(v.as_f32()?.is_infinite()))),
                }
            }
            Expression::Math { fun, .. } => math(*fun, &ops),
            Expression::As { kind, convert, .. } => map1(&ops[0], |v| cast(v, *kind, convert.is_some())),
            other => Err(format!("unsupported expression {other:?}")),
        }
    }
}

/// The operand handles of a pure expression, in the order `pure` reads them.
fn operands(e: &Expression) -> Vec<Handle<Expression>> {
    match e {
        Expression::Compose { components, .. } => components.clone(),
        Expression::Splat { value, .. } => vec![*value],
        Expression::Swizzle { vector, .. } => vec![*vector],
        Expression::Unary { expr, .. } => vec![*expr],
        Expression::Binary { left, right, .. } => vec![*left, *right],
        Expression::Select { condition, accept, reject } => vec![*condition, *accept, *reject],
        Expression::Relational { argument, .. } => vec![*argument],
        Expression::Math { arg, arg1, arg2, arg3, .. } => {
            let mut v = vec![*arg];
            v.extend(arg1.iter().chain(arg2.iter()).chain(arg3.iter()).copied());
            v
        }
        Expression::As { expr, .. } => vec![*expr],
        _ => Vec::new(),
    }
}

fn zero_scalar(kind: ScalarKind) -> Result<Value, String> {
    Ok(match kind {
        ScalarKind::Float | ScalarKind::AbstractFloat => Value::F32(0.0),
        ScalarKind::Sint | ScalarKind::AbstractInt => Value::I32(0),
        ScalarKind::Uint => Value::U32(0),
        ScalarKind::Bool => Value::Bool(false),
    })
}

fn literal(l: &Literal) -> Result<Value, String> {
    Ok(match l {
        Literal::F32(v) => Value::F32(*v),
        Literal::AbstractFloat(v) | Literal::F64(v) => Value::F32(*v as f32),
        Literal::I32(v) => Value::I32(*v),
        Literal::AbstractInt(v) | Literal::I64(v) => Value::I32(*v as i32),
        Literal::U32(v) => Value::U32(*v),
        Literal::U64(v) => Value::U32(*v as u32),
        Literal::Bool(b) => Value::Bool(*b),
    })
}

fn access(base: Value, i: usize) -> Result<Value, String> {
    match base {
        Value::Pointer(local, mut path) => {
            path.push(i);
            Ok(Value::Pointer(local, path))
        }
        Value::Vector(c) | Value::Composite(c) => c
            .get(i)
            .cloned()
            .ok_or_else(|| format!("index {i} out of range {}", c.len())),
        other => Err(format!("indexing a {other:?}")),
    }
}

fn navigate<'a>(v: &'a Value, path: &[usize]) -> Result<&'a Value, String> {
    let mut cur = v;
    for &i in path {
        cur = match cur {
            Value::Vector(c) | Value::Composite(c) => c.get(i).ok_or("pointer path out of range")?,
            other => return Err(format!("pointer path through {other:?}")),
        };
    }
    Ok(cur)
}

fn navigate_mut<'a>(v: &'a mut Value, path: &[usize]) -> Result<&'a mut Value, String> {
    let mut cur = v;
    for &i in path {
        cur = match cur {
            Value::Vector(c) | Value::Composite(c) => c.get_mut(i).ok_or("pointer path out of range")?,
            other => return Err(format!("pointer path through {other:?}")),
        };
    }
    Ok(cur)
}

/// Apply a scalar function to every component.
fn map1(v: &Value, f: impl Fn(&Value) -> Result<Value, String>) -> Result<Value, String> {
    match v {
        Value::Vector(c) => c.iter().map(&f).collect::<Result<_, _>>().map(Value::Vector),
        s => f(s),
    }
}

/// Component-wise with a scalar broadcast against a vector, as WGSL allows
/// for the arithmetic operators.
fn map2(a: &Value, b: &Value, f: impl Fn(&Value, &Value) -> Result<Value, String>) -> Result<Value, String> {
    match (a, b) {
        (Value::Vector(x), Value::Vector(y)) if x.len() == y.len() => {
            x.iter().zip(y).map(|(p, q)| f(p, q)).collect::<Result<_, _>>().map(Value::Vector)
        }
        (Value::Vector(x), s) => x.iter().map(|p| f(p, s)).collect::<Result<_, _>>().map(Value::Vector),
        (s, Value::Vector(y)) => y.iter().map(|q| f(s, q)).collect::<Result<_, _>>().map(Value::Vector),
        (p, q) => f(p, q),
    }
}

fn unary(op: UnaryOperator, v: &Value) -> Result<Value, String> {
    Ok(match (op, v) {
        (UnaryOperator::Negate, Value::F32(x)) => Value::F32(-x),
        (UnaryOperator::Negate, Value::I32(x)) => Value::I32(x.wrapping_neg()),
        (UnaryOperator::LogicalNot, Value::Bool(b)) => Value::Bool(!b),
        (UnaryOperator::BitwiseNot, Value::I32(x)) => Value::I32(!x),
        (UnaryOperator::BitwiseNot, Value::U32(x)) => Value::U32(!x),
        (op, v) => return Err(format!("unary {op:?} on {v:?}")),
    })
}

fn binary(op: BinaryOperator, a: &Value, b: &Value) -> Result<Value, String> {
    use BinaryOperator as B;
    Ok(match (a, b) {
        (Value::F32(x), Value::F32(y)) => {
            let (x, y) = (*x, *y);
            match op {
                B::Add => Value::F32(x + y),
                B::Subtract => Value::F32(x - y),
                B::Multiply => Value::F32(x * y),
                B::Divide => Value::F32(x / y),
                // WGSL float % truncates toward zero, as Rust's does.
                B::Modulo => Value::F32(x % y),
                B::Equal => Value::Bool(x == y),
                B::NotEqual => Value::Bool(x != y),
                B::Less => Value::Bool(x < y),
                B::LessEqual => Value::Bool(x <= y),
                B::Greater => Value::Bool(x > y),
                B::GreaterEqual => Value::Bool(x >= y),
                op => return Err(format!("{op:?} on floats")),
            }
        }
        (Value::I32(x), Value::I32(y)) => {
            let (x, y) = (*x, *y);
            match op {
                B::Add => Value::I32(x.wrapping_add(y)),
                B::Subtract => Value::I32(x.wrapping_sub(y)),
                B::Multiply => Value::I32(x.wrapping_mul(y)),
                B::Divide => Value::I32(if y == 0 { x } else { x.wrapping_div(y) }),
                B::Modulo => Value::I32(if y == 0 { 0 } else { x.wrapping_rem(y) }),
                B::Equal => Value::Bool(x == y),
                B::NotEqual => Value::Bool(x != y),
                B::Less => Value::Bool(x < y),
                B::LessEqual => Value::Bool(x <= y),
                B::Greater => Value::Bool(x > y),
                B::GreaterEqual => Value::Bool(x >= y),
                B::And => Value::I32(x & y),
                B::InclusiveOr => Value::I32(x | y),
                B::ExclusiveOr => Value::I32(x ^ y),
                op => return Err(format!("{op:?} on i32")),
            }
        }
        (Value::U32(x), Value::U32(y)) => {
            let (x, y) = (*x, *y);
            match op {
                B::Add => Value::U32(x.wrapping_add(y)),
                B::Subtract => Value::U32(x.wrapping_sub(y)),
                B::Multiply => Value::U32(x.wrapping_mul(y)),
                B::Divide => Value::U32(if y == 0 { x } else { x / y }),
                B::Modulo => Value::U32(if y == 0 { 0 } else { x % y }),
                B::Equal => Value::Bool(x == y),
                B::NotEqual => Value::Bool(x != y),
                B::Less => Value::Bool(x < y),
                B::LessEqual => Value::Bool(x <= y),
                B::Greater => Value::Bool(x > y),
                B::GreaterEqual => Value::Bool(x >= y),
                B::And => Value::U32(x & y),
                B::InclusiveOr => Value::U32(x | y),
                B::ExclusiveOr => Value::U32(x ^ y),
                B::ShiftLeft => Value::U32(x.wrapping_shl(y)),
                B::ShiftRight => Value::U32(x.wrapping_shr(y)),
                op => return Err(format!("{op:?} on u32")),
            }
        }
        (Value::Bool(x), Value::Bool(y)) => match op {
            B::LogicalAnd | B::And => Value::Bool(*x && *y),
            B::LogicalOr | B::InclusiveOr => Value::Bool(*x || *y),
            B::Equal => Value::Bool(x == y),
            B::NotEqual | B::ExclusiveOr => Value::Bool(x != y),
            op => return Err(format!("{op:?} on bools")),
        },
        (a, b) => return Err(format!("{op:?} between {a:?} and {b:?}")),
    })
}

fn cast(v: &Value, kind: ScalarKind, convert: bool) -> Result<Value, String> {
    if !convert {
        // Bitcast.
        return Ok(match (v, kind) {
            (Value::F32(x), ScalarKind::Uint) => Value::U32(x.to_bits()),
            (Value::F32(x), ScalarKind::Sint) => Value::I32(x.to_bits() as i32),
            (Value::U32(x), ScalarKind::Float) => Value::F32(f32::from_bits(*x)),
            (Value::I32(x), ScalarKind::Float) => Value::F32(f32::from_bits(*x as u32)),
            (Value::U32(x), ScalarKind::Sint) => Value::I32(*x as i32),
            (Value::I32(x), ScalarKind::Uint) => Value::U32(*x as u32),
            (v, k) => return Err(format!("bitcast {v:?} to {k:?}")),
        });
    }
    // Value conversion. f32 to integer truncates toward zero and saturates,
    // which is WGSL's rule and also Rust's `as`.
    Ok(match (v, kind) {
        (Value::F32(x), ScalarKind::Float) => Value::F32(*x),
        (Value::F32(x), ScalarKind::Sint) => Value::I32(*x as i32),
        (Value::F32(x), ScalarKind::Uint) => Value::U32(*x as u32),
        (Value::I32(x), ScalarKind::Float) => Value::F32(*x as f32),
        (Value::U32(x), ScalarKind::Float) => Value::F32(*x as f32),
        (Value::I32(x), ScalarKind::Uint) => Value::U32(*x as u32),
        (Value::U32(x), ScalarKind::Sint) => Value::I32(*x as i32),
        (Value::I32(x), ScalarKind::Sint) => Value::I32(*x),
        (Value::U32(x), ScalarKind::Uint) => Value::U32(*x),
        (Value::Bool(b), ScalarKind::Float) => Value::F32(if *b { 1.0 } else { 0.0 }),
        (Value::Bool(b), ScalarKind::Sint) => Value::I32(*b as i32),
        (Value::Bool(b), ScalarKind::Uint) => Value::U32(*b as u32),
        (Value::F32(x), ScalarKind::Bool) => Value::Bool(*x != 0.0),
        (v, k) => return Err(format!("convert {v:?} to {k:?}")),
    })
}

/// WGSL `round` rounds half to even.
fn round_half_even(x: f32) -> f32 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        r
    }
}

fn math(fun: MathFunction, ops: &[Value]) -> Result<Value, String> {
    use MathFunction as M;
    let f1 = |g: fn(f32) -> f32| map1(&ops[0], move |v| Ok(Value::F32(g(v.as_f32()?))));
    match fun {
        // Geometry first: these are not component-wise.
        M::Dot => {
            let (a, b) = (ops[0].floats()?, ops[1].floats()?);
            Ok(Value::F32(a.iter().zip(&b).map(|(x, y)| x * y).sum()))
        }
        M::Length => Ok(Value::F32(ops[0].floats()?.iter().map(|x| x * x).sum::<f32>().sqrt())),
        M::Distance => {
            let (a, b) = (ops[0].floats()?, ops[1].floats()?);
            Ok(Value::F32(a.iter().zip(&b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()))
        }
        M::Normalize => {
            let a = ops[0].floats()?;
            let l = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            Ok(Value::vec(&a.iter().map(|x| x / l).collect::<Vec<_>>()))
        }
        M::Cross => {
            let (a, b) = (ops[0].floats()?, ops[1].floats()?);
            Ok(Value::vec(&[a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]))
        }
        M::Abs => map1(&ops[0], |v| match v {
            Value::F32(x) => Ok(Value::F32(x.abs())),
            Value::I32(x) => Ok(Value::I32(x.wrapping_abs())),
            Value::U32(x) => Ok(Value::U32(*x)),
            other => Err(format!("abs of {other:?}")),
        }),
        M::Min | M::Max => map2(&ops[0], &ops[1], |a, b| {
            let min = fun == M::Min;
            Ok(match (a, b) {
                (Value::F32(x), Value::F32(y)) => Value::F32(if min { x.min(*y) } else { x.max(*y) }),
                (Value::I32(x), Value::I32(y)) => Value::I32(if min { *x.min(y) } else { *x.max(y) }),
                (Value::U32(x), Value::U32(y)) => Value::U32(if min { *x.min(y) } else { *x.max(y) }),
                (a, b) => return Err(format!("min/max of {a:?}, {b:?}")),
            })
        }),
        M::Clamp => {
            let lo = map2(&ops[0], &ops[1], |a, b| math(M::Max, &[a.clone(), b.clone()]))?;
            map2(&lo, &ops[2], |a, b| math(M::Min, &[a.clone(), b.clone()]))
        }
        M::Saturate => f1(|x| x.clamp(0.0, 1.0)),
        M::Cos => f1(f32::cos),
        M::Sin => f1(f32::sin),
        M::Tan => f1(f32::tan),
        M::Cosh => f1(f32::cosh),
        M::Sinh => f1(f32::sinh),
        M::Tanh => f1(f32::tanh),
        M::Acos => f1(f32::acos),
        M::Asin => f1(f32::asin),
        M::Atan => f1(f32::atan),
        M::Atan2 => map2(&ops[0], &ops[1], |a, b| Ok(Value::F32(a.as_f32()?.atan2(b.as_f32()?)))),
        M::Radians => f1(f32::to_radians),
        M::Degrees => f1(f32::to_degrees),
        M::Ceil => f1(f32::ceil),
        M::Floor => f1(f32::floor),
        M::Round => f1(round_half_even),
        M::Fract => f1(|x| x - x.floor()),
        M::Trunc => f1(f32::trunc),
        M::Exp => f1(f32::exp),
        M::Exp2 => f1(f32::exp2),
        M::Log => f1(f32::ln),
        M::Log2 => f1(f32::log2),
        M::Sqrt => f1(f32::sqrt),
        M::InverseSqrt => f1(|x| 1.0 / x.sqrt()),
        M::Sign => f1(|x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }),
        M::Pow => map2(&ops[0], &ops[1], |a, b| Ok(Value::F32(a.as_f32()?.powf(b.as_f32()?)))),
        M::Step => map2(&ops[0], &ops[1], |e, x| {
            Ok(Value::F32(if x.as_f32()? >= e.as_f32()? { 1.0 } else { 0.0 }))
        }),
        M::Fma => {
            let ab = map2(&ops[0], &ops[1], |a, b| binary(BinaryOperator::Multiply, a, b))?;
            map2(&ab, &ops[2], |p, c| binary(BinaryOperator::Add, p, c))
        }
        M::Mix => {
            // mix(a, b, t) = a * (1 - t) + b * t, t scalar or per component.
            let (a, b) = (ops[0].components(), ops[1].components());
            let t = ops[2].components();
            let out = a
                .iter()
                .zip(&b)
                .enumerate()
                .map(|(i, (x, y))| {
                    let t = t.get(i).or(t.first()).ok_or("mix without t")?.as_f32()?;
                    Ok(Value::F32(x.as_f32()? * (1.0 - t) + y.as_f32()? * t))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(if out.len() == 1 { out[0].clone() } else { Value::Vector(out) })
        }
        M::SmoothStep => {
            let lo = ops[0].components();
            let hi = ops[1].components();
            let xs = ops[2].components();
            let out = xs
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    let e0 = lo.get(i).or(lo.first()).ok_or("smoothstep")?.as_f32()?;
                    let e1 = hi.get(i).or(hi.first()).ok_or("smoothstep")?.as_f32()?;
                    let t = ((x.as_f32()? - e0) / (e1 - e0)).clamp(0.0, 1.0);
                    Ok(Value::F32(t * t * (3.0 - 2.0 * t)))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(if out.len() == 1 { out[0].clone() } else { Value::Vector(out) })
        }
        other => Err(format!("unsupported builtin {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The evaluator itself, on a small module whose answers are known: `let`,
    /// `var`, `if`, a `for` loop, a call, a struct argument, swizzles, select,
    /// and the math builtins the climate twin leans on. A regression here would
    /// make every lockstep test built on it meaningless, so it gets its own
    /// check against hand-computed values.
    #[test]
    fn the_evaluator_runs_a_known_module() {
        let src = r#"
            struct Pair { a: vec4<f32>, b: array<vec4<f32>, 2> };
            fn helper(x: f32) -> f32 { return x * 2.0 + 1.0; }
            fn sum_to(n: u32) -> f32 {
                var total = 0.0;
                for (var i = 0u; i < n; i = i + 1u) {
                    if (i == 2u) { continue; }
                    total = total + f32(i);
                }
                return total;
            }
            fn uses_struct(p: Pair, t: f32) -> vec2<f32> {
                let s = select(p.b[0], p.b[1], t > 0.5);
                let m = mix(p.a.xy, s.zw, 0.25);
                return vec2<f32>(dot(p.a, s), helper(m.x) + pow(2.0, 3.0) + clamp(-4.0, -1.0, 1.0));
            }
        "#;
        let module = Evaluator::parse(src).expect("parses");
        let ev = Evaluator::new(&module);
        // 0 + 1 + 3 + 4 (2 skipped by continue)
        assert_eq!(ev.call("sum_to", vec![Value::U32(5)]).unwrap(), Value::F32(8.0));
        let ty = ev.type_named("Pair").unwrap();
        let p = ev
            .value_from_floats(ty, &[1.0, 2.0, 3.0, 4.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 10.0, 20.0])
            .unwrap();
        // t > 0.5 picks b[1] = (0,1,10,20): dot(a, s) = 2 + 30 + 80 = 112.
        // m = mix((1,2), (10,20), 0.25) = (3.25, 6.5); helper(3.25) = 7.5;
        // pow 8; clamp(-4,-1,1) = -1  => 14.5.
        let out = ev.call("uses_struct", vec![p, Value::F32(0.9)]).unwrap();
        assert_eq!(out.floats().unwrap(), vec![112.0, 14.5]);
    }

    /// Unsupported features fail loudly, never as a silent zero.
    #[test]
    fn unsupported_features_are_errors() {
        let src = r#"
            fn uses_deriv(x: f32) -> f32 { return dpdx(x); }
        "#;
        let module = Evaluator::parse(src).expect("parses");
        let ev = Evaluator::new(&module);
        assert!(ev.call("uses_deriv", vec![Value::F32(1.0)]).is_err());
        assert!(ev.call("no_such_function", vec![]).is_err());
    }
}
