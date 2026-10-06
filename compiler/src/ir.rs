//! Backend-independent, typed scalar SSA slice of Tokit IR.
//!
//! This slice lowers scalar functions with checked calls. Unsupported
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

impl TryFrom<Op> for BinaryOp {
    type Error = ();

    fn try_from(value: Op) -> Result<Self, Self::Error> {
        Ok(match value {
            Op::And | Op::Or => return Err(()),
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
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstructionKind {
    Parameter(usize),
    I32(i32),
    I64(i64),
    F64(u64),
    Bool(bool),
    Not(ValueId),
    CheckedNeg(ValueId),
    FloatNeg(ValueId),
    Binary(BinaryOp, ValueId, ValueId),
    Call(String, Vec<ValueId>),
    Conditional {
        condition: ValueId,
        yes: Region,
        no: Region,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub instructions: Vec<Instruction>,
    pub result: ValueId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub id: ValueId,
    pub kind: InstructionKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub params: Vec<Type>,
    pub ret: Type,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret: Type,
    pub span: Span,
    pub callees: HashMap<String, Signature>,
    pub instructions: Vec<Instruction>,
    pub result: ValueId,
}

impl Function {
    /// Check SSA ordering and operand/result types before a backend consumes IR.
    pub fn verify(&self) -> Result<(), &'static str> {
        if !scalar(&self.ret) || self.params.iter().any(|(_, ty)| !scalar(ty)) {
            return Err("function signature is not scalar");
        }
        let mut next_id = 0;
        let values = verify_instructions(
            &self.instructions,
            &self.params,
            &self.callees,
            &HashMap::new(),
            &mut next_id,
        )?;
        if values.get(&self.result.0) != Some(&self.ret) {
            return Err("function result type mismatch");
        }
        Ok(())
    }
}

fn verify_instructions(
    instructions: &[Instruction],
    params: &[(String, Type)],
    callees: &HashMap<String, Signature>,
    outer: &HashMap<usize, Type>,
    next_id: &mut usize,
) -> Result<HashMap<usize, Type>, &'static str> {
    let mut values = outer.clone();
    for instruction in instructions {
        if instruction.id.0 != *next_id {
            return Err("instruction IDs must match their order");
        }
        *next_id += 1;
        let operand = |id: ValueId| values.get(&id.0).filter(|_| id.0 < instruction.id.0);
        match instruction.kind {
            InstructionKind::Parameter(param) => {
                if params.get(param).map(|(_, ty)| ty) != Some(&instruction.ty) {
                    return Err("parameter type mismatch");
                }
            }
            InstructionKind::I32(_) if instruction.ty == Type::I32 => {}
            InstructionKind::I64(_) if instruction.ty == Type::I64 => {}
            InstructionKind::F64(_) if instruction.ty == Type::F64 => {}
            InstructionKind::Bool(_) if instruction.ty == Type::Bool => {}
            InstructionKind::I32(_)
            | InstructionKind::I64(_)
            | InstructionKind::F64(_)
            | InstructionKind::Bool(_) => {
                return Err("literal type mismatch");
            }
            InstructionKind::Not(value) => {
                if instruction.ty != Type::Bool || operand(value) != Some(&Type::Bool) {
                    return Err("logical negation operand type mismatch");
                }
            }
            InstructionKind::CheckedNeg(value) => {
                if !matches!(instruction.ty, Type::I32 | Type::I64)
                    || operand(value) != Some(&instruction.ty)
                {
                    return Err("negation operand type mismatch");
                }
            }
            InstructionKind::FloatNeg(value) => {
                if instruction.ty != Type::F64 || operand(value) != Some(&Type::F64) {
                    return Err("float negation operand type mismatch");
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
                        if !matches!(left_ty, Type::I32 | Type::I64 | Type::F64)
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
                        if !matches!(left_ty, Type::I32 | Type::I64 | Type::F64)
                            || instruction.ty != Type::Bool
                        {
                            return Err("ordering result type mismatch");
                        }
                    }
                }
            }
            InstructionKind::Call(ref name, ref args) => {
                let signature = callees.get(name).ok_or("call target is not defined")?;
                if !scalar(&signature.ret) || signature.params.iter().any(|ty| !scalar(ty)) {
                    return Err("call signature is not scalar");
                }
                if signature.params.len() != args.len() || signature.ret != instruction.ty {
                    return Err("call signature mismatch");
                }
                for (arg, expected) in args.iter().zip(&signature.params) {
                    if operand(*arg) != Some(expected) {
                        return Err("call argument type mismatch");
                    }
                }
            }
            InstructionKind::Conditional {
                condition,
                ref yes,
                ref no,
            } => {
                if operand(condition) != Some(&Type::Bool) {
                    return Err("condition must be a defined bool");
                }
                let yes_values =
                    verify_instructions(&yes.instructions, params, callees, &values, next_id)?;
                if yes_values.get(&yes.result.0) != Some(&instruction.ty) {
                    return Err("yes branch result type mismatch");
                }
                let no_values =
                    verify_instructions(&no.instructions, params, callees, &values, next_id)?;
                if no_values.get(&no.result.0) != Some(&instruction.ty) {
                    return Err("no branch result type mismatch");
                }
            }
        }
        values.insert(instruction.id.0, instruction.ty.clone());
    }
    Ok(values)
}

fn scalar(ty: &Type) -> bool {
    matches!(ty, Type::I32 | Type::I64 | Type::F64 | Type::Bool)
}

struct Lowerer<'a> {
    params: &'a [(String, Type)],
    types: &'a HashMap<Span, Type>,
    signatures: &'a HashMap<String, Signature>,
    instructions: Vec<Instruction>,
    locals: Vec<HashMap<String, ValueId>>,
    callees: HashMap<String, Signature>,
    next_id: usize,
}

impl Lowerer<'_> {
    fn push(&mut self, kind: InstructionKind, ty: Type, span: Span) -> ValueId {
        let id = ValueId(self.next_id);
        self.next_id += 1;
        self.instructions.push(Instruction { id, kind, ty, span });
        id
    }

