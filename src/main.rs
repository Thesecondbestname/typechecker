#![allow(unused)]
#![allow(clippy::use_self)]
#![allow(clippy::needless_return)]
#![allow(clippy::uninlined_format_args)]

//! Implementation of "Complete and Easy Bidirectional Typechecking for Higher-Rank Polymorphism"
//! See: https://arxiv.org/abs/1306.6032
//!
//! The main focus of this implementation lies beeing able to follow the paper while reading it
//! I tried to keep naming consistent and referencing where things are defined in the paper
//! No sensible error reporting is implemented. Failures will simply result in panics
//!
//! This is an extended version. Check out original.rs for the original implementation.

use im_rc::vector;
use im_rc::Vector;
use std::fmt;

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
enum Name {
    Index(usize),
    Name(&'static str),
}

impl Name {
    const fn with_name(n: &'static str) -> Self {
        Self::Name(n)
    }
}
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => return write!(f, "t{name}"),
            Self::Index(i) => write!(f, "t{}", i),
        }
    }
}

///Figure 6
#[derive(Clone, Debug)]
enum Expr {
    /// Variable
    Var(Name),
    /// Literal
    Lit(Lit),
    /// Abstraction
    Abs(Name, Box<Expr>),
    /// Application
    App(Box<Expr>, Box<Expr>),
    /// Let expression
    Let(Name, Box<Expr>, Box<Expr>),
    /// Type Annotation
    Ann(Box<Expr>, Type),
    /// Tuple
    Tup(Box<Expr>, Box<Expr>),
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Expr::Lit(l) => write!(f, "{}", l),
            Expr::Var(x) => write!(f, "{}", x),
            Expr::Abs(x, e) => write!(f, "(λ{}.{})", x, e),
            Expr::App(e1, e2) => write!(f, "{} {}", e1, e2),
            Expr::Let(x, e0, e1) => write!(f, "let {} = {} in {}", x, e0, e1),
            Expr::Ann(e, t) => write!(f, "({}: {})", e, t),
            Expr::Tup(e0, e1) => write!(f, "({}, {})", e0, e1),
        }
    }
}

#[derive(Clone, Debug)]
enum Lit {
    Char(char),
    String(String),
    Int(isize),
    Float(f64),
    Bool(bool),
    Unit,
}

impl fmt::Display for Lit {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Lit::Char(val) => write!(f, "'{}'", val),
            Lit::String(val) => write!(f, "'{}'", val),
            Lit::Int(val) => write!(f, "{}", val),
            Lit::Float(val) => write!(f, "{}", val),
            Lit::Bool(val) => write!(f, "{}", val),
            Lit::Unit => write!(f, "()"),
        }
    }
}

/// Figure 6
#[derive(Clone, Debug, PartialEq, Eq)]
enum Type {
    /// Literal type
    Lit(LitType),
    /// Type variable
    Var(Name),
    /// Existential type
    Exists(Name),
    /// Forall quantifier
    Forall(Name, Box<Type>),
    /// Function type
    Fun(Box<Type>, Box<Type>),
    /// Tuple type
    Tup(Box<Type>, Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Type::Lit(lit) => write!(f, "{}", lit),
            Type::Var(var) => write!(f, "{}", var),
            Type::Exists(ex) => write!(f, "{}^", ex),
            Type::Forall(a, ty) => write!(f, "(∀{}. {})", a, ty),
            Type::Fun(a, c) => write!(f, "({} -> {})", a, c),
            Type::Tup(a, b) => write!(f, "{} × {}", a, b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LitType {
    Unit,
    Char,
    String,
    Int,
    Float,
    Bool,
}

impl fmt::Display for LitType {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            LitType::Unit => write!(f, "()"),
            LitType::Char => write!(f, "Char"),
            LitType::String => write!(f, "String"),
            LitType::Int => write!(f, "Int"),
            LitType::Float => write!(f, "Float"),
            LitType::Bool => write!(f, "Bool"),
        }
    }
}

impl Type {
    fn is_monotype(&self) -> bool {
        match self {
            Type::Forall(..) => false,
            Type::Fun(t1, t2) => t1.is_monotype() && t2.is_monotype(),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CtxElem {
    /// Variable
    Var(Name),
    /// Existential type variable
    Exists(Name),
    /// Solved type variable
    Solved(Name, Type),
    /// Marker type variable
    Marker(Name),
    /// Typed term variable
    TypedVar(Name, Type),
}

impl fmt::Display for CtxElem {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            CtxElem::Var(var) => write!(f, "{}", var),
            CtxElem::Exists(ex) => write!(f, "{}^", ex),
            CtxElem::Solved(a, ty) => write!(f, "{}^: {}", a, ty),
            CtxElem::Marker(a) => write!(f, "<|{}", a),
            CtxElem::TypedVar(x, ty) => write!(f, "{}: {}", x, ty),
        }
    }
}

/// As the context needs to be ordered, it is implemented as a simple Vector.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Ctx {
    elements: Vector<CtxElem>,
}

impl fmt::Display for Ctx {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "[").unwrap();
        self.elements.iter().fold(true, |first, ele| {
            if !first {
                write!(f, ", ").unwrap();
            };
            write!(f, "{}", ele).unwrap();
            false
        });
        write!(f, "]")
    }
}

