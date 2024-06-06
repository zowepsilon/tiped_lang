use std::collections::{HashMap, HashSet};

use crate::tree::{Declaration, Expression, Type};

fn occurs_in(variable: usize, term: &Type) -> bool {
    match term {
        Type::Var(name) => &variable == name,
        Type::Fun(from, to) => occurs_in(variable, from) || occurs_in(variable, to),
        Type::ParameterizedAtom {
            name: _,
            parameters,
        } => parameters.iter().any(|p| occurs_in(variable, p)),
        Type::Atom(_) | Type::PolymorphicVar(_) => false,
        Type::Polymorphic {
            variables: _,
            matrix,
        } => occurs_in(variable, matrix),
    }
}

fn substitute(old: usize, new: &Type, expr: &mut Type) {
    match expr {
        Type::Var(v) if v == &old => *expr = new.clone(),
        Type::Fun(from, to) => {
            substitute(old, new, from);
            substitute(old, new, to);
        }
        Type::ParameterizedAtom {
            name: _,
            parameters,
        } => {
            for p in parameters {
                substitute(old, new, p);
            }
        }
        Type::Polymorphic {
            variables: _,
            matrix,
        } => substitute(old, new, matrix),
        Type::Atom(_) | Type::PolymorphicVar(_) | Type::Var(_) => (),
    }
}

fn substitute_polymorphic_variables(old: usize, new: &Type, expr: &mut Type) {
    match expr {
        Type::PolymorphicVar(v) if v == &old => *expr = new.clone(),
        Type::Polymorphic {
            variables: _,
            matrix,
        } => substitute_polymorphic_variables(old, new, matrix),
        Type::Fun(from, to) => {
            substitute_polymorphic_variables(old, new, from);
            substitute_polymorphic_variables(old, new, to);
        }
        Type::ParameterizedAtom {
            name: _,
            parameters,
        } => {
            for p in parameters {
                substitute_polymorphic_variables(old, new, p);
            }
        }
        Type::PolymorphicVar(_) | Type::Atom(_) | Type::Var(_) => (),
    }
}

struct ConstraintBuildingResult {
    type_: Type,
    constraints: Vec<(Type, Type)>,
}

#[derive(Debug, Clone)]
pub struct Environment {
    variables: HashMap<String, Type>,
    type_variables: HashSet<usize>,
    parent_type_variables: HashSet<usize>,
    type_atoms: HashMap<String, usize>, // only store the arity of the atom
}

impl Environment {
    fn add_variable(&mut self, name: String, ty: Type) {
        if name == "_" {
            return;
        }

        self.variables.insert(name, ty);
    }

    fn assert_is_valid(&self, ty: &Type) {
        match ty {
            Type::Atom(name) => {
                if !self.type_atoms.contains_key(name) {
                    panic!("unknown type {name}")
                }

                if self.type_atoms[name] != 0 {
                    panic!(
                        "wrong number of parameters for type {name}, expected {}, found 0",
                        self.type_atoms[name]
                    );
                }
            }
            Type::Var(v) => {
                if !self.type_variables.contains(v) {
                    panic!("unknown type variable '{v}");
                }
            }
            Type::PolymorphicVar(_) => (),
            Type::Fun(from, to) => {
                self.assert_is_valid(from);
                self.assert_is_valid(to);
            }
            Type::ParameterizedAtom { name, parameters } => {
                if !self.type_atoms.contains_key(name) {
                    panic!("unknown type {name}");
                }

                if self.type_atoms[name] != parameters.len() {
                    panic!(
                        "wrong number of parameters for type {name}, expected {}, found {}",
                        self.type_atoms[name],
                        parameters.len()
                    );
                }

                for p in parameters {
                    self.assert_is_valid(p);
                }
            }
            Type::Polymorphic {
                variables: _,
                matrix,
            } => self.assert_is_valid(matrix),
        }
    }

    fn sub(&self) -> Self {
        let mut new = self.clone();
        new.parent_type_variables = new.parent_type_variables.clone();

        new
    }

