@echo off
setlocal enabledelayedexpansion

echo === AzuriteLang Installer Builder ===
echo.

REM Configuration - adjust these paths to your LLVM installation
set LLVM_DIR=C:\Program Files\LLVM
set INNO_SETUP="C:\Program Files (x86)\Inno Setup 6\ISCC.exe"

REM Step 1: Build release binary
echo [1/4] Building release binary...
cd /d "%~dp0.."
cargo build --release --features llvm
if %ERRORLEVEL% neq 0 (
    echo ERROR: Build failed!
    exit /b 1
)
echo Done.
echo.

REM Step 2: Copy LLVM runtime DLLs (needed for JIT)
echo [2/4] Copying LLVM runtime files...
if exist "%LLVM_DIR%\bin\LLVM-C.dll" (
    copy /Y "%LLVM_DIR%\bin\LLVM-C.dll" target\release\ > nul
    echo   LLVM-C.dll copied
) else (
    echo   WARNING: LLVM-C.dll not found at %LLVM_DIR%\bin
    echo   JIT execution will not work without it.
)

REM Copy clang for linking (optional, user can install separately)
if exist "%LLVM_DIR%\bin\clang.exe" (
    copy /Y "%LLVM_DIR%\bin\clang.exe" target\release\ > nul
    echo   clang.exe copied
) else (
    echo   WARNING: clang.exe not found - linking to .exe won't work.
)

if exist "%LLVM_DIR%\bin\lld.exe" (
    copy /Y "%LLVM_DIR%\bin\lld.exe" target\release\ > nul
    echo   lld.exe copied
)
echo.

REM Step 3: Check Inno Setup
echo [3/4] Checking Inno Setup...
if not exist %INNO_SETUP% (
    echo WARNING: Inno Setup not found at %INNO_SETUP%
    echo Download from https://jrsoftware.org/isdl.php
    echo Install it, then re-run this script.
    echo.
    echo Installer can still be built manually:
    echo   1. Open installer\setup.iss in Inno Setup
    echo   2. Click Build - Compile
    exit /b 1
)
echo Done.
echo.

REM Step 4: Compile installer
echo [4/4] Compiling installer...
cd installer
%INNO_SETUP% setup.iss
if %ERRORLEVEL% neq 0 (
    echo ERROR: Inno Setup compilation failed!
    exit /b 1
)
echo.
echo === Done! Installer created in installer\output\ ===
echo.
