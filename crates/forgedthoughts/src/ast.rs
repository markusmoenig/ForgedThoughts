#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDef {
    pub name: String,
    pub model: String,
    pub metadata: Vec<(String, Expr)>,
    pub statements: Vec<MaterialStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SdfDef {
    pub name: String,
    pub metadata: Vec<(String, Expr)>,
    pub statements: Vec<SdfStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentDef {
    pub name: String,
    pub metadata: Vec<(String, Expr)>,
    pub statements: Vec<MaterialStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<MaterialFunctionStatement>,
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
        params: Vec<String>,
        body: Vec<MaterialFunctionStatement>,
    },
}

pub type SdfFunctionStatement = MaterialFunctionStatement;

#[derive(Debug, Clone, PartialEq)]
pub enum SdfStatement {
    Binding {
        name: String,
        expr: Expr,
    },
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<SdfFunctionStatement>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Import {
        path: String,
        alias: Option<String>,
    },
    Export(Vec<String>),
    Binding {
        name: String,
        mutable: bool,
        expr: Expr,
    },
    Assign {
        path: Vec<String>,
        expr: Expr,
    },
    FunctionDef(FunctionDef),
    MaterialDef(MaterialDef),
    SdfDef(SdfDef),
    EnvironmentDef(EnvironmentDef),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    String(String),
    Array(Vec<Expr>),
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
    Intersect,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
}