    fn region(&mut self, expr: &Expr) -> Option<Region> {
        let outer = std::mem::take(&mut self.instructions);
        let result = self.expression(expr);
        let instructions = std::mem::replace(&mut self.instructions, outer);
        Some(Region {
            instructions,
            result: result?,
        })
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
        if let ExprKind::If(condition, yes, no) = &expr.kind {
            let condition = self.expression(condition)?;
            let id = ValueId(self.next_id);
            self.next_id += 1;
            let yes = self.region(yes)?;
            let no = self.region(no)?;
            self.instructions.push(Instruction {
                id,
                kind: InstructionKind::Conditional { condition, yes, no },
                ty,
                span: expr.span,
            });
            return Some(id);
        }
        if let ExprKind::Binary(left, op, right) = &expr.kind
            && matches!(op, Op::And | Op::Or)
        {
            let condition = self.expression(left)?;
            let id = ValueId(self.next_id);
            self.next_id += 1;
            let shortcut = Region {
                instructions: Vec::new(),
                result: condition,
            };
            let (yes, no) = if *op == Op::And {
                (self.region(right)?, shortcut)
            } else {
                (shortcut, self.region(right)?)
            };
            self.instructions.push(Instruction {
                id,
                kind: InstructionKind::Conditional { condition, yes, no },
                ty,
                span: expr.span,
            });
            return Some(id);
        }
        let kind = match &expr.kind {
            ExprKind::Int(value) => InstructionKind::I32(*value),
            ExprKind::I64(value) => InstructionKind::I64(*value),
            ExprKind::F64(value) => InstructionKind::F64(*value),
            ExprKind::Bool(value) => InstructionKind::Bool(*value),
            ExprKind::Not(value) => InstructionKind::Not(self.expression(value)?),
            ExprKind::Var(name) => {
                if let Some(id) = self.locals.iter().rev().find_map(|scope| scope.get(name)) {
                    return Some(*id);
                }
                InstructionKind::Parameter(self.params.iter().position(|(param, _)| param == name)?)
            }
            ExprKind::Neg(value) => {
                let id = self.expression(value)?;
                if ty == Type::F64 {
                    InstructionKind::FloatNeg(id)
                } else {
                    InstructionKind::CheckedNeg(id)
                }
            }
            ExprKind::Binary(left, op, right) => {
                let left = self.expression(left)?;
                let right = self.expression(right)?;
                InstructionKind::Binary(BinaryOp::try_from(*op).ok()?, left, right)
            }
            ExprKind::Call(name, args) => {
                let signature = self.signatures.get(name)?.clone();
                if signature.params.len() != args.len() || signature.ret != ty {
                    return None;
                }
                let values = args
                    .iter()
                    .map(|arg| self.expression(arg))
                    .collect::<Option<Vec<_>>>()?;
                self.callees.insert(name.clone(), signature);
                InstructionKind::Call(name.clone(), values)
            }
            _ => return None,
        };
        Some(self.push(kind, ty, expr.span))
    }
}

/// Reuse checked scalar signatures while lowering every function in a program.
pub struct LoweringContext {
    signatures: HashMap<String, Signature>,
}

impl LoweringContext {
    pub fn new(program: &ast::Program) -> Self {
        let signatures = program
            .functions
            .iter()
            .filter(|candidate| {
                candidate.type_params.is_empty()
                    && scalar(&candidate.ret)
                    && candidate.params.iter().all(|(_, ty)| scalar(ty))
            })
            .map(|candidate| {
                (
                    candidate.name.clone(),
                    Signature {
                        params: candidate.params.iter().map(|(_, ty)| ty.clone()).collect(),
                        ret: candidate.ret.clone(),
                    },
                )
            })
            .collect();
        Self { signatures }
    }

    /// Lower a checked function when every expression fits the scalar IR slice.
    pub fn lower_function(
        &self,
        function: &ast::Function,
        types: &HashMap<Span, Type>,
    ) -> Option<Function> {
        if !function.type_params.is_empty()
            || !scalar(&function.ret)
            || function.params.iter().any(|(_, ty)| !scalar(ty))
        {
            return None;
        }
        let mut lowerer = Lowerer {
            params: &function.params,
            types,
            signatures: &self.signatures,
            instructions: Vec::new(),
            locals: Vec::new(),
            callees: HashMap::new(),
            next_id: 0,
        };
        let result = lowerer.expression(&function.body)?;
        let lowered = Function {
            name: function.name.clone(),
            params: function.params.clone(),
            ret: function.ret.clone(),
            span: function.span,
            callees: lowerer.callees,
            instructions: lowerer.instructions,
            result,
        };
        lowered.verify().ok()?;
        Some(lowered)
    }
}

/// Convenience wrapper for lowering one function.
pub fn lower_function(
    function: &ast::Function,
    program: &ast::Program,
    types: &HashMap<Span, Type>,
) -> Option<Function> {
    LoweringContext::new(program).lower_function(function, types)
}
