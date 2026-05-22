#[cfg(feature = "llvm")]
pub mod codegen;
#[cfg(feature = "llvm")]
pub use codegen::CodeGen;

#[cfg(not(feature = "llvm"))]
pub mod dummy;
#[cfg(not(feature = "llvm"))]
pub use dummy::CodeGen;