/// Context operations derive from "Hole notation" described in 3.1 and the fact that the context is ordered.
impl Ctx {
    /// Adds an element to the end of the context
    fn add(&self, element: CtxElem) -> Self {
        let mut eles = self.elements.clone();
        eles.push_back(element);
        Ctx { elements: eles }
    }

    /// Splits a context at the index of an element, the element is included in the left-hand-side of the split
    fn split_at(&self, element: CtxElem) -> (Ctx, Ctx) {
        if let Some(index) = self.elements.iter().position(|ele| ele == &element) {
            let (lhs, rhs) = self.elements.clone().split_at(index);
            let left_context = Ctx { elements: lhs };
            let right_context = Ctx { elements: rhs };

            return (left_context, right_context);
        }
        panic!();
    }

    /// Replaces `element` with `inserts`
    fn insert_in_place(&self, element: CtxElem, inserts: Vector<CtxElem>) -> Self {
        if let Some(index) = self.elements.iter().position(|ele| ele == &element) {
            let (mut lhs, rhs) = self.elements.clone().split_at(index + 1);
            lhs.append(inserts);
            lhs.append(rhs);
            return Ctx { elements: lhs };
        }
        panic!();
    }

    /// Drops all elements after `element`
    fn drop(&self, element: CtxElem) -> Self {
        if let Some(index) = self.elements.iter().position(|ele| ele == &element) {
            let mut eles = self.elements.clone();
            eles.split_off(index);
            return Ctx { elements: eles };
        }
        panic!();
    }

    /// Returns `Some(Type)` if `a0` is solved, else `None`
    fn get_solved(&self, a0: Name) -> Option<&Type> {
        for elem in &self.elements {
            if let CtxElem::Solved(a1, t) = elem {
                if a0 == *a1 {
                    return Some(t);
                }
            }
        }
        None
    }

    /// Returns `true` if `a` is an existential, else `false`
    fn has_existential(&self, a: Name) -> bool {
        self.elements.iter().any(|elem| elem == &CtxElem::Exists(a))
    }

    /// Returns `true` if `a` is a variable, else `false`.
    fn has_variable(&self, a: Name) -> bool {
        self.elements.iter().any(|ele| ele == &CtxElem::Var(a))
    }

    /// Returns `Some(Type)` if `x` is a type annotation, else `None`
    fn get_annotation(&self, x0: Name) -> Option<&Type> {
        for elem in &self.elements {
            if let CtxElem::TypedVar(x1, t) = elem {
                if x0 == *x1 {
                    return Some(t);
                }
            }
        }
        None
    }
}

/// The state is used to generate new existentials.
/// (In the paper mostly notated as α^ α1^ or β^)
/// It is passed around mutably everywhere
#[derive(Clone, Debug, Default)]
struct State {
    existentials: usize,
}

impl State {
    /// Returns a fresh exitential
    fn fresh_existential(&mut self) -> Name {
        let result = self.existentials;
        self.existentials += 1;
        Name::Index(result)
    }
}

