//! Backend-independent, typed scalar SSA slice of Tokit IR.
//!
//! This first slice lowers pure scalar expression functions. Unsupported
//! constructs stay on the existing checked-AST backend path.

use std::collections::HashMap;

use crate::ast::{self, Expr, ExprKind, Op, Span, Stmt, Type};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl From<Op> for BinaryOp {
    fn from(value: Op) -> Self {
        match value {
            Op::Add => Self::Add,
            Op::Sub => Self::Sub,
            Op::Mul => Self::Mul,
            Op::Div => Self::Div,
            Op::Eq => Self::Eq,
            Op::Ne => Self::Ne,
            Op::Lt => Self::Lt,
            Op::Le => Self::Le,
            Op::Gt => Self::Gt,
            Op::Ge => Self::Ge,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstructionKind {
    Parameter(usize),
    I32(i32),
    I64(i64),
    Bool(bool),
    CheckedNeg(ValueId),
    Binary(BinaryOp, ValueId, ValueId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub id: ValueId,
    pub kind: InstructionKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret: Type,
    pub span: Span,
    pub instructions: Vec<Instruction>,
    pub result: ValueId,
}

impl Function {
    /// Check SSA ordering and operand/result types before a backend consumes IR.
    pub fn verify(&self) -> Result<(), &'static str> {
        for (index, instruction) in self.instructions.iter().enumerate() {
            if instruction.id.0 != index {
                return Err("instruction IDs must match their order");
            }
            let operand = |id: ValueId| {
                self.instructions
                    .get(id.0)
                    .filter(|_| id.0 < index)
                    .map(|instruction| &instruction.ty)
            };
            match instruction.kind {
                InstructionKind::Parameter(param) => {
                    if self.params.get(param).map(|(_, ty)| ty) != Some(&instruction.ty) {
                        return Err("parameter type mismatch");
                    }
                }
                InstructionKind::I32(_) if instruction.ty == Type::I32 => {}
                InstructionKind::I64(_) if instruction.ty == Type::I64 => {}
                InstructionKind::Bool(_) if instruction.ty == Type::Bool => {}
                InstructionKind::I32(_) | InstructionKind::I64(_) | InstructionKind::Bool(_) => {
                    return Err("literal type mismatch");
                }
                InstructionKind::CheckedNeg(value) => {
                    if !matches!(instruction.ty, Type::I32 | Type::I64)
                        || operand(value) != Some(&instruction.ty)
                    {
                        return Err("negation operand type mismatch");
                    }
                }
                InstructionKind::Binary(op, left, right) => {
                    let left_ty = operand(left).ok_or("binary operand is not defined")?;
                    let right_ty = operand(right).ok_or("binary operand is not defined")?;
                    if left_ty != right_ty {
                        return Err("binary operand types differ");
                    }
                    match op {
                        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                            if !matches!(left_ty, Type::I32 | Type::I64)
                                || *left_ty != instruction.ty
                            {
                                return Err("arithmetic result type mismatch");
                            }
                        }
                        BinaryOp::Eq | BinaryOp::Ne => {
                            if !scalar(left_ty) || instruction.ty != Type::Bool {
                                return Err("equality result type mismatch");
                            }
                        }
                        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                            if !matches!(left_ty, Type::I32 | Type::I64)
                                || instruction.ty != Type::Bool
                            {
                                return Err("ordering result type mismatch");
                            }
                        }
                    }
                }
            }
        }
        if self.instructions.get(self.result.0).map(|value| &value.ty) != Some(&self.ret) {
            return Err("function result type mismatch");
        }
        Ok(())
    }
}

fn scalar(ty: &Type) -> bool {
    matches!(ty, Type::I32 | Type::I64 | Type::Bool)
}

struct Lowerer<'a> {
    params: &'a [(String, Type)],
    types: &'a HashMap<Span, Type>,
    instructions: Vec<Instruction>,
    locals: Vec<HashMap<String, ValueId>>,
}

impl Lowerer<'_> {
    fn push(&mut self, kind: InstructionKind, ty: Type, span: Span) -> ValueId {
        let id = ValueId(self.instructions.len());
        self.instructions.push(Instruction { id, kind, ty, span });
        id
    }

    fn expression(&mut self, expr: &Expr) -> Option<ValueId> {
        if let ExprKind::Block(statements, tail) = &expr.kind {
            self.locals.push(HashMap::new());
            let result = (|| {
                for statement in statements {
                    match statement {
                        Stmt::Let {
                            name,
                            value,
                            mutable: false,
                            ..
                        } => {
                            let id = self.expression(value)?;
                            self.locals.last_mut()?.insert(name.clone(), id);
                        }
                        Stmt::Expr(value) => {
                            self.expression(value)?;
                        }
                        _ => return None,
                    }
                }
                self.expression(tail.as_deref()?)
            })();
            self.locals.pop();
            return result;
        }
        let ty = self.types.get(&expr.span)?.clone();
        if !scalar(&ty) {
            return None;
        }
        let kind = match &expr.kind {
            ExprKind::Int(value) => InstructionKind::I32(*value),
            ExprKind::I64(value) => InstructionKind::I64(*value),
            ExprKind::Bool(value) => InstructionKind::Bool(*value),
            ExprKind::Var(name) => {
                if let Some(id) = self.locals.iter().rev().find_map(|scope| scope.get(name)) {
                    return Some(*id);
                }
                InstructionKind::Parameter(self.params.iter().position(|(param, _)| param == name)?)
            }
            ExprKind::Neg(value) => InstructionKind::CheckedNeg(self.expression(value)?),
            ExprKind::Binary(left, op, right) => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                InstructionKind::Binary((*op).into(), left, right)
            }
            _ => return None,
        };
        Some(self.push(kind, ty, expr.span))
    }
}

/// Lower a checked function when every expression fits the scalar IR slice.
pub fn lower_function(function: &ast::Function, types: &HashMap<Span, Type>) -> Option<Function> {
    if !function.type_params.is_empty()
        || !scalar(&function.ret)
        || function.params.iter().any(|(_, ty)| !scalar(ty))
    {
        return None;
    }
    let mut lowerer = Lowerer {
        params: &function.params,
        types,
        instructions: Vec::new(),
        locals: Vec::new(),
    };
    let result = lowerer.expression(&function.body)?;
    if lowerer.instructions[result.0].ty != function.ret {
        return None;
    }
    let lowered = Function {
        name: function.name.clone(),
        params: function.params.clone(),
        ret: function.ret.clone(),
        span: function.span,
        instructions: lowerer.instructions,
        result,
    };
    lowered.verify().ok()?;
    Some(lowered)
}