    fn new_var(&mut self, next_fresh_variable: &mut usize) -> Type {
        *next_fresh_variable += 1;
        self.type_variables.insert(*next_fresh_variable);
        Type::Var(*next_fresh_variable)
    }

    fn instantiate(&mut self, ty: Type, next_fresh_variable: &mut usize) -> Type {
        match ty {
            Type::Polymorphic { variables, mut matrix } => {
                for v in variables {
                    substitute_polymorphic_variables(v, &self.new_var(next_fresh_variable), &mut matrix);
                }
                *matrix
            },
            Type::PolymorphicVar(_) => unreachable!("a polymorphic variable should not be alone"),
            | Type::ParameterizedAtom {..}
            | Type::Fun(_, _) // all polymorphic types are in prenex form
            | Type::Atom(_)
            | Type::Var(_) => ty,
        }
    }

    fn unify(&mut self, mut constraints: Vec<(Type, Type)>, type_: &mut Type) {
        use Type::*;

        while let Some(c) = constraints.pop() {
            match c {
                (Atom(left), Atom(right)) if left == right => (),
                (Var(left), Var(right)) if left == right => (),
                (Fun(from1, to1), Fun(from2, to2)) => {
                    constraints.push((*from1, *from2));
                    constraints.push((*to1, *to2));
                }
                (
                    ParameterizedAtom {
                        name: name1,
                        parameters: parameters1,
                    },
                    ParameterizedAtom {
                        name: name2,
                        parameters: parameters2,
                    },
                ) if name1 == name2 => {
                    for (p1, p2) in parameters1.into_iter().zip(parameters2) {
                        constraints.push((p1, p2))
                    }
                }
                (Var(variable), value) | (value, Var(variable)) if !occurs_in(variable, &value) => {
                    for (left, right) in &mut constraints {
                        substitute(variable, &value, left);
                        substitute(variable, &value, right);
                    }
                    // apply substitution directly
                    // so we don't have to compute compositions of substitutions
                    substitute(variable, &value, type_);
                    for ty in self.variables.values_mut() {
                        substitute(variable, &value, ty);
                    }

                    self.type_variables.remove(&variable);
                }
                (left, right) => panic!("could not unify {left} with {right}"),
            }
        }
    }

    fn generalize(&mut self, constraints: Vec<(Type, Type)>, mut ty: Type) -> Type {
        fn vars(ty: &Type) -> Vec<usize> {
            match ty {
                Type::Atom(_) => vec![],
                Type::Var(v) => vec![*v],
                Type::Fun(from, to) => {
                    let mut from_vars = vars(from);
                    let mut to_vars = vars(to);

                    // potential duplicates!
                    from_vars.append(&mut to_vars);
                    from_vars
                }
                Type::ParameterizedAtom {
                    name: _,
                    parameters,
                } => {
                    let mut param_vars = vec![];

                    for p in parameters {
                        // potential duplicates!
                        param_vars.append(&mut vars(p));
                    }

                    param_vars
                }
                Type::Polymorphic { .. } | Type::PolymorphicVar(_) => {
                    unreachable!("type shouldn't be polymorphic after unification")
                }
            }
        }

        self.unify(constraints, &mut ty);

        let mut variables = vars(&ty);

        if variables.is_empty() {
            return ty;
        }

        variables.sort();
        variables.dedup();

        let mut next_poly_var = 0;
        for v in &variables {
            if !self.parent_type_variables.contains(v) {
                next_poly_var += 1;
                substitute(*v, &Type::PolymorphicVar(next_poly_var), &mut ty);
            }
        }

        let variables = (1..=variables.len()).collect();

        Type::Polymorphic {
            variables,
            matrix: Box::new(ty),
        }
    }


