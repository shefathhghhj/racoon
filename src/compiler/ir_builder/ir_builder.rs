use itertools::Itertools;

use crate::compiler::ir::{
    arena::{BBId, InstId},
    value::{
        constant::Constant,
        func::IrFunc,
        global::Global,
        inst::{Alloca, Binary, BinaryInstOp, Br, Call, GEP, InstKind, Load, RetInst, Store, ZExt},
        ty::IrTy,
        value::Operand,
    },
};
use crate::compiler::span::Span;
use crate::compiler::syntax::{ast::{AssignExpr, AstFunc, AstTy, BinaryExpr, BlockItem, BlockStmt, CallExpr, Decl, Expr, FuncParam, IfStmt, InitVal, InitValKind, LiteralExpr, LiteralKind, PrimitiveTy, Program, ProgramItem, ReturnStmt, Stmt, Subs, TyIdentKind, TypeIdent, UnaryExpr, UnaryOp, WhileStmt}, visitor::AstVisitor};

use super::{
    context::{Context, IdInfo},
    err::SemanticError,
};

#[derive(Debug, Clone, Copy)]
pub struct BCTarget {
    pub break_target: BBId,
    pub continue_target: BBId,
}

#[derive(Debug)]
pub struct IrBuilder {
    pub ctx: Context,
    loop_targets: Vec<BCTarget>,
}

