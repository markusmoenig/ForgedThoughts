#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDef {
    pub name: String,
    pub model: String,
    pub statements: Vec<MaterialStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaterialFunctionStatement {
    Binding { name: String, expr: Expr },
    Return { expr: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaterialStatement {
    Binding {
        name: String,
        expr: Expr,
    },
    Property {
        name: String,
        expr: Expr,
    },
    Function {
        name: String,
        param: String,
        body: Vec<MaterialFunctionStatement>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Binding {
        name: String,
        mutable: bool,
        expr: Expr,
    },
    Assign {
        path: Vec<String>,
        expr: Expr,
    },
    MaterialDef(MaterialDef),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Ident(String),
    ObjectLiteral {
        type_name: String,
        fields: Vec<(String, Expr)>,
    },
    Binary {
        lhs: Box<Expr>,
        op: BinaryOp,
        rhs: Box<Expr>,
    },
    Member {
        target: Box<Expr>,
        field: String,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
}
