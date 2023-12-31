use std::collections::{
    hash_map::Entry,
    HashMap,
};

use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::compiler::ir::{
    arena::{BBId, FuncId, GlobalId, InstId, ParamId},
    value::{
        constant::Constant,
        func::IrFunc,
        global::Global,
        inst::InstKind,
        module::Module,
        ty::{FuncTy, IrTy},
        value::{Operand, Value},
    },
};
use crate::compiler::syntax::ast::{AstTy, LiteralExpr, LiteralKind};

#[derive(Debug, Clone, Copy, EnumAsInner)]
pub enum IdInfo {
    Inst(InstId),
    Func(FuncId),
    Global(GlobalId),
    Param(ParamId),
}

impl From<IdInfo> for Operand {
    fn from(name_id: IdInfo) -> Self {
        match name_id {
            IdInfo::Inst(x) => Operand::Inst(x),
            IdInfo::Global(x) => Operand::Global(x),
            IdInfo::Param(x) => Operand::Param(x),
            _ => unreachable!()
        }
    }
}

#[derive(Debug)]
pub struct Scope<T> {
    vars: HashMap<String, T>,
}

impl<T> Scope<T> {
    pub fn new() -> Scope<T> {
        Scope { vars: HashMap::new() }
    }

    pub fn find(&self, name: &str) -> Option<&T> {
        self.vars.get(name)
    }

    pub fn insert(&mut self, name: String, value: T) -> Option<&T> {
        let entry = self.vars.entry(name);
        match entry {
            Entry::Occupied(_) => None,
            Entry::Vacant(e) => Some(e.insert(value))
        }
    }
}

#[derive(Debug)]
pub struct ScopeBuilder<T> {
    scopes: Vec<Scope<T>>,
}

impl<T> ScopeBuilder<T> {
    pub fn new() -> ScopeBuilder<T> {
        ScopeBuilder {
            scopes: vec![]
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn find_name_rec(&self, name: &str) -> Option<&T> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.find(name) {
                return Some(value)
            }
        }
        None
    }

    pub fn insert(&mut self, name: &str, id: T) -> Option<&T> {
        let name = String::from(name);
        self.scopes.last_mut().expect("No scope found")
            .insert(name, id)
    }
}

#[derive(Debug)]
pub struct Context {
    pub scope_builder: ScopeBuilder<IdInfo>,
    pub cur_module: Module,
    cur_func: FuncId,
    cur_bb: BBId,
}

impl Context {
    pub fn new() -> Context {
        Context {
            scope_builder: ScopeBuilder::new(),
            cur_module: Module::new(),
            cur_func: FuncId::default(),
            cur_bb: BBId::default(),
        }
    }

    pub fn get_cur_bb_id(&self) -> BBId {
        self.cur_bb
    }

    fn get_cur_func_mut(&mut self) -> &mut IrFunc {
        self.cur_module.get_func_mut(self.cur_func).unwrap()
    }

    // fn get_cur_bb_mut(&mut self) -> &mut BasicBlock {
    //     let bb = self.cur_bb;
    //     self.get_cur_func_mut().get_bb_mut(bb).unwrap()
    // }

    pub fn get_func_ty(&self, func: FuncId) -> &FuncTy {
        let func_ty = self.cur_module.get_func(func).unwrap().get_ty();
        func_ty.as_func().unwrap()
    }

    pub fn set_cur_bb(&mut self, bb: BBId) {
        self.cur_bb = bb;
    }

    pub fn set_cur_func(&mut self, func: FuncId) {
        self.cur_func = func;
    }

    pub fn build_inst_end(&mut self, inst_kind: InstKind, ty: IrTy, bb: BBId) -> InstId {
        self.get_cur_func_mut().build_inst_at_end(inst_kind, ty, bb)
    }

    pub fn build_inst_end_of_cur(&mut self, inst_kind: InstKind, ty: IrTy) -> InstId {
        let cur_bb = self.cur_bb;
        self.build_inst_end(inst_kind, ty, cur_bb)
    }

    pub fn build_bb_after_cur(&mut self) -> BBId {
        let bb = self.cur_bb;
        self.get_cur_func_mut().build_bb_after_cur(bb)
    }

    pub fn build_bb(&mut self) -> BBId {
        self.get_cur_func_mut().build_bb()
    }

    pub fn build_func(&mut self, func: IrFunc) -> FuncId {
        self.cur_module.build_func(func)
    }

    pub fn build_global(&mut self, global: Global) -> GlobalId {
        self.cur_module.build_global(global)
    }

    pub fn build_func_param(&mut self, ty: IrTy) -> ParamId {
        self.get_cur_func_mut().build_func_param(ty)
    }

    pub fn set_bb_after(&mut self, after: BBId, cur: BBId) {
        self.get_cur_func_mut().set_bb_after_cur(after, cur);
    }

    // pub fn get_operand_ty<'a>(&'a self, operand: &'a Operand) -> &'a IrTy {
    //     let cur_func = self.cur_module.get_func(self.cur_func).unwrap();
    //     match operand {
    //         Operand::Inst(inst) => &cur_func.get_inst(*inst).unwrap().ty,
    //         Operand::Global(g) => &self.cur_module.global_arena.get(*g).unwrap().ty,
    //         Operand::Param(p) => &cur_func.get_param(*p).unwrap().ty,
    //         Operand::Const(c) => &c.get_ty(),
    //         Operand::BB(_) => &IrTy::Label
    //     }
    // }
}

#[derive(Debug, Clone)]
pub struct TyInfo {
    pub ty: AstTy,
    pub const_val: Option<LiteralExpr>,
    pub is_const: bool,
}

impl From<AstTy> for IrTy {
    fn from(ast_ty: AstTy) -> Self {
        match ast_ty {
            AstTy::Void => IrTy::Void,
            AstTy::Int => IrTy::Int(32),
            AstTy::Bool => IrTy::Int(1),
            AstTy::Func { ret_ty, param_tys: params } => IrTy::Func(Box::new(
                FuncTy {
                    ret_ty: (*ret_ty).into(),
                    params_ty: params.into_iter().map(std::convert::Into::into).collect(),
                }
            )),
            AstTy::Array { siz, elem_ty } => IrTy::Array(
                siz,
                Box::new(IrTy::from(*elem_ty))),
            AstTy::Ptr(x) => IrTy::Ptr(Box::new(x.as_ref().clone().into())),
            AstTy::Unknown => unreachable!(),
        }
    }
}

impl From<LiteralExpr> for Constant {
    fn from(literal: LiteralExpr) -> Self {
        match literal.kind {
            LiteralKind::Integer(x) => Constant::from(x),
            LiteralKind::Array(_, vals) => {
                let vals = vals.into_iter()
                    .map(Constant::from)
                    .collect_vec();
                Constant::Array {
                    ty: literal.ty.into(),
                    elems: vals
                }
            }
        }
    }
}