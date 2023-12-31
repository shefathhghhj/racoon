use itertools::Itertools;

use crate::compiler::span::Span;
use crate::compiler::syntax::ast::{AssignExpr, AstFunc, AstTy, BinaryExpr, BinaryOp, BlockItem, BlockStmt, CallExpr, Decl, Expr, FuncParam, IfStmt, InitVal, InitValKind, LiteralExpr, LiteralKind, PrimitiveTy, Program, ProgramItem, ReturnStmt, Stmt, Subs, TyIdentKind, TypeIdent, UnaryExpr, UnaryOp, WhileStmt};
use crate::compiler::syntax::visitor::AstVisitorMut;

use super::{
    context::{ScopeBuilder, TyInfo},
    err::SemanticError::{self, TypeMismatch},
};

macro_rules! expect_type {
    ($self:expr, $pat:pat) => {{
        if matches!($self, $pat) {
            Ok(())
        } else {
            Err(TypeMismatch {
                expected: String::from(stringify!($pat)),
                found: $self
            })
        }
    }};
}

#[derive(Debug)]
pub struct TypeChecker {
    pub scopes: ScopeBuilder<TyInfo>,
    pub cur_func_ret_ty: AstTy,
}

impl TypeChecker {
    #[must_use] pub fn new() -> TypeChecker {
        TypeChecker {
            scopes: ScopeBuilder::new(),
            cur_func_ret_ty: AstTy::Unknown,
        }
    }
}

impl TypeChecker {
    fn fix_array_literal(literal: &mut LiteralExpr, expected_ty: &AstTy) -> Result<(), SemanticError> {
        match (&mut literal.kind, expected_ty) {
            (LiteralKind::Integer(_), AstTy::Int) => Ok(()),
            (LiteralKind::Array(literal_siz, literal_vals),
                AstTy::Array { siz: ty_siz, elem_ty })
            => {
                if *literal_siz > *ty_siz {
                    return Err(SemanticError::TooMuchElement);
                }

                literal_vals.iter_mut()
                    .try_for_each(|x| Self::fix_array_literal(x, elem_ty))?;

                // fix array literal
                *literal_siz = *ty_siz;
                literal.ty = expected_ty.clone();
                Ok(())
            }
            _ => assert_type_eq(expected_ty, &AstTy::Unknown)
        }
    }

    fn fix_array_init_val(init_val: &mut InitVal, expected_ty: &AstTy) -> Result<(), SemanticError> {
        match (&mut init_val.kind, expected_ty) {
            (InitValKind::Expr(expr), _) => assert_type_eq(expected_ty, &expr.ty()),
            (InitValKind::ArrayVal(elem_vals),
                AstTy::Array { siz: ty_siz, elem_ty })
            => {
                if elem_vals.len() > *ty_siz {
                    return Err(SemanticError::TooMuchElement);
                }

                elem_vals.iter_mut()
                    .try_for_each(|x| Self::fix_array_init_val(x, elem_ty))?;

                // fix array init_val
                init_val.ty = expected_ty.clone();
                Ok(())
            }
            _ => assert_type_eq(expected_ty, &AstTy::Unknown)
        }
    }

    fn build_ast_ty(&mut self, base_ty: &AstTy, subs: &mut Option<Subs>) -> Result<AstTy, SemanticError> {
        if subs.is_some() {
            for sub in &mut subs.as_mut().unwrap().subs {
                let literal = self.visit_expr(sub)?
                    .ok_or(SemanticError::RequireConstant)?;

                if literal.get_int().unwrap() <= 0 {
                    return Err(SemanticError::IllegalArrayDim);
                }
                *sub = Expr::Literal(literal);
            }
        }
        let mut ty = base_ty.clone();
        if let Some(subs) = &subs {
            for sub in subs.subs.iter().rev() {
                ty = AstTy::Array {
                    siz: sub.as_literal().unwrap().get_int().unwrap() as usize,
                    elem_ty: Box::new(ty.clone()),
                };
            }
        }
        Ok(ty)
    }

