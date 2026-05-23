# AzuriteLang Installer

Builds a Windows `.exe` installer using [Inno Setup](https://jrsoftware.org/isdl.php).

## Prerequisites

- Windows
- [Inno Setup 6+](https://jrsoftware.org/isdl.php)
- LLVM 22.1 SDK (for LLVM-C.dll and clang.exe)

## Build

```bat
installer\build-installer.bat
```

Or manually:
1. `cargo build --release --features llvm`
2. Copy `LLVM-C.dll`, `clang.exe`, `lld.exe` from your LLVM `bin\` to `target\release\`
3. Open `installer\setup.iss` in Inno Setup → **Build → Compile**

## Output

`installer\output\AzuriteLang-0.1.0-setup.exe`

## What it installs

- `azurite.exe` (compiler)
- `LLVM-C.dll` (JIT runtime)
- `clang.exe` + `lld.exe` (linking)
- `.az` file association
- PATH registration
- Start menu shortcuts (REPL, Docs)
- Example `main.az` file
- VS Code extension (optional)
