use enum_as_inner::EnumAsInner;

use crate::compiler::span::Span;

use super::token::TokenType;

#[derive(Debug, Clone)]
pub struct Program {
    pub program_items: Vec<ProgramItem>,
}

#[derive(Debug, Clone)]
pub enum ProgramItem {
    Decl(Decl),
    Func(AstFunc)
}

#[derive(Debug, Clone)]
pub struct Decl {
    pub is_const: bool,
    pub ty_ident: TypeIdent,
    pub sub_decls: Vec<SubDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SubDecl {
    pub ident: Ident,
    pub subs: Option<Subs>,
    pub init_val: Option<InitVal>,
    pub span: Span,
    pub ty: AstTy,
}

#[derive(Debug, Clone)]
pub struct InitVal {
    pub ty: AstTy,
    pub kind: InitValKind,
    pub span: Span,
}

#[derive(Debug, Clone, EnumAsInner)]
pub enum InitValKind {
    Expr(Expr),
    ArrayVal(Vec<InitVal>),
    Const(LiteralExpr),
}

#[derive(Debug, Clone)]
pub struct AstFunc {
    pub ident: Ident,
    pub params: Vec<FuncParam>,
    pub ret_ty_ident: TypeIdent,
    pub body: BlockStmt,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FuncParam {
    pub ident: Ident,
    pub subs: Option<Subs>,
    pub ty_ident: TypeIdent,
    pub ty: AstTy,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BlockStmt {
    pub block_items: Vec<BlockItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum BlockItem {
    Stmt(Stmt),
    Decl(Decl),
}

impl BlockItem {
    pub fn span(&mut self) -> Span {
        match self {
            BlockItem::Stmt(v) => v.span(),
            BlockItem::Decl(v) => v.span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Block(BlockStmt),
    If(IfStmt),
    While(WhileStmt),
    Break(Span),
    Continue(Span),
    Return(ReturnStmt),
    Empty(Span),
}

impl Stmt {
    #[must_use] pub fn span(&self) -> Span {
        match self {
            Stmt::Expr(v) => v.span(),
            Stmt::Block(v) => v.span,
            Stmt::If(v) => v.span,
            Stmt::While(v) => v.span,
            Stmt::Break(span) | Stmt::Empty(span) | Stmt::Continue(span) => *span,
            Stmt::Return(v) => v.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub cond: Box<Expr>,
    pub then_block: Box<Stmt>,
    pub else_block: Option<Box<Stmt>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub cond: Box<Expr>,
    pub body: Box<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub val: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, EnumAsInner)]
pub enum Expr {
    LVal(LVal),
    Assign(AssignExpr),
    Literal(LiteralExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Call(CallExpr),
}

impl Expr {
    #[must_use] pub fn span(&self) -> Span {
        match self {
            Expr::LVal(x) => x.span,
            Expr::Assign(x) => x.span,
            Expr::Literal(x) => x.span,
            Expr::Unary(x) => x.span,
            Expr::Binary(x) => x.span,
            Expr::Call(x) => x.span,
        }
    }

    #[must_use] pub fn ty(&self) -> AstTy {
        match self {
            Expr::LVal(x) => x.ty.clone(),
            Expr::Assign(x) => x.ty.clone(),
            Expr::Literal(x) => x.ty.clone(),
            Expr::Unary(x) => x.ty.clone(),
            Expr::Binary(x) => x.ty.clone(),
            Expr::Call(x) => x.ty.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
    pub allow_assign_const: bool,
    pub span: Span,
    pub ty: AstTy,
}

#[derive(Debug, Clone)]
pub struct LiteralExpr {
    pub kind: LiteralKind,
    pub span: Span,
    pub ty: AstTy,
}

#[derive(Debug, Clone, EnumAsInner)]
pub enum LiteralKind {
    Integer(i32),
    Array(usize, Vec<LiteralExpr>)
}

impl LiteralExpr {
    #[must_use] pub fn get_int(&self) -> Option<i32> {
        self.kind.as_integer().copied()
    }
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub sub_expr: Box<Expr>,
    pub span: Span,
    pub ty: AstTy,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
    pub span: Span,
    pub ty: AstTy,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub func: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
    pub ty: AstTy,
}

#[derive(Debug, Clone)]
pub struct LVal {
    pub ident: Ident,
    pub subs: Option<Subs>,
    pub span: Span,
    pub ty: AstTy,
    pub is_lvalue: bool
}

#[derive(Debug, Clone)]
pub struct Subs {
    pub subs: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeIdent {
    pub kind: TyIdentKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum TyIdentKind {
    Primitive(PrimitiveTy),
    Void,
}

#[derive(Debug, Clone, Copy)]
pub enum PrimitiveTy {
    Integer,
}

#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Copy, Clone)]
pub enum UnaryOp {
    Neg,
    Pos,
    Not,
}

#[derive(Debug, Copy, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, Eq, EnumAsInner)]
pub enum AstTy {
    Unknown,
    Void,
    Int,
    Bool,
    Func { ret_ty: Box<AstTy>, param_tys: Vec<AstTy> },
    Array { siz: usize, elem_ty: Box<AstTy> },
    Ptr(Box<AstTy>),
}

impl PartialEq for AstTy {
    fn eq(&self, other: &Self) -> bool {
        use AstTy::{Array, Func, Int, Ptr, Unknown, Void};
        match (self, other) {
            (Unknown, Unknown) | (Void, Void) | (Int, Int) => true,
            (Func { ret_ty: ret_ty_x, param_tys: param_tys_x },
                Func { ret_ty: ret_ty_y, param_tys: param_tys_y }) =>
                ret_ty_x == ret_ty_y && param_tys_x == param_tys_y,
            (Array { siz: siz_x, elem_ty: elem_ty_x },
                Array { siz: siz_y, elem_ty: elem_ty_y }) =>
                siz_x == siz_y && elem_ty_x == elem_ty_y,
            (Ptr(x) | Array { elem_ty: x, .. }, Ptr(y) | Array { elem_ty: y, .. }) => x == y,
            _ => false
        }
    }
}

impl TokenType {
    pub fn is_binary_op(&self) -> bool {
        use super::token::TokenType::{And, Assign, Div, Eq, Ge, Gt, Le, Lt, Minus, Mod, Mul, Ne, Not, Or, Plus};
        matches!(
            self,
            Assign | Plus | Minus | Mul | Div | Mod | Eq | Ne | Lt | Gt | Le | Ge | Not | And | Or
        )
    }

    pub fn prec(&self) -> u32 {
        use super::token::TokenType::{And, Assign, Div, Eq, Ge, Gt, Le, Lt, Minus, Mod, Mul, Ne, Or, Plus};
        match self {
            Mul | Div => 20,
            Plus | Minus | Mod => 10,
            Lt | Gt | Le | Ge => 5,
            Eq | Ne => 4,
            And => 3,
            Or => 2,
            Assign => 1,
            _ => unreachable!(),
        }
    }

    pub fn is_left_assoc(&self) -> bool {
        use super::token::TokenType::{And, Assign, Div, Eq, Ge, Gt, Le, Lt, Minus, Mod, Mul, Ne, Or, Plus};
        match self {
            Plus | Minus | Mul | Div | Mod | Eq | Ne | Lt | Gt | Le | Ge | And | Or => true,
            Assign => false,
            _ => unreachable!(),
        }
    }

    pub fn to_binary_op(&self) -> Option<BinaryOp> {
        match self {
            TokenType::Plus => Some(BinaryOp::Add),
            TokenType::Minus => Some(BinaryOp::Sub),
            TokenType::Mul => Some(BinaryOp::Mul),
            TokenType::Div => Some(BinaryOp::Div),
            TokenType::Mod => Some(BinaryOp::Mod),
            TokenType::Eq => Some(BinaryOp::Eq),
            TokenType::Ne => Some(BinaryOp::Ne),
            TokenType::Lt => Some(BinaryOp::Lt),
            TokenType::Gt => Some(BinaryOp::Gt),
            TokenType::Le => Some(BinaryOp::Le),
            TokenType::Ge => Some(BinaryOp::Ge),
            TokenType::And => Some(BinaryOp::And),
            TokenType::Or => Some(BinaryOp::Or),
            _ => None,
        }
    }

    pub fn to_ty_ident(&self) -> Option<TyIdentKind> {
        match self {
            TokenType::IntTy => Some(TyIdentKind::Primitive(PrimitiveTy::Integer)),
            TokenType::VoidTy => Some(TyIdentKind::Void),
            _ => None,
        }
    }

    pub fn is_unary_op(&self) -> bool {
        use super::token::TokenType::{Minus, Not, Plus};
        matches!(
            self,
            Minus | Plus | Not
        )
    }

    pub fn to_unary_op(&self) -> Option<UnaryOp> {
        match self {
            TokenType::Plus => Some(UnaryOp::Pos),
            TokenType::Minus => Some(UnaryOp::Neg),
            TokenType::Not => Some(UnaryOp::Not),
            _ => None,
        }
    }

    pub fn is_ty(&self) -> bool {
        use super::token::TokenType::{IntTy, VoidTy};
        matches!(self, IntTy | VoidTy)
    }
}