    fn push_built_in_funcs(&mut self) {
        // getint
        self.scopes.insert(
            "getint",
            TyInfo {
                ty: AstTy::Func { ret_ty: Box::from(AstTy::Int), param_tys: vec![] },
                const_val: None,
                is_const: false,
            });


        // getch
        self.scopes.insert(
            "getch",
            TyInfo {
                ty: AstTy::Func { ret_ty: Box::from(AstTy::Int), param_tys: vec![] },
                const_val: None,
                is_const: false,
            });

        // getarray
        self.scopes.insert(
            "getarray",
            TyInfo {
                ty: AstTy::Func { ret_ty: Box::from(AstTy::Int), param_tys: vec![AstTy::Ptr(Box::new(AstTy::Int))] },
                const_val: None,
                is_const: false,
            });

        // putint
        self.scopes.insert(
            "putint",
            TyInfo {
                ty: AstTy::Func { ret_ty: Box::from(AstTy::Void), param_tys: vec![AstTy::Int] },
                const_val: None,
                is_const: false,
            });

        // putch
        self.scopes.insert(
            "putch",
            TyInfo {
                ty: AstTy::Func { ret_ty: Box::from(AstTy::Void), param_tys: vec![AstTy::Int] },
                const_val: None,
                is_const: false,
            });

        // putarray
        self.scopes.insert(
            "putarray",
            TyInfo {
                ty: AstTy::Func {
                    ret_ty: Box::from(AstTy::Int),
                    param_tys: vec![AstTy::Int, AstTy::Ptr(Box::new(AstTy::Int))] },
                const_val: None,
                is_const: false,
            });
    }
}

impl AstVisitorMut for TypeChecker {
    type ProgramResult = Result<(), SemanticError>;
    type ConstInitValResult = Result<LiteralExpr, SemanticError>;
    type FuncResult = Result<(), SemanticError>;
    type StmtResult = Result<(), SemanticError>;
    type ExprResult = Result<Option<LiteralExpr>, SemanticError>;
    type LExprResult = Result<Option<LiteralExpr>, SemanticError>;
    type TyResult = Result<AstTy, SemanticError>;

    fn visit_program(&mut self, program: &mut Program) -> Self::ProgramResult {
        self.scopes.push_scope();

        self.push_built_in_funcs();

        program.program_items.iter_mut().try_for_each(|item| {
            match item {
                ProgramItem::Decl(x) => self.visit_global_decl(x),
                ProgramItem::Func(x) => self.visit_func(x),
            }
        })?;
        self.scopes.pop_scope();
        Ok(())
    }

    fn visit_const_init_val(&mut self, init_val: &mut InitVal) -> Self::ConstInitValResult {
        match &mut init_val.kind {
            InitValKind::Expr(x) => {
                let literal = match self.visit_expr(x)? {
                    Some(x) => Ok(x),
                    None => Err(SemanticError::RequireConstant),
                };
                init_val.ty = x.ty();
                literal
            }
            InitValKind::ArrayVal(vals) => {
                let literals: Vec<_> = vals.iter_mut()
                    .map(|x| self.visit_const_init_val(x))
                    .try_collect()?;
                Ok(LiteralExpr {
                    kind: LiteralKind::Array(literals.len(), literals),
                    span: init_val.span,
                    ty: AstTy::Unknown,
                })
            }
            _ => unreachable!()
        }
    }

    fn visit_global_decl(&mut self, decl: &mut Decl) -> Self::StmtResult {
        let ty = self.visit_ty(&mut decl.ty_ident)?;

        for sub_decl in &mut decl.sub_decls {
            let ty = self.build_ast_ty(&ty, &mut sub_decl.subs)?;

            let init_val = if let Some(init_val) = &mut sub_decl.init_val {
                let mut literal = self.visit_const_init_val(init_val)?;
                match ty {
                    AstTy::Int => assert_type_eq(&ty, &literal.ty)?,
                    AstTy::Array { .. } => Self::fix_array_literal(&mut literal, &ty)?,
                    _ => unreachable!()
                };
                init_val.kind = InitValKind::Const(literal.clone());
                Some(literal)
            } else {
                None
            };

            if decl.is_const && init_val.is_none() {
                return Err(SemanticError::RequireConstant);
            }

            sub_decl.ty = ty.clone();

            let info = TyInfo {
                ty,
                const_val: if decl.is_const { init_val } else { None },
                is_const: decl.is_const,
            };
            self.scopes.insert(&sub_decl.ident.name, info);
        }
        Ok(())
    }

