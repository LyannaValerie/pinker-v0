//! Sonda temporária (Task #505). NÃO faz parte do diff final.
mod common;
use common::parse;
use pinker_v0::intrinsic_authority::{
    all_canonical_intrinsic_spellings, all_public_intrinsic_members,
    canonical_public_intrinsic_spelling,
};
use std::collections::BTreeSet;

#[test]
fn dump() {
    let mut cand: BTreeSet<String> = all_canonical_intrinsic_spellings()
        .into_iter()
        .map(|e| e.spelling.to_string())
        .collect();
    for m in all_public_intrinsic_members() {
        cand.insert(m.member.to_string());
    }
    for g in &cand {
        let fonte = format!("pacote main;\ncarinho principal() -> bombom {{ mimo {g}(); }}\n");
        match parse(&fonte) {
            Ok(_) => println!("SURVIVES\t{g}"),
            Err(e) => {
                let msg = format!("{e:?}");
                if !msg.contains("não está no escopo") {
                    println!("OTHER\t{g}\t{}", msg.replace('\n', " "));
                }
            }
        }
    }
    println!("--- membro cuja grafia e canonica de outra identidade ---");
    for m in all_public_intrinsic_members() {
        if let Some(h) = canonical_public_intrinsic_spelling(m.member) {
            if format!("{:?}", h.identity) != format!("{:?}", m.identity) {
                println!(
                    "SHADOW\t{}.{}\tcanonica_de={:?}",
                    m.module, m.member, h.identity
                );
            }
        }
    }
}