/// Returns `true` if literal expression checks against literal type, else `false`.
const fn literal_checks_against(e: &Lit, t: &LitType) -> bool {
    matches!(
        (e, t),
        (Lit::Char(_), LitType::Char)
            | (Lit::String(_), LitType::String)
            | (Lit::Int(_), LitType::Int)
            | (Lit::Float(_), LitType::Float)
            | (Lit::Bool(_), LitType::Bool)
            | (Lit::Unit, LitType::Unit)
    )
}

/// Figure 11.
fn checks_against(state: &mut State, ctx0: &Ctx, e: &Expr, t: &Type) -> Ctx {
    print_helper("check", format!("{}", e), format!("{}", t), ctx0);
    assert!(is_well_formed(ctx0, t));
    match (e, t) {
        // 1I
        (Expr::Lit(e), Type::Lit(t)) => {
            print_rule("1I");
            assert!(literal_checks_against(e, t));
            ctx0.clone()
        }
        // ->I
        (Expr::Abs(x, e), Type::Fun(t0, t1)) => {
            print_rule("->I");
            let elem = CtxElem::TypedVar(*x, *t0.clone());
            let ctx1 = ctx0.add(elem.clone());
            let ctx2 = checks_against(state, &ctx1, e, t1);
            let ctx3 = ctx2.drop(elem);
            ctx3
        }
        // ∀I
        (_, Type::Forall(a, t)) => {
            print_rule("∀I");
            let elem = CtxElem::Var(*a);
            let ctx1 = ctx0.add(elem.clone());
            let ctx2 = checks_against(state, &ctx1, e, t);
            let ctx3 = ctx2.drop(elem);
            ctx3
        }
        // xI
        (Expr::Tup(e0, e1), Type::Tup(t0, t1)) => {
            print_rule("xI");
            let ctx1 = checks_against(state, ctx0, e0, t0);
            let ctx2 = checks_against(state, &ctx1, e1, t1);
            ctx2
        }
        // Sub
        (_, _) => {
            print_rule("Sub");
            let (t1, ctx1) = synthesizes_to(state, ctx0, e);
            let ctx2 = apply_context(t1, &ctx1);
            let ctx3 = apply_context(t.clone(), &ctx1);
            let ctx4 = subtype(state, &ctx1, &ctx2, &ctx3);
            ctx4
        }
    }
}

/// Synthesizes a type from a literal.
const fn literal_synthesizes_to(e: &Lit) -> LitType {
    match e {
        Lit::Char(_) => LitType::Char,
        Lit::String(_) => LitType::String,
        Lit::Int(_) => LitType::Int,
        Lit::Float(_) => LitType::Float,
        Lit::Bool(_) => LitType::Bool,
        Lit::Unit => LitType::Unit,
    }
}

///Figure 11
fn synthesizes_to(state: &mut State, ctx0: &Ctx, e: &Expr) -> (Type, Ctx) {
    print_helper("synth", format!("{e}"), String::new(), ctx0);
    match e {
        // 1I=>
        Expr::Lit(e) => {
            print_rule("1I=>");
            (Type::Lit(literal_synthesizes_to(e)), ctx0.clone())
        }
        // Var
        Expr::Var(x) => {
            print_rule("Var");
            if let Some(t) = ctx0.get_annotation(*x) {
                return (t.clone(), ctx0.clone());
            };
            panic!();
        }
        // Anno
        Expr::Ann(e, t) => {
            print_rule("Anno");
            if is_well_formed(ctx0, t) {
                let ctx1 = checks_against(state, ctx0, e, t);
                return (t.clone(), ctx1);
            }
            panic!();
        }
        //->I=>
        Expr::Abs(x, e) => {
            print_rule("->I=>");
            let ex0 = state.fresh_existential();
            let ex1 = state.fresh_existential();
            let ctx1 = ctx0
                .add(CtxElem::Exists(ex0))
                .add(CtxElem::Exists(ex1))
                .add(CtxElem::TypedVar(*x, Type::Exists(ex0)));
            let ctx2 = checks_against(state, &ctx1, e, &Type::Exists(ex1))
                .drop(CtxElem::TypedVar(*x, Type::Exists(ex0)));
            return (
                Type::Fun(Type::Exists(ex0).into(), Type::Exists(ex1).into()),
                ctx2,
            );
        }
        Expr::Tup(e0, e1) => {
            print_rule("SynthProduct");
            let (t1, ctx1) = synthesizes_to(state, ctx0, e0);
            let (t2, ctx2) = synthesizes_to(state, &ctx1, e1);
            return (Type::Tup(t1.into(), t2.into()), ctx2);
        }
        Expr::Let(x, e0, e1) => {
            print_rule("Let");
            let (t0, ctx1) = synthesizes_to(state, ctx0, e0);
            let ctx2 = ctx1.add(CtxElem::TypedVar(*x, t0.clone()));
            let (t1, ctx3) = synthesizes_to(state, &ctx2, e1);
            let ctx4 = ctx3.insert_in_place(CtxElem::TypedVar(*x, t0), vector![]);
            return (t1, ctx4);
        }
        // ->E
        Expr::App(e0, e1) => {
            print_rule("->E");
            let (t, ctx1) = synthesizes_to(state, ctx0, e0);
            return application_synthesizes_to(state, &ctx1, &apply_context(t, &ctx1), e1);
        }
    }
}