    fn build_constraints(
        &mut self,
        expr: &Expression,
        next_fresh_variable: &mut usize,
    ) -> ConstraintBuildingResult {
        match expr {
            Expression::LetIn { name, value, body } => {
                let ConstraintBuildingResult {
                    type_: value_type,
                    constraints: value_constraints,
                } = self.build_constraints(value, next_fresh_variable);
                let value_type = self.generalize(value_constraints.clone(), value_type);
                
                let mut with_var = self.sub();
                with_var.add_variable(name.to_string(), value_type);

                let ConstraintBuildingResult {
                    type_: body_type,
                    constraints: mut body_constraints,
                } = with_var.build_constraints(body, next_fresh_variable);

                let mut constraints = value_constraints;
                constraints.append(&mut body_constraints);

                ConstraintBuildingResult {
                    type_: body_type,
                    constraints,
                }
            }
            Expression::Fun {
                arg,
                body,
            } => {
                let mut with_arg = self.sub();
                let arg_type = with_arg.new_var(next_fresh_variable);

                with_arg.add_variable(arg.to_string(), arg_type.clone());

                let ConstraintBuildingResult {
                    type_: body_type,
                    constraints,
                } = with_arg.build_constraints(body, next_fresh_variable);

                let arg_type = Box::new(arg_type);
                let body_type = Box::new(body_type);

                ConstraintBuildingResult {
                    type_: Type::Fun(arg_type, body_type),
                    constraints,
                }
            }
            Expression::App { fun, arg } => {
                let result_type = self.new_var(next_fresh_variable);

                let ConstraintBuildingResult {
                    type_: fun_type,
                    constraints: fun_constraints,
                } = self.build_constraints(fun, next_fresh_variable);
                let ConstraintBuildingResult {
                    type_: arg_type,
                    constraints: mut arg_constraints,
                } = self.build_constraints(arg, next_fresh_variable);

                let mut constraints = fun_constraints;
                constraints.append(&mut arg_constraints);

                let result_type_boxed = Box::new(result_type.clone());
                let arg_type = Box::new(arg_type);

                constraints.push((fun_type, Type::Fun(arg_type, result_type_boxed)));

                ConstraintBuildingResult {
                    type_: result_type,
                    constraints,
                }
            }
            Expression::Variable(v) => match self.variables.get(v) {
                Some(ty) => ConstraintBuildingResult {
                    type_: self.instantiate(ty.clone(), next_fresh_variable),
                    constraints: vec![],
                },
                None => panic!("unknown variable {v}"),
            },
            Expression::StringLiteral(_) => ConstraintBuildingResult {
                type_: Type::Atom("string".to_string()),
                constraints: vec![],
            },
            Expression::IntLiteral(_) => ConstraintBuildingResult {
                type_: Type::Atom("int".to_string()),
                constraints: vec![],
            },
        }
    }

    fn infer_type(&mut self, expr: Expression) -> Type {
        let mut next_fresh_variable = 0;
        let ConstraintBuildingResult { type_, constraints } =
            self.build_constraints(&expr, &mut next_fresh_variable);

        let type_ = self.generalize(constraints, type_);

        // assert_eq!(self.type_variables, HashSet::new());

        self.type_variables.clear();

        type_
    }

    pub fn new() -> Self {
        Environment {
            variables: HashMap::new(),
            type_variables: HashSet::new(),
            parent_type_variables: HashSet::new(),
            type_atoms: HashMap::from([
                // primitive types
                ("int".to_string(), 0),
                ("string".to_string(), 0),
            ]),
        }
    }

    pub fn declaration(&mut self, decl: Declaration) {
        match decl {
            Declaration::Let { name, value } => {
                let type_ = self.infer_type(value);

                println!("{name}: {type_}");
                self.add_variable(name, type_);
            }
            Declaration::ExternLet { name, type_ } => {
                self.assert_is_valid(&type_);

                println!("extern {name}: {type_}");
                self.add_variable(name, type_);
            }
            Declaration::UnboundExpr(expr) => {
                let type_ = self.infer_type(expr);

                println!("_: {type_}");
            }
            Declaration::Type {
                name,
                parameter_count,
            } => {
                println!("type {name}");
                self.type_atoms.insert(name, parameter_count);
            }
        }
    }
}
