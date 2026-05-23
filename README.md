# AzuriteLang

A compiled programming language with Python-like syntax and LLVM backend. Written in Rust.

```az
func main() {
    let msg = "Hello, Azurite!"
    print(msg)
}
```

## Features

- **Python-like syntax** with `{}` blocks, no indentation rules
- **Strong static typing** with type inference
- **LLVM backend** generates native executables
- **Classes** with fields, methods, constructors, and inheritance
- **Enums** with data variants and pattern matching (`match`)
- **Arrays** with heap allocation
- **Generics** (generic classes)
- **Package manager** — git dependencies with `azurite.toml`
- **Standard library** — `string`, `math`, `random`, `color` libs on GitHub

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs) 1.75+
- LLVM 22.1 SDK *(for full compilation)*

### Build

```bash
# Build the compiler
cargo build --release

# Run tests (270+)
cargo test
```

### Compile with LLVM backend

```bash
# Point to your LLVM SDK
$env:LLVM_SYS_221_PREFIX = "C:\LLVM-22.1"

# Build with LLVM support
cargo build --release --features llvm

# Compile .az to .exe
cargo run --features llvm -- build hello.az
./hello.exe
```

> Without `--features llvm`, the compiler can still parse and type-check code.

## CLI Commands

| Command | Description |
|---|---|---|
| `azurite check file.az` | Type-check with colored errors |
| `azurite build file.az` | Compile to `.exe` (requires `llvm` feature) |
| `azurite repl` | Interactive REPL |
| `azurite init [dir]` | Create a new project with `azurite.toml` |
| `azurite install <name>` | Add a dependency from registry or custom (`--git`, `--path`) |

## Package Manager

Azurite uses a Cargo-inspired manifest + cache system for dependencies.

### `azurite.toml`

```toml
[package]
name = "my-project"
version = "0.1.0"

[dependencies]
string = { git = "https://github.com/AzuriteLang/string" }
local  = { path = "../my-lib" }
```

### Dependency types

| Field | Description |
|---|---|
| `git` | GitHub (or any git) repository URL |
| `path` | Local filesystem path (absolute or relative) |
| `rev` | Optional git revision (tag, commit, branch) |

### Resolution

1. `azurite check/build` finds `azurite.toml` (walks up directories)
2. **Git deps** → cloned to `~/.azurite/cache/<name>/` (shallow `--depth 1`)
3. **Path deps** → resolved relative to the manifest location
4. All imported code is inlined recursively before type-checking

### `azurite install`

```bash
azurite install string                  # from built-in registry
azurite install math --git https://...  # custom git URL
azurite install mylib --path ./libs     # local path
```

The registry currently knows: `string`, `math`, `random`, `color`.

## Imports

```az
import "string"
import "mylib"

func main() {
    print(contains("hello", "ell"))    // 1 (true)
    print(to_upper("azurite"))          // AZURITE
}
```

- **Named imports** resolve to dependencies listed in `azurite.toml`
- **File paths** (`import "math.az"`) resolve relative to the current file
- Convention: each dependency exposes `src/lib.az` or `main.az`

## Standard Library

### `string` — [`github.com/AzuriteLang/string`](https://github.com/AzuriteLang/string)

```toml
[dependencies]
string = { git = "https://github.com/AzuriteLang/string" }
```

| Category | Functions |
|---|---|
| **Core** | `is_empty`, `first`, `last`, `is_digit_char`, `is_letter_char`, `is_whitespace_char` |
| **Search** | `contains`, `starts_with`, `ends_with`, `index_of`, `count` |
| **Pad** | `repeat`, `pad_left`, `pad_right`, `pad_left_with`, `pad_right_with`, `zfill`, `center` |
| **Parse** | `to_int`, `equals_ignore_case` |
| **Transform** | `to_upper`, `to_lower`, `reverse`, `trim`, `trim_start`, `trim_end`, `replace`, `substring` |
| **Check** | `is_upper`, `is_lower`, `is_digit`, `is_letter`, `is_alnum`, `is_whitespace` |

### `math` — [`github.com/AzuriteLang/math`](https://github.com/AzuriteLang/math)

```toml
[dependencies]
math = { git = "https://github.com/AzuriteLang/math" }
```