/// Figure 11
fn application_synthesizes_to(state: &mut State, ctx0: &Ctx, t: &Type, e: &Expr) -> (Type, Ctx) {
    print_helper("app_synth", format!("{e}"), format!("{t}"), ctx0);
    match t {
        // α^App
        Type::Exists(ex0) => {
            print_rule("α^App");
            let ex1 = state.fresh_existential();
            let ex2 = state.fresh_existential();
            let ctx1 = ctx0.insert_in_place(
                CtxElem::Exists(*ex0),
                vector![
                    CtxElem::Exists(ex2),
                    CtxElem::Exists(ex1),
                    CtxElem::Solved(
                        *ex0,
                        Type::Fun(Type::Exists(ex1).into(), Type::Exists(ex2).into(),),
                    ),
                ],
            );
            let ctx2 = checks_against(state, &ctx1, e, &Type::Exists(ex1));
            return (Type::Exists(ex2), ctx2);
        }
        // ∀App
        Type::Forall(a0, t0) => {
            print_rule("∀App");
            let ex0 = state.fresh_existential();
            let ctx1 = ctx0.add(CtxElem::Exists(ex0));
            let t1 = substitution(t0, *a0, &Type::Exists(ex0));
            return application_synthesizes_to(state, &ctx1, &t1, e);
        }
        // App
        Type::Fun(t0, t1) => {
            print_rule("->App");
            let ctx1 = checks_against(state, ctx0, e, t0);
            return (*t1.clone(), ctx1);
        }
        _ => panic!(),
    }
}

/// Figure 7
fn is_well_formed(ctx: &Ctx, t: &Type) -> bool {
    match t {
        Type::Lit(_) => true,
        Type::Var(x) => ctx.has_variable(*x),
        Type::Fun(t0, t1) => is_well_formed(ctx, t0) && is_well_formed(ctx, t1),
        Type::Forall(a, t) => is_well_formed(&ctx.add(CtxElem::Var(*a)), t),
        Type::Exists(ex) => ctx.has_existential(*ex) || ctx.get_solved(*ex).is_some(),
        Type::Tup(t0, t1) => is_well_formed(ctx, t0) && is_well_formed(ctx, t1),
    }
}

/// This corresponds to the FV call in Figure 9 Rule <:`InstantiateL` and <:`InstantiateR`
/// It checks if a existential variable already occurs in a type to be able to find and panic on cycles
///
/// Alas, I could not find a definition of the FV function and had to copy the implementation of
/// https://github.com/ollef/Bidirectional and https://github.com/atennapel/bidirectional.js
fn occurs_in(x: Name, a: &Type) -> bool {
    match a {
        Type::Lit(_) => false,
        Type::Var(a) => x == *a,
        Type::Fun(t1, t2) => occurs_in(x, t1) || occurs_in(x, t2),
        Type::Forall(a, t) => {
            if x == *a {
                return true;
            }
            return occurs_in(x, t);
        }
        Type::Exists(ex) => x == *ex,
        Type::Tup(t0, t1) => occurs_in(x, t0) || occurs_in(x, t1),
    }
}

