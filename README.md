# tiped_lang

This is an implementation of type inference in a Hindley-Milner type system using algorithm W.
It can parse and type check full expressions without type annotations.
The syntax is based on OCaml's with a few differences.

### Usage

You can input expression in a REPL:
```shell
cargo run
```

You can also import a file to be type checked before entering the REPL:
```shell
cargo run -- test.tp
```

In the REPL expressions are terminated with a `;`, similar to `;;` in utop.

### Examples

Some examples from [test.tp](test.tp):

```rust

// Basics
#> let id = fun x -> x; 
id: ∀ 'a, 'a -> 'a

#> let id2 = (fun x -> x)(fun x -> x);
id2: ∀ 'a, 'a -> 'a

// Typing some equivalent to List.map

// list type (extern is some opaque value with the specified type)
type 'a list
  let nil = extern 'a list
  let cons = extern 'a -> 'a list -> 'a list

// type-equivalent of a match for a list
let match_list = extern 
  'a list ->                // list to match
  (unit -> 'b) ->           // nil case
  ('a -> 'a list -> 'b) ->  // cons case
  'b                        // result

// fixed-point combinator to avoid implementing recursion
// type taken from https://en.wikipedia.org/wiki/Fixed-point_combinator
let fix = extern (('a -> 'b) -> 'a -> 'b) -> 'a -> 'b

let map =
  let map_inner = fun rec f lst ->
    match_list (lst,
      fun -> nil,
      fun h t -> cons(f(h), rec(f, t)),
    )
  in fix(map_inner)

// Produces map: ∀ 'a 'b, ('b -> 'a) -> 'b list -> 'a list

```

### Architecture

- `src/main.rs`, `src/lexer.rs`, `src/parser.rs`: self-explanatory
- `src/tree.rs`: AST types and pretty printing
- `src/typer.rs`: the type inference algorithm

### Algorithm

The algorithm works roughly as follows: to type a given expression, the algorithm starts generating type equality constraints from the expression by introducing type variables and then solves this system of constraints using unification. If any unification variable was left unsolved, we promote it to a generalized variable.

### Ressources used (either directly or indirectly)

- https://cs3110.github.io/textbook/chapters/interp/inference.html
- https://en.wikipedia.org/wiki/Type_inference
- https://en.wikipedia.org/wiki/Unification_(computer_science)
- https://www.csd.uwo.ca/~mmorenom/cs2209_moreno/read/read6-unification.pdf
- https://docs.julialang.org/en/v1/devdocs/inference/
- https://langdev.stackexchange.com/questions/2424/when-do-we-need-complex-type-inference
