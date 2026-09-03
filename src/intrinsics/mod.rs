//! Família das intrínsecas: onde mora a autoridade declarativa do binding.
//!
//! Este `mod.rs` é a fronteira pública interna da família. Hoje ela contém a
//! autoridade criada pela consolidação C1:
//!
//! ```text
//! registry  QUAL é o binding declarativo da grafia histórica
//! ```
//!
//! As outras duas autoridades da família continuam fora do diretório por um
//! motivo mecânico, não por desenho: renomear `intrinsic_authority` (identidade)
//! ou `familia_superficie` (superfície pública) muda o hash de regiões
//! `@pinker-nav` que projeções `FROZEN` reconstroem sem regra de override, e o
//! contrato de projeções não autoriza recalibrar medida congelada. A relocação
//! está reportada como achado estrutural; a autoridade, que é o que C1 consolida,
//! já está aqui.
//!
//! Nenhuma das três hospeda implementação: os corpos continuam no interpretador
//! e no `pinker_rt`.

pub mod registry;
