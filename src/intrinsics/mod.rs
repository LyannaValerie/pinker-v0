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

// @pinker-nav:start intrinsecos.familia.fronteira
// @pinker-nav:domain intrinsecos
// @pinker-nav:layer compilador
// @pinker-nav:summary Fronteira publica interna da familia das intrinsecas: declara as tres autoridades declarativas (registry, identity e public_surface) e registra que nenhuma delas hospeda implementacao, que continua no interpretador e no pinker_rt.
pub mod identity;
pub mod public_surface;
pub mod registry;
// @pinker-nav:end intrinsecos.familia.fronteira