| Category | Functions |
|---|---|
| **Constants** | `pi()` |
| **Conversion** | `deg_to_rad`, `rad_to_deg` |
| **Rounding** | `round`, `floor`, `ceil` |
| **Clamp** | `clamp_int`, `clamp_float`, `min_int`, `max_int`, `min_float`, `max_float` |
| **Interpolation** | `lerp` |
| **Sign** | `sign_int` |

### `random` — [`github.com/AzuriteLang/random`](https://github.com/AzuriteLang/random)

```toml
[dependencies]
random = { git = "https://github.com/AzuriteLang/random" }
```

| Function | Description |
|---|---|
| `seed(n)` | Seed the RNG |
| `random_int()` | Random integer |
| `random_float()` | Float in `[0, 1)` |
| `random_bool()` | `true` or `false` |
| `random_range(lo, hi)` | Integer in `[lo, hi]` |
| `roll_dice(sides)` | `random_range(1, sides)` |

### `color` — [`github.com/AzuriteLang/color`](https://github.com/AzuriteLang/color)

```toml
[dependencies]
color = { git = "https://github.com/AzuriteLang/color" }
```

| Category | Functions |
|---|---|
| **Class** | `Color{r,g,b}` with `.to_hex()`, `.brightness()`, `.lerp()`, `.blend()`, `.fg()`, `.bg()` |
| **Parse** | `from_hex(hex)` |
| **Constants** | `red()`, `green()`, `blue()`, `white()`, `black()`, `yellow()`, `cyan()`, `magenta()`, `orange()`, `pink()` |
| **ANSI styles** | `reset_color()`, `bold()`, `dim()`, `italic()`, `underline()` |
| **ANSI colors** | `fg(r,g,b)`, `bg(r,g,b)`, `fg_hex(hex)`, `bg_hex(hex)` |

Built-in math functions (no import needed): `sqrt`, `abs`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `exp`, `expm1`, `log`, `log2`, `log10`, `pow`, `hypot`, `fmod`, `copysign`, `floor`, `ceil`.

## Language Syntax

### Variables

```az
let x: int = 42        // explicit type
let y = 10              // inferred type
let name = "Azurite"    // string
let flag = true         // bool
x = 99                  // reassignment
```

### Functions

```az
func add(a: int, b: int) -> int {
    return a + b
}

func greet(name: string) {
    print("Hello, ", name)
}
```

### Control Flow

```az
if x > 0 {
    print("positive")
} else {
    print("non-positive")
}

while i < 10 {
    print(i)
    i = i + 1
}

for i in 0..10 {
    print(i)
}
```

### Classes

```az
class Person {
    name: string
    age: int

    func new(name: string, age: int) {}

    func greet(self) {
        print("Hi, I'm ", self.name)
    }
}

let p = Person.new("Alice", 30)
p.greet()
```

### Enums + Match

```az
enum Color { Red, Green, Blue }
enum Option { Some(int), None }

match x {
    1 => print("one")
    2 => print("two")
    _ => print("other")
}
```

### Arrays

```az
let arr = [10, 20, 30]
print(arr[0])
arr[1] = 99
```

### Built-in Functions

```az
print(a, b, c)     // print any types (varargs)
len(s)             // string length
chr(n)             // int ASCII code → 1-char string
sqrt(x)            // square root
abs(x)             // absolute value (int)
int(f)             // float to int
float(i)           // int to float
char_at(s, i)      // char code at position
sin(x)             // sine (float)
cos(x)             // cosine (float)
tan(x)             // tangent (float)
pow(x, y)          // power (float)
log(x)             // natural log (float)
log10(x)           // base-10 log (float)
floor(x)           // round down (float)
ceil(x)            // round up (float)
read()             // read stdin
input(prompt)      // read with prompt
rand()             // random integer (C rand)
srand(seed)        // seed the RNG
exit(code)         // exit program
```

## Project Structure

```
azurite_lexer/      # Lexer (tokenizer)
azurite_parser/     # Parser + AST (Pratt parser)
azurite_checker/    # Type checker + symbol table
azurite_codegen/    # LLVM IR codegen (feature-gated)
azurite_errors/     # Error messages with spans
azurite_resolver/   # Package/dependency resolver (azurite.toml)
azurite_cli/        # CLI (check, build, repl, init)
azurite_test/       # 270+ integration tests
```

## License

MIT