/// Figure 9
fn subtype(state: &mut State, ctx0: &Ctx, t0: &Type, t1: &Type) -> Ctx {
    print_helper("subtype", format!("{t0}"), format!("{t1}"), ctx0);
    assert!(is_well_formed(ctx0, t0));
    assert!(is_well_formed(ctx0, t1));
    match (t0, t1) {
        // <:Unit
        (Type::Lit(t0), Type::Lit(t1)) => {
            print_rule("<:Unit");
            assert_eq!(t0, t1);
            ctx0.clone()
        }
        // <:Var
        (Type::Var(a0), Type::Var(a1)) => {
            print_rule("<:Var");
            if is_well_formed(ctx0, t0) && a0 == a1 {
                return ctx0.clone();
            } else {
                panic!();
            }
        }
        // <:Exvar
        (Type::Exists(ex0), Type::Exists(ex1)) if ex0 == ex1 => {
            print_rule("<:Exvar");
            if is_well_formed(ctx0, t0) {
                return ctx0.clone();
            } else {
                panic!();
            }
        }
        // <:->
        (Type::Fun(ta1, ta2), Type::Fun(tb1, tb2)) => {
            print_rule("<:->");
            let ctx1 = subtype(state, ctx0, tb1, ta1);
            return subtype(
                state,
                &ctx1,
                &apply_context(*ta2.clone(), &ctx1),
                &apply_context(*tb2.clone(), &ctx1),
            );
        }
        (Type::Tup(ta1, ta2), Type::Tup(tb1, tb2)) => {
            print_rule("SubProduct");
            let ctx1 = subtype(state, ctx0, ta1, tb1);
            let ctx2 = subtype(state, &ctx1, ta2, tb2);
            ctx2
        }
        // <:∀L
        (Type::Forall(a, t2), _) => {
            print_rule("<:∀L");
            let ex0 = state.fresh_existential();
            let ctx1 = ctx0.add(CtxElem::Marker(ex0)).add(CtxElem::Exists(ex0));
            let t3 = substitution(t2, *a, &Type::Exists(ex0));
            let ctx2 = subtype(state, &ctx1, &t3, t1);
            return ctx2.drop(CtxElem::Marker(ex0));
        }
        // <:∀R
        (_, Type::Forall(a, t2)) => {
            print_rule("<:∀R");
            let ctx1 = ctx0.add(CtxElem::Var(*a));
            let ctx2 = subtype(state, &ctx1, t0, t2);
            return ctx2.drop(CtxElem::Var(*a));
        }
        // <:InstatiateL
        (Type::Exists(ex0), _) => {
            print_rule("<:InstantiateL");
            if !occurs_in(*ex0, t1) {
                instantiate_l(state, ctx0, *ex0, t1)
            } else {
                panic!("Circular!");
            }
        }
        // <:InstantiateR
        (_, Type::Exists(ex0)) => {
            print_rule("<:InstantiateR");
            if !occurs_in(*ex0, t0) {
                instantiate_r(state, ctx0, t0, *ex0)
            } else {
                panic!("Circular!");
            }
        }
        _ => panic!("Couldn't subtype!"),
    }
}

/// Figure 10
fn instantiate_l(state: &mut State, ctx0: &Ctx, ex0: Name, t: &Type) -> Ctx {
    print_helper("instantiate_l", ex0.to_string(), format!("{}", t), ctx0);
    match t {
        // InstLSolve
        t if {
            let (ctx1, _) = ctx0.split_at(CtxElem::Exists(ex0));
            t.is_monotype() && is_well_formed(&ctx1, t)
        } =>
        {
            print_rule("InstLSolve");
            return ctx0.insert_in_place(
                CtxElem::Exists(ex0),
                vector![CtxElem::Solved(ex0.into(), t.clone())],
            );
        }
        // InstLArr
        Type::Fun(t1, t2) => {
            print_rule("InstLArr");
            let ex1 = state.fresh_existential();
            let ex2 = state.fresh_existential();
            let ctx1 = ctx0.insert_in_place(
                CtxElem::Exists(ex0),
                vector![
                    CtxElem::Exists(ex2.clone()),
                    CtxElem::Exists(ex1.clone()),
                    CtxElem::Solved(
                        ex0.into(),
                        Type::Fun(
                            Type::Exists(ex1.clone()).into(),
                            Type::Exists(ex2.clone()).into(),
                        ),
                    ),
                ],
            );
            let ctx2 = instantiate_r(state, &ctx1, t1, ex1);
            let ctx3 = instantiate_l(state, &ctx2, ex2, &apply_context(*t2.clone(), &ctx2));
            return ctx3;
        }
        // InstAIIR
        Type::Forall(a, t1) => {
            print_rule("InstLAllR");
            let ctx1 = instantiate_l(state, &ctx0.add(CtxElem::Var(*a)), ex0, t1);
            return ctx1.drop(CtxElem::Var(*a));
        }
        // InstLReach
        Type::Exists(ex1) => {
            print_rule("InstLReach");
            return ctx0.insert_in_place(
                CtxElem::Exists(ex1.clone()),
                vector![CtxElem::Solved(ex1.clone(), Type::Exists(ex0.into()),)],
            );
        }
        _ => panic!(),
    }
}