    fn visit_func(&mut self, ast_func: &mut AstFunc) -> Self::FuncResult {
        let ret_ty = self.visit_ty(&mut ast_func.ret_ty_ident)?;
        self.cur_func_ret_ty = ret_ty.clone();

        ast_func.params.iter_mut()
            .try_for_each(|param| self.visit_func_param(param))?;
        let param_tys = ast_func.params.iter()
            .map(|x| x.ty.clone())
            .collect();

        let func_ty = AstTy::Func { ret_ty: Box::new(ret_ty), param_tys };
        let func_info = TyInfo {
            ty: func_ty,
            const_val: None,
            is_const: false,
        };
        self.scopes.insert(&ast_func.ident.name, func_info)
            .ok_or(SemanticError::DuplicateName(ast_func.ident.name.clone()))?;

        self.scopes.push_scope();
        for param in &ast_func.params {
            let param_info = TyInfo {
                ty: param.ty.clone(),
                const_val: None,
                is_const: false,
            };
            self.scopes.insert(&param.ident.name, param_info)
                .ok_or(SemanticError::DuplicateName(param.ident.name.clone()))?;
        }

        self.visit_block_stmt(&mut ast_func.body)?;
        self.scopes.pop_scope();
        Ok(())
    }

    fn visit_func_param(&mut self, param: &mut FuncParam) -> Self::StmtResult {
        let base_ty = self.visit_ty(&mut param.ty_ident)?;
        let mut ty = self.build_ast_ty(&base_ty, &mut param.subs)?;
        if param.subs.is_some() {
            ty = AstTy::Ptr(Box::new(ty));
        }
        param.ty = ty;
        Ok(())
    }