impl IrBuilder {
    #[must_use] pub fn new() -> IrBuilder {
        IrBuilder {
            ctx: Context::new(),
            loop_targets: vec![],
        }
    }

    pub fn visit(&mut self, program: &Program) -> Result<(), SemanticError> {
        self.visit_program(program)?;
        Ok(())
    }

    fn push_break_target(&mut self, break_target: BBId, continue_target: BBId) {
        self.loop_targets.push(BCTarget { break_target, continue_target });
    }

    fn pop_loop_target(&mut self) {
        self.loop_targets.pop();
    }

    fn get_break_target(&self) -> Option<BBId> {
        Some(self.loop_targets.last()?.break_target)
    }

    fn get_continue_target(&self) -> Option<BBId> {
        Some(self.loop_targets.last()?.continue_target)
    }

    // fn set_bb_after(&mut self, after: BBId, cur: BBId) {
    //     self.ctx.set_bb_after(after, cur);
    // }

    fn build_decl_init_val(&mut self, init_val: &InitVal, base_addr: InstId) -> Result<(), SemanticError> {
        match &init_val.kind {
            InitValKind::Expr(expr) => {
                let init_expr_id = self.visit_expr(expr)?;

                let store_inst = Store {
                    addr: base_addr.into(),
                    data: init_expr_id,
                };
                self.ctx.build_inst_end_of_cur(InstKind::Store(store_inst), IrTy::Void);
            }
            InitValKind::ArrayVal(array_vals) => {
                array_vals.iter().enumerate()
                    .try_for_each(|(idx, val)| {
                        let ir_ty = IrTy::from(init_val.ty.clone());
                        let ty = ir_ty.as_array().unwrap().1.as_ref();

                        let gep_inst = GEP {
                            ptr: base_addr.into(),
                            indices: vec![0.into(), (idx as i32).into()]
                        };
                        let gep_inst_id = self.ctx.build_inst_end_of_cur(
                            InstKind::GEP(gep_inst),
                            IrTy::ptr_of(ty)
                        );
                        Self::build_decl_init_val(self, val, gep_inst_id)
                    })?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn push_built_in_funcs(&mut self) {
        // getint
        let func_getint = IrFunc::new("getint", IrTy::Int(32), true);
        let func_getint_id = self.ctx.cur_module.build_func(func_getint);
        self.ctx.scope_builder.insert("getint", IdInfo::Func(func_getint_id));

        // getch
        let func_getch = IrFunc::new("getch", IrTy::Int(32), true);
        let func_getch_id = self.ctx.cur_module.build_func(func_getch);
        self.ctx.scope_builder.insert("getch", IdInfo::Func(func_getch_id));

        // getarray
        let mut func_getarray = IrFunc::new("getarray", IrTy::Int(32), true);
        func_getarray.build_func_param(IrTy::Ptr(Box::from(IrTy::Int(32))));
        let func_getarray_id = self.ctx.cur_module.build_func(func_getarray);
        self.ctx.scope_builder.insert("getarray", IdInfo::Func(func_getarray_id));

        // putint
        let mut func_putint = IrFunc::new("putint", IrTy::Void, true);
        func_putint.build_func_param(IrTy::Int(32));
        let func_putint_id = self.ctx.cur_module.build_func(func_putint);
        self.ctx.scope_builder.insert("putint", IdInfo::Func(func_putint_id));

        // putch
        let mut func_putch = IrFunc::new("putch", IrTy::Void, true);
        func_putch.build_func_param(IrTy::Int(32));
        let func_putch_id = self.ctx.cur_module.build_func(func_putch);
        self.ctx.scope_builder.insert("putch", IdInfo::Func(func_putch_id));

        // putarray
        let mut func_putarray = IrFunc::new("putarray", IrTy::Int(32), true);
        func_putarray.build_func_param(IrTy::Int(32));
        func_putarray.build_func_param(IrTy::Ptr(Box::from(IrTy::Int(32))));
        let func_putarray_id = self.ctx.cur_module.build_func(func_putarray);
        self.ctx.scope_builder.insert("putarray", IdInfo::Func(func_putarray_id));
    }
}

impl AstVisitor for IrBuilder {
    type ProgramResult = Result<(), SemanticError>;
    type ConstInitValResult = Result<Constant, SemanticError>;
    type FuncResult = Result<(), SemanticError>;
    type StmtResult = Result<(), SemanticError>;
    type ExprResult = Result<Operand, SemanticError>;
    type LExprResult = Result<Operand, SemanticError>;
    type TyResult = Result<IrTy, SemanticError>;

    fn visit_program(&mut self, program: &Program) -> Self::ProgramResult {
        self.ctx.scope_builder.push_scope();

        self.push_built_in_funcs();

        program.program_items.iter()
            .try_for_each(|item| match item {
                ProgramItem::Decl(x) => self.visit_global_decl(x),
                ProgramItem::Func(x) => self.visit_func(x),
            })?;
        self.ctx.scope_builder.pop_scope();
        Ok(())
    }

    fn visit_const_init_val(&mut self, init_val: &InitVal) -> Self::ConstInitValResult {
        Ok(init_val.kind.as_const().unwrap().clone().into())
    }

    fn visit_global_decl(&mut self, decl: &Decl) -> Self::StmtResult {
        for sub_decl in &decl.sub_decls {
            let ty = IrTy::from(sub_decl.ty.clone());
            let ptr_ty = IrTy::ptr_of(&ty);

            // visit initial value
            let const_init_val = if let Some(init_val) = &sub_decl.init_val {
                self.visit_const_init_val(init_val)?
            } else {
                Constant::build_zero(&ty)
            };

            let global = Global::new(
                ptr_ty,
                &sub_decl.ident.name,
                const_init_val,
            );
            let global_id = self.ctx.build_global(global);

            self.ctx.scope_builder.insert(&sub_decl.ident.name, IdInfo::Global(global_id));
        }

        Ok(())
    }

    fn visit_func(&mut self, ast_func: &AstFunc) -> Self::FuncResult {
        let ret_ty = self.visit_ty(&ast_func.ret_ty_ident)?;

        let func = IrFunc::new(
            &ast_func.ident.name,
            ret_ty.clone(),
            false,
        );
        let func_id = self.ctx.build_func(func);
        self.ctx.scope_builder.insert(&ast_func.ident.name, IdInfo::Func(func_id));
        self.ctx.set_cur_func(func_id);

        self.ctx.scope_builder.push_scope();

        // build bb
        let init_bb_id = self.ctx.build_bb();
        self.ctx.set_cur_bb(init_bb_id);

        // visit params
        ast_func.params.iter()
            .try_for_each(|param| self.visit_func_param(param))?;

        self.visit_block_stmt(&ast_func.body)?;

        // add default return inst
        let ret_inst = match &ret_ty {
            IrTy::Void => RetInst { val: None },
            IrTy::Int(_) => RetInst { val: Some(0.into()) },
            _ => unreachable!()
        };
        self.ctx.build_inst_end_of_cur(InstKind::RetInst(ret_inst), IrTy::Void);

        self.ctx.scope_builder.pop_scope();

        Ok(())
    }

    fn visit_func_param(&mut self, param: &FuncParam) -> Self::StmtResult {
        let ty = IrTy::from(param.ty.clone());
        let param_id = self.ctx.build_func_param(ty.clone());

        if let IrTy::Ptr(_) = ty {
            self.ctx.scope_builder.insert(&param.ident.name, IdInfo::Param(param_id));
        } else {
            let alloca_inst = Alloca { alloca_ty: ty.clone() };
            let alloca_addr = self.ctx.build_inst_end_of_cur(
                InstKind::Alloca(alloca_inst),
                IrTy::ptr_of(&ty),
            );

            self.ctx.scope_builder.insert(&param.ident.name, IdInfo::Inst(alloca_addr));

            let store_inst = Store {
                addr: alloca_addr.into(),
                data: param_id.into(),
            };
            self.ctx.build_inst_end_of_cur(InstKind::Store(store_inst), IrTy::Void);
        }

        Ok(())
    }

    fn visit_block_stmt(&mut self, stmt: &BlockStmt) -> Self::StmtResult {
        self.ctx.scope_builder.push_scope();
        stmt.block_items.iter()
            .try_for_each(|sub_stmt| match sub_stmt {
                BlockItem::Stmt(x) => self.visit_stmt(x),
                BlockItem::Decl(x) => self.visit_decl_stmt(x),
            })?;
        self.ctx.scope_builder.pop_scope();

        Ok(())
    }

    fn visit_stmt(&mut self, stmt: &Stmt) -> Self::StmtResult {
        match stmt {
            Stmt::Expr(x) => self.visit_expr_stmt(x),
            Stmt::Block(x) => self.visit_block_stmt(x),
            Stmt::If(x) => self.visit_if_stmt(x),
            Stmt::While(x) => self.visit_while_stmt(x),
            Stmt::Break(x) => self.visit_break_stmt(*x),
            Stmt::Continue(x) => self.visit_continue_stmt(*x),
            Stmt::Return(x) => self.visit_return_stmt(x),
            Stmt::Empty(x) => self.visit_empty_stmt(*x),
        }
    }

    fn visit_init_val(&mut self, _init_val: &InitVal) -> Self::StmtResult {
        Ok(())
    }

    fn visit_decl_stmt(&mut self, decl: &Decl) -> Self::StmtResult {
        for sub_decl in &decl.sub_decls {
            let ty = IrTy::from(sub_decl.ty.clone());

            let alloca_inst = Alloca { alloca_ty: ty.clone() };
            let alloca_addr = self.ctx.build_inst_end_of_cur(
                InstKind::Alloca(alloca_inst),
                IrTy::ptr_of(&ty),
            );
            self.ctx.scope_builder.insert(&sub_decl.ident.name, IdInfo::Inst(alloca_addr));

            if let Some(init_val) = &sub_decl.init_val {
                self.build_decl_init_val(init_val, alloca_addr)?;
            }
        }
        Ok(())
    }

    fn visit_expr_stmt(&mut self, stmt: &Expr) -> Self::StmtResult {
        self.visit_expr(stmt)?;
        Ok(())
    }

    fn visit_if_stmt(&mut self, stmt: &IfStmt) -> Self::StmtResult {
        let cond = self.visit_expr(&stmt.cond)?;
        let old_bb = self.ctx.get_cur_bb_id();

        // then blk
        let then_bb = self.ctx.build_bb_after_cur();
        self.ctx.set_cur_bb(then_bb);
        self.visit_stmt(&stmt.then_block)?;
        let then_bb_end = self.ctx.get_cur_bb_id();

        // else bb
        let (else_bb, else_bb_end) =
            if let Some(else_blk) = &stmt.else_block {
                let else_bb = self.ctx.build_bb_after_cur();
                self.ctx.set_cur_bb(else_bb);
                self.visit_stmt(else_blk)?;
                (Some(else_bb), Some(self.ctx.get_cur_bb_id()))
            } else {
                (None, None)
            };

        // nxt bb
        let nxt_bb = self.ctx.build_bb_after_cur();

        let cond_br_inst = Br::Br {
            cond,
            true_bb: then_bb,
            false_bb: else_bb.unwrap_or(nxt_bb),
        };
        self.ctx.build_inst_end(
            InstKind::Br(cond_br_inst),
            IrTy::Void,
            old_bb);

        let then_br_inst = Br::Jump { nxt_bb };
        self.ctx.build_inst_end(
            InstKind::Br(then_br_inst),
            IrTy::Void,
            then_bb_end);

        if let Some(else_bb_end) = else_bb_end {
            let else_br_inst = Br::Jump { nxt_bb };
            self.ctx.build_inst_end(
                InstKind::Br(else_br_inst),
                IrTy::Void,
                else_bb_end);
        }

        self.ctx.set_cur_bb(nxt_bb);

        Ok(())
    }

    fn visit_while_stmt(&mut self, stmt: &WhileStmt) -> Self::StmtResult {
        let old_bb = self.ctx.get_cur_bb_id();

        // cond_bb
        let cond_bb = self.ctx.build_bb_after_cur();
        self.ctx.set_cur_bb(cond_bb);
        let cond = self.visit_expr(&stmt.cond)?;

        // loop_bb, nxt_bb
        let loop_bb = self.ctx.build_bb_after_cur();
        let nxt_bb = self.ctx.build_bb_after_cur();

        // visit loop_bb
        self.ctx.set_cur_bb(loop_bb);
        self.push_break_target(nxt_bb, cond_bb);
        self.visit_stmt(&stmt.body)?;
        self.pop_loop_target();
        let loop_end_bb = self.ctx.get_cur_bb_id();

        let old_bb_br_inst = Br::Jump { nxt_bb: cond_bb };
        self.ctx.build_inst_end(
            InstKind::Br(old_bb_br_inst),
            IrTy::Void,
            old_bb);

        let loop_cond_br_inst = Br::Br {
            cond,
            true_bb: loop_bb,
            false_bb: nxt_bb,
        };
        self.ctx.build_inst_end(
            InstKind::Br(loop_cond_br_inst),
            IrTy::Void,
            cond_bb);

        let loop_body_br_inst = Br::Jump { nxt_bb: cond_bb };
        self.ctx.build_inst_end(
            InstKind::Br(loop_body_br_inst),
            IrTy::Void,
            loop_end_bb);

        self.ctx.set_cur_bb(nxt_bb);
        Ok(())
    }

    fn visit_break_stmt(&mut self, _span: Span) -> Self::StmtResult {
        let break_target = self.get_break_target()
            .ok_or(SemanticError::BreakOutsideLoop)?;

        let break_br_inst = Br::Jump { nxt_bb: break_target };
        self.ctx.build_inst_end_of_cur(
            InstKind::Br(break_br_inst),
            IrTy::Void);

        let nxt_bb = self.ctx.build_bb_after_cur();
        self.ctx.set_cur_bb(nxt_bb);

        Ok(())
    }

    fn visit_continue_stmt(&mut self, _span: Span) -> Self::StmtResult {
        let continue_target = self.get_continue_target()
            .ok_or(SemanticError::ContinueOutsideLoop)?;

        let continue_br_inst = Br::Jump { nxt_bb: continue_target };
        self.ctx.build_inst_end_of_cur(
            InstKind::Br(continue_br_inst),
            IrTy::Void);

        let nxt_bb = self.ctx.build_bb_after_cur();
        self.ctx.set_cur_bb(nxt_bb);

        Ok(())
    }

    fn visit_return_stmt(&mut self, stmt: &ReturnStmt) -> Self::StmtResult {
        let val = stmt.val.as_ref()
            .map(|val| self.visit_expr(val))
            .map_or(Ok(None), |r| r.map(Some))?;

        let ret_inst = RetInst { val };
        self.ctx.build_inst_end_of_cur(InstKind::RetInst(ret_inst), IrTy::Void);

        let nxt_bb = self.ctx.build_bb_after_cur();
        self.ctx.set_cur_bb(nxt_bb);

        Ok(())
    }

    fn visit_empty_stmt(&mut self, _span: Span) -> Self::StmtResult {
        Ok(())
    }

    fn visit_expr(&mut self, expr: &Expr) -> Self::ExprResult {
        match expr {
            Expr::LVal(_) => self.visit_lexpr(expr, false),
            Expr::Assign(x) => self.visit_assign_expr(x),
            Expr::Literal(x) => self.visit_literal_expr(x),
            Expr::Unary(x) => self.visit_unary_expr(x),
            Expr::Binary(x) => self.visit_binary_expr(x),
            Expr::Call(x) => self.visit_call_expr(x),
        }
    }

    fn visit_lexpr(&mut self, expr: &Expr, is_lvalue: bool) -> Self::LExprResult {
        let lval = expr.as_l_val().unwrap();
        let ty = lval.ty.clone().into();
        let mut addr = Operand::from(
            *self.ctx.scope_builder
                .find_name_rec(&lval.ident.name)
                .unwrap());

        if let Some(Subs { subs, .. }) = &lval.subs {
            let mut indices = vec![];

            if !matches!(addr, Operand::Param(_)) {
                indices.push(0.into());
            }

            for sub in subs {
                let idx = self.visit_expr(sub)?;
                indices.push(idx);
            }

            let gep_inst = GEP {
                ptr: addr,
                indices,
            };
            let gep = self.ctx.build_inst_end_of_cur(InstKind::GEP(gep_inst), IrTy::ptr_of(&ty));
            addr = gep.into();
        }

        if is_lvalue || matches!(lval.ty, AstTy::Array { .. }) {
            Ok(addr)
        } else {
            let val_id = self.ctx.build_inst_end_of_cur(
                InstKind::Load(Load { addr }),
                ty);

            Ok(val_id.into())
        }
    }

    fn visit_assign_expr(&mut self, expr: &AssignExpr) -> Self::ExprResult {
        let lval = self.visit_lexpr(&expr.lhs, true)?;
        let rval = self.visit_expr(&expr.rhs)?;
        let store_inst = Store {
            addr: lval,
            data: rval.clone()
        };
        self.ctx.build_inst_end_of_cur(InstKind::Store(store_inst), IrTy::Void);
        Ok(rval)
    }

    fn visit_literal_expr(&mut self, expr: &LiteralExpr) -> Self::ExprResult {
        let constant = match expr.kind {
            LiteralKind::Integer(i) => Operand::Const(Constant::Int(i)),
            _ => unreachable!()
        };
        Ok(constant)
    }

    fn visit_unary_expr(&mut self, expr: &UnaryExpr) -> Self::ExprResult {
        let mut val = self.visit_expr(&expr.sub_expr)?;
        match expr.op {
            UnaryOp::Neg => {
                let inst = Binary {
                    op: BinaryInstOp::Sub,
                    left: 0.into(),
                    right: val,
                };
                let sub_inst_id = self.ctx.build_inst_end_of_cur(InstKind::Binary(inst), expr.ty.clone().into());

                Ok(sub_inst_id.into())
            }
            UnaryOp::Pos => Ok(val),
            UnaryOp::Not => {
                if let AstTy::Bool = expr.sub_expr.ty() {
                    let zext_inst = ZExt {
                        ori_val: val,
                        target_ty: IrTy::int(),
                    };
                    let id = self.ctx.build_inst_end_of_cur(InstKind::ZExt(zext_inst), IrTy::int());
                    val = id.into();
                }

                let inst = Binary {
                    op: BinaryInstOp::Ne,
                    left: val,
                    right: 0.into(),
                };
                let not_inst_id = self.ctx.build_inst_end_of_cur(InstKind::Binary(inst), IrTy::bool());

                Ok(not_inst_id.into())
            }
        }
    }

    fn visit_binary_expr(&mut self, expr: &BinaryExpr) -> Self::ExprResult {
        let left = self.visit_expr(&expr.lhs)?;
        let right = self.visit_expr(&expr.rhs)?;
        let op = expr.op.to_binary_inst_kind();

        let binary_inst = Binary { op, left, right };
        let binary_inst_id = self.ctx.build_inst_end_of_cur(InstKind::Binary(binary_inst), expr.ty.clone().into());

        Ok(binary_inst_id.into())
    }

    fn visit_call_expr(&mut self, expr: &CallExpr) -> Self::ExprResult {
        let func_id = *self.ctx.scope_builder.find_name_rec(&expr.func.name).unwrap()
            .as_func().unwrap();

        let args = expr.args.iter()
            .map(|x| {
                let expr_id = self.visit_expr(x)?;
                match x.ty() {
                    AstTy::Int | AstTy::Bool | AstTy::Ptr(_) => Ok(expr_id),
                    AstTy::Array { elem_ty, .. } => {
                        let gep_inst = GEP {
                            ptr: expr_id,
                            // convert array type to ptr
                            indices: vec![0.into(), 0.into()],
                        };
                        let gep_id = self.ctx.build_inst_end_of_cur(
                            InstKind::GEP(gep_inst),
                            IrTy::ptr_of(&elem_ty.as_ref().clone().into())
                        );
                        Ok(Operand::Inst(gep_id))
                    }
                    _ => unreachable!()
                }

            })
            .try_collect()?;

        let ret_ty = self.ctx.get_func_ty(func_id).ret_ty.clone();

        let call_inst = Call { func_id, args };
        let call_inst_id = self.ctx.build_inst_end_of_cur(InstKind::Call(call_inst), ret_ty);

        Ok(call_inst_id.into())
    }

    fn visit_ty(&mut self, ty_def: &TypeIdent) -> Self::TyResult {
        let ty = match &ty_def.kind {
            TyIdentKind::Primitive(prim_ty) => match prim_ty {
                PrimitiveTy::Integer => IrTy::Int(32)
            }
            TyIdentKind::Void => IrTy::Void
        };

        Ok(ty)
    }
}