/// Figure 10
fn instantiate_r(state: &mut State, ctx0: &Ctx, t: &Type, ex0: Name) -> Ctx {
    print_helper("instantiate_r", format!("{}", t), ex0.to_string(), ctx0);
    match t {
        // InstRSolve
        t if {
            let (ctx1, _) = ctx0.split_at(CtxElem::Exists(ex0));
            t.is_monotype() && is_well_formed(&ctx1, t)
        } =>
        {
            return ctx0.insert_in_place(
                CtxElem::Exists(ex0.into()),
                vector![CtxElem::Solved(ex0.into(), t.clone())],
            );
        }
        // InstRArr
        Type::Fun(t0, t1) => {
            print_rule("InstRArr");
            let ex1 = state.fresh_existential();
            let ex2 = state.fresh_existential();
            let ctx1 = ctx0.insert_in_place(
                CtxElem::Exists(ex0.into()),
                vector![
                    CtxElem::Exists(ex2.clone()),
                    CtxElem::Exists(ex1.clone()),
                    CtxElem::Solved(
                        ex0.into(),
                        Type::Fun(
                            Type::Exists(ex1.clone()).into(),
                            Type::Exists(ex2.clone()).into(),
                        ),
                    ),
                ],
            );
            let ctx2 = instantiate_l(state, &ctx1, ex1, t0);
            let ctx3 = instantiate_r(state, &ctx2, &apply_context(*t1.clone(), &ctx2), ex2);
            return ctx3;
        }
        // InstRAllL
        Type::Forall(a, t1) => {
            print_rule("InstRAllL");
            let ex1 = state.fresh_existential();
            let ctx1 = ctx0
                .add(CtxElem::Marker(ex1.clone()))
                .add(CtxElem::Exists(ex1.clone()));
            let ctx2 = instantiate_r(
                state,
                &ctx1,
                &substitution(t1, *a, &Type::Exists(ex1.clone())),
                ex0,
            );
            let ctx3 = ctx2.drop(CtxElem::Marker(ex1.clone()));
            return ctx3;
        }
        Type::Tup(t0, t1) => {
            print_rule("InstRProd");
            let ex1 = state.fresh_existential();
            let ex2 = state.fresh_existential();
            let ctx1 = ctx0.insert_in_place(
                CtxElem::Exists(ex0.into()),
                vector![
                    CtxElem::Exists(ex2.clone()),
                    CtxElem::Exists(ex1.clone()),
                    CtxElem::Solved(
                        ex0.into(),
                        Type::Tup(
                            Type::Exists(ex1.clone()).into(),
                            Type::Exists(ex2.clone()).into(),
                        ),
                    ),
                ],
            );
            let ctx2 = instantiate_l(state, &ctx1, ex1, t0);
            let ctx3 = instantiate_r(state, &ctx2, &apply_context(*t1.clone(), &ctx2), ex2);
            return ctx3;
        }
        // InstRReach
        Type::Exists(ex1) => {
            print_rule("InstRReach");
            return ctx0.insert_in_place(
                CtxElem::Exists(ex1.clone()),
                vector![CtxElem::Solved(ex1.clone(), Type::Exists(ex0.into()),)],
            );
        }
        _ => panic!(),
    }
}

