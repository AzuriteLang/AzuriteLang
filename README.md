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
- **Classes** with fields, methods, and constructors
- **Enums** with data variants
- **Pattern matching** with `match` expressions
- **Arrays** with heap allocation
- **Generics, inheritance** *(coming soon)*

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs) 1.75+
- LLVM 22.1 SDK *(for full compilation)*

### Build

```bash
# Build the compiler
cargo build --release

# Run tests
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

> Without `--features llvm`, the compiler can still tokenize, parse, and type-check code.

## CLI Commands

| Command | Description |
|---|---|
| `azurite tokenize file.az` | Show tokens |
| `azurite parse file.az` | Show AST |
| `azurite check file.az` | Type-check with colored errors |
| `azurite build file.az` | Compile to `.exe` (requires `--features llvm`) |

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
// If-else
if x > 0 {
    print("positive")
} else {
    print("non-positive")
}

// While
while i < 10 {
    print(i)
    i = i + 1
}

// For
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

### Enums

```az
enum Color { Red, Green, Blue }
enum Option { Some(int), None }
```

### Match

```az
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
print(a, b, c)     // print any types
len(s)             // string length
sqrt(x)            // square root
abs(x)             // absolute value
int(f)             // float to int
float(i)           // int to float
read()             // read stdin
input(prompt)      // read with prompt
exit(code)         // exit program
```

## Project Structure

```
azurite_lexer/     # Lexer (tokenizer)
azurite_parser/    # Parser + AST (Pratt parser)
azurite_checker/   # Type checker + symbol table
azurite_codegen/   # LLVM IR codegen (feature-gated)
azurite_errors/    # Error messages with spans
azurite_cli/       # CLI (tokenize, parse, check, build)
azurite_test/      # 164 integration tests
```

## License

MIT
