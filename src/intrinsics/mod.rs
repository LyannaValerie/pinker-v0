//! Família das intrínsecas: onde mora a autoridade declarativa do binding.
//!
//! Este `mod.rs` é a fronteira pública interna da família. As três autoridades
//! das intrínsecas históricas vivem aqui, e cada fato tem exatamente uma dona:
//!
//! ```text
//! registry        QUAL é o binding declarativo da grafia histórica
//! identity        QUEM é a identidade por trás da grafia, e a política de alias
//! public_surface  QUAL módulo built-in exporta cada membro público
//! ```
//!
//! Nenhuma das três hospeda implementação: os corpos continuam no interpretador
//! e no `pinker_rt`.

// @pinker-nav:start intrinsics.family.boundary
// @pinker-nav:domain intrinsics
// @pinker-nav:layer compiler
// @pinker-nav:summary Internal public boundary of the intrinsics family: it declares the three declarative authorities (registry, identity and public_surface) and records that none of them hosts an implementation, which remains in the interpreter and in pinker_rt.
pub mod identity;
pub mod public_surface;
pub mod registry;
// @pinker-nav:end intrinsics.family.boundary
