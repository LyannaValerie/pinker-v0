//! Contrato puro e compartilhado de orçamento da memória pública.
//!
//! Este crate não realiza alocação. Runtime e interpretador consultam o mesmo
//! veredicto antes de qualquer efeito e só publicam o estado retornado depois
//! de seus efeitos falíveis próprios terem sido concluídos.

// @pinker-nav:start runtime.memoria.contrato-publico
// @pinker-nav:domain memoria
// @pinker-nav:layer runtime
// @pinker-nav:summary Autoridade pura compartilhada pelo interpretador e runtime nativo para arredondamento de página e quatro cotas independentes da memória pública — identidade e virtual vitalícios, reservado vivo recuperável e metadata histórica —, produzindo veredictos diagnósticos sem realizar efeitos nem publicar contadores parciais.

pub const PUBLIC_PAGE_BYTES: usize = 4096;
pub const MAX_PUBLIC_IDENTITIES: u64 = 1_000_000;
pub const MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_PUBLIC_SINGLE_RESERVED_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_PUBLIC_LIVE_RESERVED_BYTES: u64 = 256 * 1024 * 1024;

/// Unidade canônica de contabilidade de metadata por identidade histórica.
///
/// É deliberadamente independente do layout Rust de cada back-end: ambos
/// debitam a mesma unidade pública e podem manter representações internas
/// diferentes sem divergir no contrato observável.
pub const PUBLIC_METADATA_BYTES_PER_IDENTITY: u64 = 64;
pub const MAX_PUBLIC_METADATA_BYTES: u64 =
    MAX_PUBLIC_IDENTITIES * PUBLIC_METADATA_BYTES_PER_IDENTITY;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicMemoryBudget {
    pub identity_count: u64,
    pub lifetime_virtual_bytes: u64,
    pub live_reserved_bytes: u64,
    pub metadata_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicMemoryLimits {
    pub max_identities: u64,
    pub max_lifetime_virtual_bytes: u64,
    pub max_single_reserved_bytes: u64,
    pub max_live_reserved_bytes: u64,
    pub max_metadata_bytes: u64,
}

pub const PUBLIC_MEMORY_LIMITS: PublicMemoryLimits = PublicMemoryLimits {
    max_identities: MAX_PUBLIC_IDENTITIES,
    max_lifetime_virtual_bytes: MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES,
    max_single_reserved_bytes: MAX_PUBLIC_SINGLE_RESERVED_BYTES,
    max_live_reserved_bytes: MAX_PUBLIC_LIVE_RESERVED_BYTES,
    max_metadata_bytes: MAX_PUBLIC_METADATA_BYTES,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicAllocationReservation {
    pub logical_bytes: usize,
    pub reserved_page_bytes: usize,
    pub identity: u64,
    pub next_budget: PublicMemoryBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicAllocationVerdict {
    Allowed(PublicAllocationReservation),
    ZeroSize,
    PlatformWidthExceeded,
    AlignmentOverflow,
    SingleAllocationBudgetExceeded,
    LiveBytesBudgetExceeded,
    LifetimeVirtualBudgetExceeded,
    IdentityBudgetExceeded,
    MetadataBudgetExceeded,
    CounterOverflow,
    MappingFailure,
}

impl PublicAllocationVerdict {
    pub const fn diagnostic(self) -> Option<&'static str> {
        match self {
            Self::Allowed(_) => None,
            Self::ZeroSize => Some("E-RUNTIME-MEM-PUBLIC-ZERO: 'alocar' rejeita tamanho zero"),
            Self::PlatformWidthExceeded => {
                Some(
                    "E-RUNTIME-MEM-PUBLIC-WIDTH: 'alocar' excede o maior bloco representável pela plataforma (largura pública indisponível)",
                )
            }
            Self::AlignmentOverflow => {
                Some("E-RUNTIME-MEM-PUBLIC-ALIGN: overflow ao alinhar alocação pública")
            }
            Self::SingleAllocationBudgetExceeded => {
                Some("E-RUNTIME-MEM-PUBLIC-SINGLE-BUDGET: limite por alocação pública excedido")
            }
            Self::LiveBytesBudgetExceeded => {
                Some("E-RUNTIME-MEM-PUBLIC-LIVE-BUDGET: limite de bytes públicos vivos excedido")
            }
            Self::LifetimeVirtualBudgetExceeded => Some(
                "E-RUNTIME-MEM-PUBLIC-VIRTUAL-BUDGET: espaço virtual público vitalício esgotado",
            ),
            Self::IdentityBudgetExceeded => Some(
                "E-RUNTIME-MEM-PUBLIC-IDENTITY-BUDGET: limite de identidades públicas esgotado",
            ),
            Self::MetadataBudgetExceeded => {
                Some("E-RUNTIME-MEM-PUBLIC-METADATA-BUDGET: limite de metadata pública esgotado")
            }
            Self::CounterOverflow => {
                Some("E-RUNTIME-MEM-PUBLIC-COUNTER-OVERFLOW: overflow na contabilidade pública")
            }
            Self::MappingFailure => {
                Some("E-RUNTIME-MEM-PUBLIC-MAP: falha ao mapear memória pública")
            }
        }
    }
}

pub fn round_public_bytes(logical_bytes: usize) -> Option<usize> {
    logical_bytes
        .checked_add(PUBLIC_PAGE_BYTES - 1)
        .map(|value| value & !(PUBLIC_PAGE_BYTES - 1))
}

pub fn reserve_public_allocation(
    current: PublicMemoryBudget,
    requested_logical_bytes: u64,
    max_platform_logical_bytes: usize,
    limits: PublicMemoryLimits,
) -> PublicAllocationVerdict {
    if requested_logical_bytes == 0 {
        return PublicAllocationVerdict::ZeroSize;
    }
    let Ok(logical_bytes) = usize::try_from(requested_logical_bytes) else {
        return PublicAllocationVerdict::PlatformWidthExceeded;
    };
    if logical_bytes > max_platform_logical_bytes {
        return PublicAllocationVerdict::PlatformWidthExceeded;
    }
    let Some(reserved_page_bytes) = round_public_bytes(logical_bytes) else {
        return PublicAllocationVerdict::AlignmentOverflow;
    };
    let reserved = reserved_page_bytes as u64;
    if reserved > limits.max_single_reserved_bytes {
        return PublicAllocationVerdict::SingleAllocationBudgetExceeded;
    }

    let Some(identity_count) = current.identity_count.checked_add(1) else {
        return PublicAllocationVerdict::CounterOverflow;
    };
    let Some(lifetime_virtual_bytes) = current.lifetime_virtual_bytes.checked_add(reserved) else {
        return PublicAllocationVerdict::CounterOverflow;
    };
    let Some(live_reserved_bytes) = current.live_reserved_bytes.checked_add(reserved) else {
        return PublicAllocationVerdict::CounterOverflow;
    };
    let Some(metadata_bytes) = current
        .metadata_bytes
        .checked_add(PUBLIC_METADATA_BYTES_PER_IDENTITY)
    else {
        return PublicAllocationVerdict::CounterOverflow;
    };

    if live_reserved_bytes > limits.max_live_reserved_bytes {
        return PublicAllocationVerdict::LiveBytesBudgetExceeded;
    }
    if lifetime_virtual_bytes > limits.max_lifetime_virtual_bytes {
        return PublicAllocationVerdict::LifetimeVirtualBudgetExceeded;
    }
    if identity_count > limits.max_identities {
        return PublicAllocationVerdict::IdentityBudgetExceeded;
    }
    if metadata_bytes > limits.max_metadata_bytes {
        return PublicAllocationVerdict::MetadataBudgetExceeded;
    }

    PublicAllocationVerdict::Allowed(PublicAllocationReservation {
        logical_bytes,
        reserved_page_bytes,
        identity: identity_count,
        next_budget: PublicMemoryBudget {
            identity_count,
            lifetime_virtual_bytes,
            live_reserved_bytes,
            metadata_bytes,
        },
    })
}

pub fn release_public_live_bytes(
    current: PublicMemoryBudget,
    reserved_page_bytes: usize,
) -> Option<PublicMemoryBudget> {
    Some(PublicMemoryBudget {
        live_reserved_bytes: current
            .live_reserved_bytes
            .checked_sub(reserved_page_bytes as u64)?,
        ..current
    })
}

// @pinker-nav:end runtime.memoria.contrato-publico

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(
        current: PublicMemoryBudget,
        requested: u64,
        limits: PublicMemoryLimits,
    ) -> PublicAllocationReservation {
        match reserve_public_allocation(current, requested, usize::MAX, limits) {
            PublicAllocationVerdict::Allowed(reservation) => reservation,
            other => panic!("esperava concessão, recebeu {other:?}"),
        }
    }

    #[test]
    fn arredondamento_publico_e_por_pagina() {
        assert_eq!(round_public_bytes(1), Some(4096));
        assert_eq!(round_public_bytes(4095), Some(4096));
        assert_eq!(round_public_bytes(4096), Some(4096));
        assert_eq!(round_public_bytes(4097), Some(8192));
    }

    #[test]
    fn zero_largura_e_overflow_de_alinhamento_sao_distintos() {
        assert_eq!(
            reserve_public_allocation(
                PublicMemoryBudget::default(),
                0,
                usize::MAX,
                PUBLIC_MEMORY_LIMITS
            ),
            PublicAllocationVerdict::ZeroSize
        );
        assert_eq!(
            reserve_public_allocation(
                PublicMemoryBudget::default(),
                1025,
                1024,
                PUBLIC_MEMORY_LIMITS
            ),
            PublicAllocationVerdict::PlatformWidthExceeded
        );
        if usize::BITS == 64 {
            let mut limits = PUBLIC_MEMORY_LIMITS;
            limits.max_single_reserved_bytes = u64::MAX;
            assert_eq!(
                reserve_public_allocation(
                    PublicMemoryBudget::default(),
                    u64::MAX,
                    usize::MAX,
                    limits
                ),
                PublicAllocationVerdict::AlignmentOverflow
            );
        }
    }

    #[test]
    fn fronteira_do_limite_individual() {
        assert_eq!(
            allowed(
                PublicMemoryBudget::default(),
                MAX_PUBLIC_SINGLE_RESERVED_BYTES,
                PUBLIC_MEMORY_LIMITS
            )
            .reserved_page_bytes as u64,
            MAX_PUBLIC_SINGLE_RESERVED_BYTES
        );
        assert_eq!(
            reserve_public_allocation(
                PublicMemoryBudget::default(),
                MAX_PUBLIC_SINGLE_RESERVED_BYTES + 1,
                usize::MAX,
                PUBLIC_MEMORY_LIMITS
            ),
            PublicAllocationVerdict::SingleAllocationBudgetExceeded
        );
    }

    #[test]
    fn fronteira_de_bytes_vivos() {
        let current = PublicMemoryBudget {
            live_reserved_bytes: MAX_PUBLIC_LIVE_RESERVED_BYTES - PUBLIC_PAGE_BYTES as u64,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            allowed(current, 1, PUBLIC_MEMORY_LIMITS)
                .next_budget
                .live_reserved_bytes,
            MAX_PUBLIC_LIVE_RESERVED_BYTES
        );
        let current = PublicMemoryBudget {
            live_reserved_bytes: MAX_PUBLIC_LIVE_RESERVED_BYTES,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            reserve_public_allocation(current, 1, usize::MAX, PUBLIC_MEMORY_LIMITS),
            PublicAllocationVerdict::LiveBytesBudgetExceeded
        );
    }

    #[test]
    fn fronteira_virtual_vitalicia() {
        let current = PublicMemoryBudget {
            lifetime_virtual_bytes: MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES - PUBLIC_PAGE_BYTES as u64,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            allowed(current, 1, PUBLIC_MEMORY_LIMITS)
                .next_budget
                .lifetime_virtual_bytes,
            MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES
        );
        let current = PublicMemoryBudget {
            lifetime_virtual_bytes: MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            reserve_public_allocation(current, 1, usize::MAX, PUBLIC_MEMORY_LIMITS),
            PublicAllocationVerdict::LifetimeVirtualBudgetExceeded
        );
    }

    #[test]
    fn fronteiras_de_identidade_e_metadata() {
        let identity_current = PublicMemoryBudget {
            identity_count: MAX_PUBLIC_IDENTITIES - 1,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            allowed(identity_current, 1, PUBLIC_MEMORY_LIMITS)
                .next_budget
                .identity_count,
            MAX_PUBLIC_IDENTITIES
        );
        let identity_current = PublicMemoryBudget {
            identity_count: MAX_PUBLIC_IDENTITIES,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            reserve_public_allocation(identity_current, 1, usize::MAX, PUBLIC_MEMORY_LIMITS),
            PublicAllocationVerdict::IdentityBudgetExceeded
        );

        let metadata_current = PublicMemoryBudget {
            metadata_bytes: MAX_PUBLIC_METADATA_BYTES - PUBLIC_METADATA_BYTES_PER_IDENTITY,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            allowed(metadata_current, 1, PUBLIC_MEMORY_LIMITS)
                .next_budget
                .metadata_bytes,
            MAX_PUBLIC_METADATA_BYTES
        );
        let metadata_current = PublicMemoryBudget {
            metadata_bytes: MAX_PUBLIC_METADATA_BYTES,
            ..PublicMemoryBudget::default()
        };
        assert_eq!(
            reserve_public_allocation(metadata_current, 1, usize::MAX, PUBLIC_MEMORY_LIMITS),
            PublicAllocationVerdict::MetadataBudgetExceeded
        );
    }

    #[test]
    fn overflow_de_contador_nao_publica_estado_parcial() {
        let limits = PublicMemoryLimits {
            max_identities: u64::MAX,
            max_lifetime_virtual_bytes: u64::MAX,
            max_single_reserved_bytes: u64::MAX,
            max_live_reserved_bytes: u64::MAX,
            max_metadata_bytes: u64::MAX,
        };
        for current in [
            PublicMemoryBudget {
                identity_count: u64::MAX,
                ..PublicMemoryBudget::default()
            },
            PublicMemoryBudget {
                lifetime_virtual_bytes: u64::MAX,
                ..PublicMemoryBudget::default()
            },
            PublicMemoryBudget {
                live_reserved_bytes: u64::MAX,
                ..PublicMemoryBudget::default()
            },
            PublicMemoryBudget {
                metadata_bytes: u64::MAX,
                ..PublicMemoryBudget::default()
            },
        ] {
            let before = current;
            assert_eq!(
                reserve_public_allocation(current, 1, usize::MAX, limits),
                PublicAllocationVerdict::CounterOverflow
            );
            assert_eq!(current, before);
        }
    }

    #[test]
    fn liberar_recupera_somente_bytes_vivos() {
        let reservation = allowed(
            PublicMemoryBudget::default(),
            PUBLIC_PAGE_BYTES as u64 + 1,
            PUBLIC_MEMORY_LIMITS,
        );
        let after =
            release_public_live_bytes(reservation.next_budget, reservation.reserved_page_bytes)
                .expect("budget vivo consistente");
        assert_eq!(after.live_reserved_bytes, 0);
        assert_eq!(after.identity_count, 1);
        assert_eq!(after.lifetime_virtual_bytes, 2 * PUBLIC_PAGE_BYTES as u64);
        assert_eq!(after.metadata_bytes, PUBLIC_METADATA_BYTES_PER_IDENTITY);
        assert!(release_public_live_bytes(after, PUBLIC_PAGE_BYTES).is_none());
    }

    #[test]
    fn recusas_possuem_diagnosticos_publicos_distintos() {
        let verdicts = [
            PublicAllocationVerdict::SingleAllocationBudgetExceeded,
            PublicAllocationVerdict::LiveBytesBudgetExceeded,
            PublicAllocationVerdict::LifetimeVirtualBudgetExceeded,
            PublicAllocationVerdict::IdentityBudgetExceeded,
            PublicAllocationVerdict::MetadataBudgetExceeded,
            PublicAllocationVerdict::MappingFailure,
            PublicAllocationVerdict::CounterOverflow,
        ];
        let diagnostics: std::collections::BTreeSet<_> = verdicts
            .into_iter()
            .map(|verdict| verdict.diagnostic().expect("diagnóstico"))
            .collect();
        assert_eq!(diagnostics.len(), verdicts.len());
    }
}