    fn visit_block_stmt(&mut self, stmt: &mut BlockStmt) -> Self::StmtResult {
        self.scopes.push_scope();
        stmt.block_items.iter_mut().try_for_each(|sub_stmt| match sub_stmt {
            BlockItem::Stmt(x) => self.visit_stmt(x),
            BlockItem::Decl(x) => self.visit_decl_stmt(x),
        })?;
        self.scopes.pop_scope();
        Ok(())
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) -> Self::StmtResult {
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

    fn visit_init_val(&mut self, init_val: &mut InitVal) -> Self::StmtResult {
        match &mut init_val.kind {
            InitValKind::Expr(expr) => {
                let literal = self.visit_expr(expr)?;
                init_val.ty = expr.ty();
                if let Some(literal) = literal {
                    init_val.kind = InitValKind::Expr(Expr::Literal(literal));
                }
            }
            InitValKind::ArrayVal(vals) => {
                vals.iter_mut()
                    .try_for_each(|x| self.visit_init_val(x))?;
            }
            _ => unreachable!()
        }
        Ok(())
    }

    fn visit_decl_stmt(&mut self, decl: &mut Decl) -> Self::StmtResult {
        let base_ty = self.visit_ty(&mut decl.ty_ident)?;

        for sub_decl in &mut decl.sub_decls {
            let ty = self.build_ast_ty(&base_ty, &mut sub_decl.subs)?;

            if let Some(init_val) = &mut sub_decl.init_val {
                self.visit_init_val(init_val)?;
                match ty {
                    AstTy::Int => assert_type_eq(&ty, &init_val.ty)?,
                    AstTy::Array { .. } => Self::fix_array_init_val(init_val, &ty)?,
                    _ => unreachable!()
                };
            }

            if decl.is_const && sub_decl.init_val.is_none() {
                return Err(SemanticError::RequireConstant);
            }

            sub_decl.ty = ty.clone();

            let ty_info = TyInfo {
                ty,
                const_val: None,
                is_const: decl.is_const,
            };
            self.scopes.insert(&sub_decl.ident.name, ty_info);
        }
        Ok(())
    }

    fn visit_expr_stmt(&mut self, stmt: &mut Expr) -> Self::StmtResult {
        self.visit_expr(stmt)?;
        Ok(())
    }

    fn visit_if_stmt(&mut self, stmt: &mut IfStmt) -> Self::StmtResult {
        self.visit_expr(&mut stmt.cond)?;
        expect_type!(stmt.cond.ty(), AstTy::Bool)?;
        self.visit_stmt(&mut stmt.then_block)?;
        if let Some(else_blk) = &mut stmt.else_block {
            self.visit_stmt(else_blk)?;
        }
        Ok(())
    }

    fn visit_while_stmt(&mut self, stmt: &mut WhileStmt) -> Self::StmtResult {
        self.visit_expr(&mut stmt.cond)?;
        expect_type!(stmt.cond.ty(), AstTy::Bool)?;
        self.visit_stmt(&mut stmt.body)?;
        Ok(())
    }

    fn visit_break_stmt(&mut self, _span: Span) -> Self::StmtResult {
        Ok(())
    }

    fn visit_continue_stmt(&mut self, _span: Span) -> Self::StmtResult {
        Ok(())
    }

    fn visit_return_stmt(&mut self, stmt: &mut ReturnStmt) -> Self::StmtResult {
        let ret_val_ty = match &mut stmt.val {
            Some(expr) => {
                self.visit_expr(expr.as_mut())?;
                expr.ty()
            }
            None => AstTy::Void,
        };

        assert_type_eq(&ret_val_ty, &self.cur_func_ret_ty)?;
        Ok(())
    }

    fn visit_empty_stmt(&mut self, _span: Span) -> Self::StmtResult {
        Ok(())
    }

    fn visit_expr(&mut self, expr: &mut Expr) -> Self::ExprResult {
        match expr {
            Expr::LVal(_) => self.visit_lexpr(expr, false),
            Expr::Assign(x) => self.visit_assign_expr(x),
            Expr::Literal(x) => self.visit_literal_expr(x),
            Expr::Unary(x) => self.visit_unary_expr(x),
            Expr::Binary(x) => self.visit_binary_expr(x),
            Expr::Call(x) => self.visit_call_expr(x)
        }
    }

    fn visit_lexpr(&mut self, expr: &mut Expr, is_lvalue: bool) -> Self::LExprResult {
        match expr {
            Expr::LVal(lval) => {
                let lval_name = &lval.ident.name;
                let ty_info = self.scopes.find_name_rec(lval_name)
                    .ok_or(SemanticError::UnknownName(lval_name.clone()))?
                    .clone();
                if ty_info.is_const && is_lvalue {
                    return Err(SemanticError::CannotModifyConstValue(lval_name.clone()));
                }

                lval.is_lvalue = is_lvalue;
                lval.ty = ty_info.ty.clone();

                if let Some(Subs { subs, .. }) = &mut lval.subs {
                    let mut cur_ty = &ty_info.ty;

                    for sub in &mut *subs {
                        self.visit_expr(sub)?;
                        expect_type!(sub.ty(), AstTy::Int)?;

                        if let AstTy::Array { elem_ty, .. } | AstTy::Ptr(elem_ty) = cur_ty {
                            cur_ty = elem_ty.as_ref();
                        } else {
                            return Err(SemanticError::TypeMismatch {
                                expected: String::from("ArrayType"),
                                found: (*cur_ty).clone(),
                            })
                        }
                    }

                    lval.ty = (*cur_ty).clone();
                }

                let literal =
                    if ty_info.is_const && !matches!(ty_info.ty, AstTy::Array { .. }) {
                        ty_info.const_val
                    } else {
                        None
                    };
                Ok(literal)
            }
            _ => Err(SemanticError::RequireLValue)
        }
    }

    fn visit_assign_expr(&mut self, expr: &mut AssignExpr) -> Self::ExprResult {
        self.visit_lexpr(&mut expr.lhs, true)?;
        let rval = self.visit_expr(&mut expr.rhs)?;
        if let Some(rval) = &rval {
            expr.rhs = Box::new(Expr::Literal(rval.clone()));
        }
        expect_type!(expr.lhs.ty(), AstTy::Int | AstTy::Bool)?;
        Ok(rval)
    }

    fn visit_literal_expr(&mut self, expr: &mut LiteralExpr) -> Self::ExprResult {
        expr.ty = match &mut expr.kind {
            LiteralKind::Integer(_) => AstTy::Int,
            _ => unreachable!()
        };
        Ok(Some(expr.clone()))
    }

    fn visit_unary_expr(&mut self, expr: &mut UnaryExpr) -> Self::ExprResult {
        let sub_expr_val = self.visit_expr(&mut expr.sub_expr)?;
        if let Some(sub_expr_val) = &sub_expr_val {
            expr.sub_expr = Box::new(Expr::Literal(sub_expr_val.clone()));
        }
        let sub_expr_ty = expr.sub_expr.ty();

        let result_val = match expr.op {
            UnaryOp::Neg => {
                expect_type!(sub_expr_ty, AstTy::Int)?;
                expr.ty = AstTy::Int;

                sub_expr_val.and_then(|x| x.get_int())
                    .map(|x| LiteralExpr {
                        kind: LiteralKind::Integer(-x),
                        span: expr.span,
                        ty: AstTy::Int,
                    })
            }
            UnaryOp::Pos => {
                expect_type!(sub_expr_ty, AstTy::Int)?;
                expr.ty = AstTy::Int;
                sub_expr_val
            }
            UnaryOp::Not => {
                expect_type!(sub_expr_ty, AstTy::Int | AstTy::Bool)?;
                expr.ty = expr.sub_expr.ty();

                sub_expr_val.and_then(|x| x.get_int())
                    .map(|x| LiteralExpr {
                        kind: LiteralKind::Integer(i32::from(x != 0)),
                        span: expr.span,
                        ty: expr.ty.clone(),
                    })
            }
        };
        Ok(result_val)
    }

    fn visit_binary_expr(&mut self, expr: &mut BinaryExpr) -> Self::ExprResult {
        use BinaryOp::{Add, And, Div, Eq, Ge, Gt, Le, Lt, Mod, Mul, Ne, Or, Sub};
        let lval = self.visit_expr(&mut expr.lhs)?;
        let rval = self.visit_expr(&mut expr.rhs)?;
        let op = expr.op;

        if let Some(lval) = lval {
            expr.lhs = Box::new(Expr::Literal(lval));
        }

        if let Some(rval) = rval {
            expr.rhs = Box::new(Expr::Literal(rval));
        }

        let legal = match (expr.lhs.ty(), expr.rhs.ty()) {
            (AstTy::Int, AstTy::Int) => matches!(op, Add | Sub | Mul | Div | Mod | Lt | Le | Gt | Ge | Eq | Ne),
            (AstTy::Bool, AstTy::Bool) => matches!(op, And | Or),
            _ => false,
        };

        if !legal {
            return Err(SemanticError::TypeMismatch {
                expected: match op {
                    Add | Sub | Mul | Div | Mod | Lt | Le | Gt | Ge | Eq | Ne => String::from("AstTy::Int"),
                    And | Or => String::from("AstTy::Bool"),
                },
                found: expr.lhs.ty(),
            })
        }

        let result_ty = match op {
            Add | Sub | Mul | Div | Mod => AstTy::Int,
            Lt | Le | Gt | Ge | Eq | Ne | And | Or => AstTy::Bool,
        };
        expr.ty = result_ty.clone();

        let result_val = if let (
            Expr::Literal(LiteralExpr { kind: LiteralKind::Integer(lval), span: lspan, .. }),
            Expr::Literal(LiteralExpr { kind: LiteralKind::Integer(rval), span: rspan, .. })
        ) = (expr.lhs.as_ref(), expr.rhs.as_ref()) {
            let lval = *lval;
            let rval = *rval;
            let result = match op {
                Add => lval + rval,
                Sub => lval - rval,
                Mul => lval * rval,
                Div => lval / rval,
                Mod => lval % rval,
                Lt => i32::from(lval < rval),
                Le => i32::from(lval <= rval),
                Gt => i32::from(lval > rval),
                Ge => i32::from(lval >= rval),
                Eq => i32::from(lval == rval),
                Ne => i32::from(lval != rval),
                And => i32::from(lval != 0 && rval != 0),
                Or => i32::from(lval != 0 || rval != 0),
            };
            Some(LiteralExpr {
                kind: LiteralKind::Integer(result),
                span: Span { start: lspan.start, end: rspan.end },
                ty: result_ty,
            })
        } else {
            None
        };

        Ok(result_val)
    }

    fn visit_call_expr(&mut self, expr: &mut CallExpr) -> Self::ExprResult {
        expr.args.iter_mut()
            .try_for_each(|arg| self.visit_expr(arg).and(Ok(())))?;

        let func_name = &expr.func.name;
        let (ret_ty, param_tys) = self.scopes.find_name_rec(func_name)
            .ok_or(SemanticError::UnknownName(func_name.clone()))?
            .ty.as_func()
            .ok_or(SemanticError::ExpectedFunction(func_name.clone()))?;

        expr.args.iter()
            .map(Expr::ty)
            .zip(param_tys)
            .try_for_each(|(expected, found)| {
                assert_type_eq(&expected, found)
            })?;

        expr.ty = ret_ty.as_ref().clone();
        Ok(None)
    }

    fn visit_ty(&mut self, ty_def: &mut TypeIdent) -> Self::TyResult {
        let ty = match &ty_def.kind {
            TyIdentKind::Primitive(prim_ty) => match prim_ty {
                PrimitiveTy::Integer => AstTy::Int
            }
            TyIdentKind::Void => AstTy::Void
        };
        Ok(ty)
    }
}

fn assert_type_eq(expected: &AstTy, found: &AstTy) -> Result<(), SemanticError> {
    if expected != found {
        return Err(TypeMismatch {
            expected: format!("{expected:?}"),
            found: found.clone(),
        });
    }
    Ok(())
}