/// Figure 8
fn apply_context(t: Type, ctx: &Ctx) -> Type {
    match t {
        Type::Var(_) => t,
        Type::Lit(_) => t,
        Type::Exists(ref ex) => {
            if let Some(t1) = ctx.get_solved(*ex) {
                apply_context(t1.clone(), ctx)
            } else {
                t
            }
        }
        Type::Fun(t0, t1) => Type::Fun(
            apply_context(*t0, ctx).into(),
            apply_context(*t1, ctx).into(),
        ),
        Type::Forall(a, t0) => Type::Forall(a, apply_context(*t0, ctx).into()),
        Type::Tup(t0, t1) => Type::Tup(
            apply_context(*t0, ctx).into(),
            apply_context(*t1, ctx).into(),
        ),
    }
}

/// Similar to the FV function from subtyping I couldn't find a definition of substitution in the paper
/// Thus I tried to copy the implementation of
/// <https://github.com/ollef/Bidirectional> and <https://github.com/atennapel/bidirectional.js>
///
/// Substitution is written in the paper as [α^/α]A which means, α is replaced with α^ in all occurrences in A
fn substitution(t: &Type, xr: Name, tr: &Type) -> Type {
    match t {
        Type::Lit(_) => t.clone(),
        Type::Var(x) => {
            if xr == *x {
                tr.clone()
            } else {
                t.clone()
            }
        }
        Type::Forall(a, t2) => {
            if xr == *a {
                Type::Forall(*a, tr.clone().into())
            } else {
                Type::Forall(*a, substitution(t2, xr, tr).into())
            }
        }
        Type::Exists(ex) => {
            if xr == *ex {
                tr.clone()
            } else {
                t.clone()
            }
        }
        Type::Tup(t0, t1) => Type::Tup(
            substitution(t0, xr, tr).into(),
            substitution(t1, xr, tr).into(),
        ),
        Type::Fun(t0, t1) => Type::Fun(
            substitution(t0, xr, tr).into(),
            substitution(t1, xr, tr).into(),
        ),
    }
}

fn print_helper(fun: &str, c1: String, c2: String, context: &Ctx) {
    print!(
        "{:<15} {:<45}| {:<25} {:<48}",
        fun,
        c1,
        c2,
        format!("{}", context)
    );
}

fn print_rule(rule: &str) {
    println!("{rule:>20}");
}
mod test {
    use crate::apply_context;
    use crate::synthesizes_to;
    use crate::Ctx;
    use crate::Expr;
    use crate::Lit;
    use crate::Name;
    use crate::State;
    use crate::{LitType, Type};

    /// "Test": String
    #[test]
    fn basic() {
        println!();
        println!();
        assert_eq!(lit_str().synth(), Type::Lit(LitType::String));
    }

    /// (λx.x) "Test": String
    #[test]
    fn application_string() {
        println!();
        println!();
        assert_eq!(application(abs("x", var("x")), lit_str()).synth(), ty_str());
    }

    /// (λx.x) true: bool
    #[test]
    fn application_bool() {
        println!();
        println!();
        assert_eq!(
            application(abs("x", var("x")), lit_bool()).synth(),
            ty_bool()
        );
    }

    /// λx.x: 't0->'t0
    #[test]
    fn lambda() {
        println!();
        println!();
        assert_eq!(
            abs("x", var("x")).synth(),
            ty_fun(ty_existential("t0"), ty_existential("t0"))
        );
    }

    /// (λx.x) "Test": String
    #[test]
    fn idunit() {
        println!();
        println!();
        assert_eq!(application(id(), lit_str()).synth(), ty_str());
    }

    /// ("Test" × true): (String × Bool)
    #[test]
    fn tuples() {
        println!();
        println!();
        assert_eq!(
            tuple(lit_str(), lit_bool()).synth(),
            ty_tuple(ty_str(), ty_bool())
        );
    }

    /// (λx.(x × x)) "Test": (String × String)
    #[test]
    fn tuples_in_lambda() {
        println!();
        println!();
        assert_eq!(
            application(abs("x", tuple(var("x"), var("x"))), lit_str()).synth(),
            ty_tuple(ty_str(), ty_str())
        );
    }

    /// ((λx.(x × (x × x))) "Test"): (String × (String × String))
    #[test]
    fn nested_tuples() {
        println!();
        println!();
        assert_eq!(
            application(
                abs("x", tuple(var("x"), tuple(var("x"), var("x")))),
                lit_str()
            )
            .synth(),
            ty_tuple(ty_str(), ty_tuple(ty_str(), ty_str()))
        );
    }

    /// ((λx.x) ("Test" × true)): (String × bool)
    #[test]
    fn tuples_in_fn() {
        println!();
        println!();
        assert_eq!(
            application(id(), tuple(lit_str(), lit_bool())).synth(),
            ty_tuple(ty_str(), ty_bool())
        );
    }

    /// (let newid = λx.x in ((newid "Test") × (newid true))): (String × bool)
    #[test]
    fn generalised_let() {
        println!();
        println!();
        assert_eq!(
            let_in(
                "newid",
                id(),
                // Without annotation, e.g. abs("x", var("x")) It fails.
                tuple(
                    application(var("newid"), lit_str()),
                    application(var("newid"), lit_bool())
                )
            )
            .synth(),
            ty_tuple(ty_str(), ty_bool())
        );
    }

    /// (let a = true in id a): bool
    #[test]
    fn let_binding() {
        println!();
        println!();
        assert_eq!(
            let_in("a", lit_bool(), application(id(), var("a"))).synth(),
            ty_bool()
        );
    }

    /// ((let newid = λx.x in newid) "Test"): String
    #[test]
    fn let_fn() {
        println!();
        println!();
        assert_eq!(
            application(let_in("newid", abs("x", var("x")), var("newid")), lit_str()).synth(),
            ty_str()
        );
    }

    fn application(e0: Expr, e1: Expr) -> Expr {
        Expr::App(e0.into(), e1.into())
    }

    fn let_in(x: &'static str, e0: Expr, e1: Expr) -> Expr {
        Expr::Let(Name::with_name(x), e0.into(), e1.into())
    }

    fn abs(x: &'static str, e: Expr) -> Expr {
        Expr::Abs(Name::with_name(x), e.into())
    }

    fn var(x: &'static str) -> Expr {
        Expr::Var(Name::with_name(x))
    }

    /// (λx.x): ∀t.t->t
    fn id() -> Expr {
        ann(
            abs("x", var("x")),
            ty_forall("t", ty_fun(ty_var("t"), ty_var("t"))),
        )
    }

    fn lit_str() -> Expr {
        Expr::Lit(Lit::String("Test".into()))
    }

    const fn lit_bool() -> Expr {
        Expr::Lit(Lit::Bool(true))
    }

    fn tuple(e0: Expr, e1: Expr) -> Expr {
        Expr::Tup(e0.into(), e1.into())
    }

    fn ann(e: Expr, t: Type) -> Expr {
        Expr::Ann(e.into(), t)
    }

    const fn ty_str() -> Type {
        Type::Lit(LitType::String)
    }

    const fn ty_bool() -> Type {
        Type::Lit(LitType::Bool)
    }

    fn ty_tuple(t0: Type, t1: Type) -> Type {
        Type::Tup(t0.into(), t1.into())
    }

    fn ty_fun(t0: Type, t1: Type) -> Type {
        Type::Fun(t0.into(), t1.into())
    }

    fn ty_existential(ex: &'static str) -> Type {
        Type::Exists(Name::with_name(ex))
    }

    fn ty_var(x: &'static str) -> Type {
        Type::Var(Name::with_name(x))
    }

    fn ty_forall(x: &'static str, t: Type) -> Type {
        Type::Forall(Name::with_name(x), t.into())
    }

    impl Expr {
        fn synth(self) -> Type {
            let (t, ctx) = synthesizes_to(&mut State::default(), &Ctx::default(), &self);
            println!();
            println!();
            println!("-------------------RESULTS-------------------");
            println!("{} in context {}", t, ctx);
            let t = apply_context(t, &ctx);
            println!("Applied: {}", t);
            // println!("{}", expression);
            println!("-------------------");
            t
        }
    }
}

const fn main() {}
