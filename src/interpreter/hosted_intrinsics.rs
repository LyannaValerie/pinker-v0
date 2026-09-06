use super::*;

// @pinker-nav:start interpreter.intrinsecos.despacho-hospedado
// @pinker-nav:domain intrinsecos
// @pinker-nav:layer interpreter
// @pinker-nav:summary Roteia o despacho hospedado a partir de identidade de callee já resolvida, encaminhando para autoridades hospedadas já existentes: consulta a autoridade de mapas genéricos, executa os acessores nominais de SaidaProcesso e ValorJson e encaminha alocar e liberar ao estado endereçável do interpretador, além dos braços genéricos anteriores à região de aleatoriedade. Roteia identidade intrínseca já resolvida e não detém autoridade do registry: a superfície histórica de intrínsecas permanece de C1, e este prefixo não implementa aleatoriedade nem substitui as autoridades de identidade, JSON, processo ou memória.
/// #532 — porta única do despacho intrínseco do interpretador.
///
/// ```text
/// CALL_IS_INTRINSIC <- RESOLVED_IDENTITY, NOT SPELLING
/// ```
///
/// A tabela abaixo continua endereçada pela grafia canônica — é ela que escolhe
/// QUAL intrínseca —, mas o portão é a identidade. Antes, qualquer callee cuja
/// grafia casasse era atendido aqui, e por isso a grafia canônica precisava ser
/// reservada contra declaração do usuário.
// A identidade entrou como primeiro parâmetro justamente para ser lida antes de
// qualquer estado: ela decide SE a tabela abaixo responde. Agrupar os cinco
// estados de runtime numa struct só para caber no limite moveria a fronteira de
// ownership do interpretador sem melhorar nada aqui.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_call_intrinsic(
    identidade: crate::intrinsics::identity::CalleeIdentity,
    callee: &str,
    args: &[RuntimeValue],
    public_memory_state: &mut PublicMemoryState,
    io_state: &mut RuntimeIoState,
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
) -> Result<IntrinsicCall, PinkerError> {
    if identidade.is_user() {
        return Ok(IntrinsicCall::NotIntrinsic);
    }
    if let Some(result) = try_call_map_intrinsic_authority(callee, args, map_state) {
        return result;
    }
    match callee {
        nome if crate::saida_processo::e_acessor(nome) => {
            if args.len() != 1 {
                return Err(runtime_err(&format!(
                    "intrínseca '{nome}' exige 1 argumento (SaidaProcesso)"
                )));
            }
            let RuntimeValue::SaidaProcesso(handle) = args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{nome}' exige SaidaProcesso"
                )));
            };
            let saida = map_state
                .saidas_processo
                .obter(handle)
                .ok_or_else(|| runtime_err("handle SaidaProcesso inválido"))?;
            let valor = match nome {
                crate::saida_processo::ACESSOR_CODIGO => RuntimeValue::Int(saida.codigo()),
                crate::saida_processo::ACESSOR_SAIDA => {
                    RuntimeValue::Str(saida.saida().to_string())
                }
                crate::saida_processo::ACESSOR_ERRO => RuntimeValue::Str(saida.erro().to_string()),
                _ => unreachable!(),
            };
            Ok(IntrinsicCall::Done(Some(valor)))
        }
        // Parte E1 — acessores da árvore JSON.
        //
        // Todos atravessam a MESMA arena por handle: `json_lista_obter` e
        // `json_objeto_obter` devolvem `ValorJson`, então qualquer profundidade
        // é alcançada sem helper por formato.
        //
        // Tag errada aqui é erro de programa, não dado externo malformado: o
        // documento já foi aceito pela superfície falível. As três categorias
        // permanecem separadas.
        nome if crate::valor_json::e_acessor(nome) => {
            use crate::valor_json::intrinsecas as ji;
            use crate::valor_json::{NoJson, TipoJson};

            let esperado_args =
                if matches!(nome, ji::LISTA_OBTER | ji::OBJETO_OBTER | ji::OBJETO_TEM) {
                    2
                } else {
                    1
                };
            if args.len() != esperado_args {
                return Err(runtime_err(&format!(
                    "intrínseca '{nome}' exige {esperado_args} argumento(s)"
                )));
            }
            let RuntimeValue::ValorJson(handle) = args[0] else {
                return Err(runtime_err(&format!("intrínseca '{nome}' exige ValorJson")));
            };
            let no = map_state
                .valores_json
                .obter(handle)
                .ok_or_else(|| runtime_err("handle ValorJson inválido"))?;

            let erro_tipo = |esperado: &str| {
                runtime_err(&format!(
                    "intrínseca '{nome}' exige valor JSON do tipo {esperado}"
                ))
            };

            let valor = match nome {
                ji::EMITIR => {
                    let texto = crate::valor_json::serializar(handle, &map_state.valores_json)
                        .map_err(|causa| runtime_err(&causa))?;
                    RuntimeValue::Str(texto)
                }
                ji::TIPO => RuntimeValue::Int(no.tipo().discriminante()),
                ji::VERSO => match no {
                    NoJson::Verso(texto) => RuntimeValue::Str(texto.clone()),
                    _ => return Err(erro_tipo(TipoJson::Verso.nome())),
                },
                ji::NUMERO => match no {
                    NoJson::Numero(valor) => RuntimeValue::IntSigned(*valor),
                    _ => return Err(erro_tipo(TipoJson::Numero.nome())),
                },
                ji::LOGICA => match no {
                    NoJson::Logica(valor) => RuntimeValue::Bool(*valor),
                    _ => return Err(erro_tipo(TipoJson::Logica.nome())),
                },
                ji::LISTA_TAMANHO => match no {
                    NoJson::Lista(itens) => RuntimeValue::Int(itens.len() as u64),
                    _ => return Err(erro_tipo(TipoJson::Lista.nome())),
                },
                ji::LISTA_OBTER => {
                    let NoJson::Lista(itens) = no else {
                        return Err(erro_tipo(TipoJson::Lista.nome()));
                    };
                    let RuntimeValue::Int(indice) = args[1] else {
                        return Err(runtime_err(&format!(
                            "intrínseca '{nome}' exige índice em bombom"
                        )));
                    };
                    let item = itens.get(indice as usize).copied().ok_or_else(|| {
                        runtime_err(&format!("índice {indice} fora da faixa em '{nome}'"))
                    })?;
                    RuntimeValue::ValorJson(item)
                }
                ji::OBJETO_TAMANHO => match no {
                    NoJson::Objeto(membros) => RuntimeValue::Int(membros.len() as u64),
                    _ => return Err(erro_tipo(TipoJson::Objeto.nome())),
                },
                ji::OBJETO_TEM => {
                    let NoJson::Objeto(membros) = no else {
                        return Err(erro_tipo(TipoJson::Objeto.nome()));
                    };
                    let RuntimeValue::Str(chave) = &args[1] else {
                        return Err(runtime_err(&format!(
                            "intrínseca '{nome}' exige chave em verso"
                        )));
                    };
                    RuntimeValue::Bool(membros.contains_key(chave.as_str()))
                }
                ji::OBJETO_OBTER => {
                    let NoJson::Objeto(membros) = no else {
                        return Err(erro_tipo(TipoJson::Objeto.nome()));
                    };
                    let RuntimeValue::Str(chave) = &args[1] else {
                        return Err(runtime_err(&format!(
                            "intrínseca '{nome}' exige chave em verso"
                        )));
                    };
                    let filho = membros
                        .get(chave.as_str())
                        .copied()
                        .ok_or_else(|| runtime_err(&format!("chave ausente em '{nome}'")))?;
                    RuntimeValue::ValorJson(filho)
                }
                ji::OBJETO_CHAVES => {
                    let NoJson::Objeto(membros) = no else {
                        return Err(erro_tipo(TipoJson::Objeto.nome()));
                    };
                    // BTreeMap: a ordem é de chave por construção, não de
                    // iteração acidental.
                    let chaves: Vec<String> = membros.keys().cloned().collect();
                    let lista = list_state.next_list_handle;
                    list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
                    list_state.lists_verso.insert(lista, chaves);
                    RuntimeValue::ListVerso(lista)
                }
                _ => unreachable!("acessor JSON sem implementação"),
            };
            Ok(IntrinsicCall::Done(Some(valor)))
        }
        "alocar" => public_memory_allocate(args, public_memory_state),
        "liberar" => public_memory_free(args, public_memory_state),
        // @pinker-nav:end interpreter.intrinsecos.despacho-hospedado
        // @pinker-nav:start interpreter.intrinsecos.acaso
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa intrínsecas hospedadas de aleatoriedade inicial, validando aridade, semente e handle de gerador, mutando o estado pseudoaleatório do interpretador e retornando handles ou números; não representa geradores do runtime nativo.
        "aleatorio_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_criar' exige 1 argumento (semente bombom)",
                ));
            }
            let RuntimeValue::Int(seed) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_criar' exige semente bombom",
                ));
            };
            let handle = random_state.next_generator_handle;
            random_state.next_generator_handle =
                random_state.next_generator_handle.saturating_add(1);
            random_state
                .generators
                .insert(handle, RuntimeRandomGenerator { state: seed });
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "aleatorio_proximo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_proximo' exige 1 argumento (gerador bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_proximo' exige gerador bombom",
                ));
            };
            let Some(generator) = random_state.generators.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de aleatoriedade inválido em 'aleatorio_proximo'",
                ));
            };
            let next = advance_random_generator(&mut generator.state);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(next))))
        }
        // @pinker-nav:end interpreter.intrinsecos.acaso

        // @pinker-nav:start interpreter.intrinsecos.listas
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa operações hospedadas contíguas de listas de bombom e verso, criando handles tipados, anexando, obtendo, medindo, definindo, removendo e inserindo elementos com validação dinâmica de aridade, índice, handle e tipo; os handles pertencem ao estado do interpretador.
        "lista_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = list_state.next_list_handle;
            list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
            list_state.lists_bombom.insert(handle, Vec::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListBombom(handle))))
        }
        "lista_bombom_anexar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige 2 argumentos (lista<bombom>, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            lista.push(value);
            Ok(IntrinsicCall::Done(None))
        }
        "lista_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige 2 argumentos (lista<bombom>, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(value) = lista.get(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_bombom_obter'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "lista_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tamanho' exige 1 argumento (lista<bombom>)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tamanho' exige lista<bombom> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                lista.len() as u64
            ))))
        }
        "lista_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige 3 argumentos (lista<bombom>, bombom, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(slot) = lista.get_mut(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_bombom_definir'",
                ));
            };
            *slot = value;
            Ok(IntrinsicCall::Done(None))
        }
        "lista_bombom_tirar_ultimo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tirar_ultimo' exige 1 argumento (lista<bombom>)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tirar_ultimo' exige lista<bombom> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(value) = lista.pop() else {
                return Err(runtime_err("lista vazia em 'lista_bombom_tirar_ultimo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(value))))
        }
        "lista_verso_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_criar' exige 0 argumentos",
                ));
            }
            let handle = list_state.next_list_handle;
            list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
            list_state.lists_verso.insert(handle, Vec::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListVerso(handle))))
        }
        "lista_verso_anexar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_anexar' exige 2 argumentos (lista<verso>, verso)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_anexar' exige lista<verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(value) = args[1].clone() else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_anexar' exige verso no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            lista.push(value);
            Ok(IntrinsicCall::Done(None))
        }
        "lista_verso_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_obter' exige 2 argumentos (lista<verso>, bombom)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_obter' exige lista<verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            let Some(value) = lista.get(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_verso_obter'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        "lista_verso_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tamanho' exige 1 argumento (lista<verso>)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tamanho' exige lista<verso> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                lista.len() as u64
            ))))
        }
        "lista_verso_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige 3 argumentos (lista<verso>, bombom, verso)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige lista<verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Str(value) = args[2].clone() else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_definir' exige verso no terceiro argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            let Some(slot) = lista.get_mut(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_verso_definir'",
                ));
            };
            *slot = value;
            Ok(IntrinsicCall::Done(None))
        }
        "lista_verso_tirar_ultimo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tirar_ultimo' exige 1 argumento (lista<verso>)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_tirar_ultimo' exige lista<verso> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_verso.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<verso> inválido em runtime"));
            };
            let Some(value) = lista.pop() else {
                return Err(runtime_err("lista vazia em 'lista_verso_tirar_ultimo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        "lista_verso_inserir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige 3 argumentos (lista, índice bombom, valor verso)",
                ));
            }
            let RuntimeValue::ListVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige lista<verso>",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige índice bombom",
                ));
            };
            let RuntimeValue::Str(valor) = args[2].clone() else {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir' exige valor verso",
                ));
            };
            let lista = list_state
                .lists_verso
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("intrínseca 'lista_verso_inserir': lista inválida"))?;
            let idx = index as usize;
            if idx > lista.len() {
                return Err(runtime_err(
                    "intrínseca 'lista_verso_inserir': índice fora dos limites",
                ));
            }
            lista.insert(idx, valor);
            Ok(IntrinsicCall::Done(None))
        }
        // @pinker-nav:end interpreter.intrinsecos.listas

        // Autoridade hospedada genérica: K escolhe somente a igualdade; V é
        // preservado como RuntimeValue completo, sem enumeração K × V.
        "__pinker_internal_mapa_criar_chave_bombom"
        | "__pinker_internal_mapa_criar_chave_verso" => {
            if !args.is_empty() {
                return Err(runtime_err("mapa_criar exige 0 argumentos"));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps.insert(
                handle,
                RuntimeGenericMap {
                    key_is_verso: callee.ends_with("_verso"),
                    entries: Vec::new(),
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Map(handle))))
        }
        "__pinker_internal_mapa_definir" => {
            if args.len() != 3 {
                return Err(runtime_err("mapa_definir exige 3 argumentos"));
            }
            let RuntimeValue::Map(handle) = args[0] else {
                return Err(runtime_err("mapa_definir exige mapa genérico"));
            };
            let key_is_verso = map_state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?
                .key_is_verso;
            let key = generic_map_key(&args[1], key_is_verso)?;
            let value = args[2].clone();
            let map = map_state
                .maps
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?;
            if let Some((_, current)) = map.entries.iter_mut().find(|(stored, _)| *stored == key) {
                *current = value;
            } else {
                map.entries.push((key, value));
            }
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_obter" => {
            if args.len() != 2 {
                return Err(runtime_err("mapa_obter exige 2 argumentos"));
            }
            let RuntimeValue::Map(handle) = args[0] else {
                return Err(runtime_err("mapa_obter exige mapa genérico"));
            };
            let map = map_state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?;
            let key = generic_map_key(&args[1], map.key_is_verso)?;
            let value = map
                .entries
                .iter()
                .find(|(stored, _)| *stored == key)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| runtime_err("chave ausente em leitura de mapa"))?;
            Ok(IntrinsicCall::Done(Some(value)))
        }
        "__pinker_internal_mapa_tem" => {
            if args.len() != 2 {
                return Err(runtime_err("mapa_tem exige 2 argumentos"));
            }
            let RuntimeValue::Map(handle) = args[0] else {
                return Err(runtime_err("mapa_tem exige mapa genérico"));
            };
            let map = map_state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?;
            let key = generic_map_key(&args[1], map.key_is_verso)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                map.entries.iter().any(|(stored, _)| *stored == key),
            ))))
        }
        "__pinker_internal_mapa_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err("mapa_tamanho exige 1 argumento"));
            }
            let RuntimeValue::Map(handle) = args[0] else {
                return Err(runtime_err("mapa_tamanho exige mapa genérico"));
            };
            let map = map_state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                map.entries.len() as u64,
            ))))
        }
        "__pinker_internal_mapa_remover" => {
            if args.len() != 2 {
                return Err(runtime_err("mapa_remover exige 2 argumentos"));
            }
            let RuntimeValue::Map(handle) = args[0] else {
                return Err(runtime_err("mapa_remover exige mapa genérico"));
            };
            let key_is_verso = map_state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?
                .key_is_verso;
            let key = generic_map_key(&args[1], key_is_verso)?;
            let map = map_state
                .maps
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?;
            if let Some(index) = map.entries.iter().position(|(stored, _)| *stored == key) {
                map.entries.remove(index);
            }
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err("iterador de mapa exige 1 argumento"));
            }
            let RuntimeValue::Map(handle) = args[0] else {
                return Err(runtime_err("iterador exige mapa genérico"));
            };
            let map = map_state
                .maps
                .get(&handle)
                .ok_or_else(|| runtime_err("handle de mapa genérico inválido"))?;
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters.insert(
                iter_handle,
                RuntimeGenericMapIter {
                    keys_snapshot: map
                        .entries
                        .iter()
                        .map(|(key, _)| generic_map_key_value(key))
                        .collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_iterador_proxima_chave_bombom"
        | "__pinker_internal_mapa_iterador_proxima_chave_verso" => {
            if args.len() != 1 {
                return Err(runtime_err("avanço de iterador de mapa exige 1 argumento"));
            }
            let RuntimeValue::Int(iter_handle) = args[0] else {
                return Err(runtime_err("cursor de mapa exige bombom"));
            };
            let iter = map_state
                .map_iters
                .get_mut(&iter_handle)
                .ok_or_else(|| runtime_err("cursor de mapa inválido"))?;
            let key = iter
                .keys_snapshot
                .get(iter.next_index)
                .cloned()
                .ok_or_else(|| runtime_err("cursor de mapa esgotado"))?;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(key)))
        }

        // @pinker-nav:start interpreter.intrinsecos.mapas-verso-bombom
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa o primeiro bloco contíguo de mapa hospedado `verso -> bombom`, incluindo criação, escrita, leitura, presença, tamanho e cursores internos usados por lowering de iteração; valida aridade, handles e chaves sem definir layout nativo.
        "mapa_verso_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_verso_bombom.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoBombom(
                handle,
            ))))
        }
        "mapa_verso_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige 3 argumentos (mapa<verso,bombom>, verso, bombom)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige verso no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_definir'",
                ));
            };
            mapa.insert(key.clone(), value);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige 2 argumentos (mapa<verso,bombom>, verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_obter'",
                ));
            };
            let Some(value) = mapa.get(key) else {
                return Err(runtime_err("chave ausente em 'mapa_verso_bombom_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "mapa_verso_bombom_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige 2 argumentos (mapa<verso,bombom>, verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(key),
            ))))
        }
        "mapa_verso_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tamanho' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tamanho' exige mapa<verso,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "__pinker_internal_mapa_verso_bombom_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_criar' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_criar' exige mapa<verso,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em '__pinker_internal_mapa_verso_bombom_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_verso_bombom.insert(
                iter_handle,
                RuntimeMapVersoBombomIter {
                    keys_snapshot: mapa.keys().cloned().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_verso_bombom.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave'",
                )
            })?;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(key.clone()))))
        }
        // @pinker-nav:end interpreter.intrinsecos.mapas-verso-bombom

        // @pinker-nav:start interpreter.intrinsecos.leques
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Implementa leques hospedados por handle opaco, criando valores, anexando payload inteiro ou textual e carregando tag ou carga com validações de handle, tag e índice; descreve somente a representação do interpretador, não o layout futuro do runtime nativo.
        "__pinker_internal_leque_criar_0" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_criar_0' exige 1 argumento (tag)",
                ));
            }
            let RuntimeValue::Int(tag) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_criar_0' exige tag 'bombom'",
                ));
            };
            let handle = map_state.next_enum_handle;
            map_state.next_enum_handle = map_state.next_enum_handle.saturating_add(1);
            map_state.enum_values.insert(handle, (*tag, Vec::new()));
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "__pinker_internal_leque_anexar_b" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_b' exige 2 argumentos (leque, carga)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(payload)) = (&args[0], &args[1])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_b' exige handle e carga 'bombom'",
                ));
            };
            let Some((_, payloads)) = map_state.enum_values.get_mut(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_anexar_b'",
                ));
            };
            payloads.push(RuntimeEnumPayload::Int(*payload));
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*handle))))
        }
        "__pinker_internal_leque_anexar_v" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_v' exige 2 argumentos (leque, carga)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Str(payload)) = (&args[0], &args[1])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_anexar_v' exige handle 'bombom' e carga 'verso'",
                ));
            };
            let Some((_, payloads)) = map_state.enum_values.get_mut(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_anexar_v'",
                ));
            };
            payloads.push(RuntimeEnumPayload::Str(payload.clone()));
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*handle))))
        }
        // D1: cargas de lista. A variante guarda o **handle**, nunca uma cópia
        // do conteúdo: alterar a lista depois de construir a variante é
        // observado pela extração, e vice-versa.
        "__pinker_internal_leque_anexar_lista_b" | "__pinker_internal_leque_anexar_lista_v" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca interna de anexo de carga de lista exige 2 argumentos (leque, carga)",
                ));
            }
            let RuntimeValue::Int(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna de anexo de carga de lista exige handle de leque 'bombom'",
                ));
            };
            let payload = match (callee, &args[1]) {
                ("__pinker_internal_leque_anexar_lista_b", RuntimeValue::ListBombom(lista)) => {
                    RuntimeEnumPayload::ListBombom(*lista)
                }
                ("__pinker_internal_leque_anexar_lista_v", RuntimeValue::ListVerso(lista)) => {
                    RuntimeEnumPayload::ListVerso(*lista)
                }
                _ => {
                    return Err(runtime_err(&format!(
                        "intrínseca interna '{callee}' exige carga de lista da categoria correspondente"
                    )))
                }
            };
            let Some((_, payloads)) = map_state.enum_values.get_mut(handle) else {
                return Err(runtime_err(&format!(
                    "handle de leque inválido em '{callee}'"
                )));
            };
            payloads.push(payload);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*handle))))
        }
        "__pinker_internal_leque_anexar_saida_processo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca interna de anexo de SaidaProcesso exige 2 argumentos",
                ));
            }
            // A intrínseca interna cobre handles opacos nominais que não são
            // listas — hoje `SaidaProcesso` e `ValorJson`. A categoria viaja
            // junto com o handle para que a extração devolva a MESMA família,
            // e não uma que apenas compartilha a largura de palavra.
            let RuntimeValue::Int(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna de anexo de handle nominal exige handle de leque",
                ));
            };
            let carga = match &args[1] {
                RuntimeValue::SaidaProcesso(saida) => RuntimeEnumPayload::SaidaProcesso(*saida),
                RuntimeValue::ValorJson(raiz) => RuntimeEnumPayload::ValorJson(*raiz),
                _ => {
                    return Err(runtime_err(
                        "intrínseca interna de anexo de handle nominal exige handle opaco nominal",
                    ));
                }
            };
            let Some((_, payloads)) = map_state.enum_values.get_mut(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido ao anexar handle nominal",
                ));
            };
            payloads.push(carga);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*handle))))
        }
        "__pinker_internal_leque_carga_lista_b" | "__pinker_internal_leque_carga_lista_v" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca interna de extração de carga de lista exige 3 argumentos (leque, tag, índice)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(tag), RuntimeValue::Int(index)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(runtime_err(
                    "intrínseca interna de extração de carga de lista exige argumentos 'bombom'",
                ));
            };
            let Some((stored_tag, payloads)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(&format!(
                    "handle de leque inválido em '{callee}'"
                )));
            };
            if stored_tag != tag {
                return Err(runtime_err(&format!(
                    "extração de carga com variante inconsistente em '{callee}'"
                )));
            }
            // A extração devolve o mesmo handle armazenado, sem materializar
            // uma segunda lista.
            let value = match (callee, payloads.get(*index as usize)) {
                (
                    "__pinker_internal_leque_carga_lista_b",
                    Some(RuntimeEnumPayload::ListBombom(lista)),
                ) => RuntimeValue::ListBombom(*lista),
                (
                    "__pinker_internal_leque_carga_lista_v",
                    Some(RuntimeEnumPayload::ListVerso(lista)),
                ) => RuntimeValue::ListVerso(*lista),
                _ => {
                    return Err(runtime_err(&format!(
                        "carga de lista ausente ou de categoria divergente em '{callee}'"
                    )))
                }
            };
            Ok(IntrinsicCall::Done(Some(value)))
        }
        "__pinker_internal_leque_carga_saida_processo" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca interna de extração de SaidaProcesso exige 3 argumentos",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(tag), RuntimeValue::Int(index)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(runtime_err(
                    "intrínseca interna de extração de SaidaProcesso exige argumentos bombom",
                ));
            };
            let Some((stored_tag, payloads)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido ao extrair SaidaProcesso",
                ));
            };
            if stored_tag != tag {
                return Err(runtime_err(
                    "extração de SaidaProcesso com variante inconsistente",
                ));
            }
            match payloads.get(*index as usize) {
                Some(RuntimeEnumPayload::SaidaProcesso(saida)) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::SaidaProcesso(*saida),
                ))),
                Some(RuntimeEnumPayload::ValorJson(raiz)) => {
                    Ok(IntrinsicCall::Done(Some(RuntimeValue::ValorJson(*raiz))))
                }
                _ => Err(runtime_err("carga de handle nominal ausente ou divergente")),
            }
        }
        // Não há intrínseca chamável de união: `union_tag` e `union_extract`
        // são instruções tipadas da máquina, executadas diretamente.
        "__pinker_internal_leque_tag" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_tag' exige 1 argumento (leque)",
                ));
            }
            let RuntimeValue::Int(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_tag' exige handle 'bombom'",
                ));
            };
            let Some((tag, _)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_tag'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*tag))))
        }
        "__pinker_internal_leque_carga_b" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_b' exige 3 argumentos (leque, tag, índice)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(tag), RuntimeValue::Int(index)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_b' exige argumentos 'bombom'",
                ));
            };
            let Some((stored_tag, payloads)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_carga_b'",
                ));
            };
            if stored_tag != tag {
                return Err(runtime_err(
                    "extração de carga com variante inconsistente em '__pinker_internal_leque_carga_b'",
                ));
            }
            let Some(RuntimeEnumPayload::Int(value)) = payloads.get(*index as usize) else {
                return Err(runtime_err(
                    "carga 'bombom' ausente em '__pinker_internal_leque_carga_b'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "__pinker_internal_leque_carga_v" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_v' exige 3 argumentos (leque, tag, índice)",
                ));
            }
            let (RuntimeValue::Int(handle), RuntimeValue::Int(tag), RuntimeValue::Int(index)) =
                (&args[0], &args[1], &args[2])
            else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_leque_carga_v' exige argumentos 'bombom'",
                ));
            };
            let Some((stored_tag, payloads)) = map_state.enum_values.get(handle) else {
                return Err(runtime_err(
                    "handle de leque inválido em '__pinker_internal_leque_carga_v'",
                ));
            };
            if stored_tag != tag {
                return Err(runtime_err(
                    "extração de carga com variante inconsistente em '__pinker_internal_leque_carga_v'",
                ));
            }
            let Some(RuntimeEnumPayload::Str(value)) = payloads.get(*index as usize) else {
                return Err(runtime_err(
                    "carga 'verso' ausente em '__pinker_internal_leque_carga_v'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        // @pinker-nav:end interpreter.intrinsecos.leques

        // @pinker-nav:start interpreter.intrinsecos.io-arquivo-texto
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Agrupa intrínsecas hospedadas contíguas de stdin, arquivos por handle, operações diretas de texto e serialização mínima, validando aridade e tipos, lendo stdin, escrevendo stdout ou filesystem real e retornando valores Pinker; não é filesystem virtual nem runtime nativo.
        "ouvir" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'ouvir' exige 0 argumentos"));
            }
            let mut raw = String::new();
            io::stdin()
                .read_line(&mut raw)
                .map_err(|err| runtime_err(&format!("falha ao ler stdin em 'ouvir': {}", err)))?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(runtime_err(
                    "entrada inválida para 'ouvir': esperado inteiro bombom (u64), recebido vazio",
                ));
            }
            let parsed = trimmed.parse::<u64>().map_err(|_| {
                runtime_err(&format!(
                    "entrada inválida para 'ouvir': '{}' não é bombom válido",
                    trimmed
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "ouvir_verso" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'ouvir_verso' exige 0 argumentos"));
            }
            let maybe_line = read_stdin_line_minima("ouvir_verso")?;
            let Some(line) = maybe_line else {
                return Err(runtime_err(
                    "falha ao ler stdin em 'ouvir_verso': EOF imediato sem linha disponível",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                trim_final_newline_minimo(line),
            ))))
        }
        "ouvir_verso_ou" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ouvir_verso_ou' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(default_value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ouvir_verso_ou' exige valor padrão em verso",
                ));
            };
            match read_stdin_line_minima("ouvir_verso_ou") {
                Ok(Some(line)) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    trim_final_newline_minimo(line),
                )))),
                Ok(None) | Err(_) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
            }
        }
        "abrir" => {
            if args.len() != 1 {
                return Err(runtime_err("intrínseca 'abrir' exige 1 argumento (verso)"));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'abrir' exige caminho em verso"));
            };
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!("falha ao abrir arquivo em 'abrir': {}", err))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content,
                    append_enabled: false,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "criar_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'criar_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'criar_arquivo' exige caminho em verso",
                ));
            };
            fs::write(path, "").map_err(|err| {
                runtime_err(&format!(
                    "falha ao criar arquivo em 'criar_arquivo': {}",
                    err
                ))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content: String::new(),
                    append_enabled: false,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "abrir_anexo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'abrir_anexo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'abrir_anexo' exige caminho em verso",
                ));
            };
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .map_err(|err| {
                    runtime_err(&format!("falha ao abrir arquivo em 'abrir_anexo': {}", err))
                })?;
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao carregar conteúdo em 'abrir_anexo': {}",
                    err
                ))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content,
                    append_enabled: true,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "ler_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'ler_arquivo' exige handle bombom"));
            };
            let Some(open_file) = io_state.open_files.get(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'ler_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'ler_arquivo'"));
            };
            let trimmed = open_file.content.trim();
            if trimmed.is_empty() {
                return Err(runtime_err(
                    "conteúdo inválido para 'ler_arquivo': esperado inteiro bombom (u64), recebido vazio",
                ));
            }
            let parsed = trimmed.parse::<u64>().map_err(|_| {
                runtime_err(&format!(
                    "conteúdo inválido para 'ler_arquivo': '{}' não é bombom válido",
                    trimmed
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "ler_verso_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_verso_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_verso_arquivo' exige handle bombom",
                ));
            };
            let Some(open_file) = io_state.open_files.get(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'ler_verso_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'ler_verso_arquivo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                open_file.content.clone(),
            ))))
        }
        "ler_arquivo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo_verso' exige caminho em verso",
                ));
            };
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao ler arquivo em 'ler_arquivo_verso': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(content))))
        }
        "arquivo_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige caminho em verso",
                ));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige valor padrão em verso",
                ));
            };
            match fs::read_to_string(path) {
                Ok(content) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(content)))),
                Err(_) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
            }
        }
        "escrever" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'escrever' exige 2 argumentos (handle, bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'escrever' exige handle bombom"));
            };
            let RuntimeValue::Int(value) = args[1] else {
                return Err(runtime_err("intrínseca 'escrever' exige valor bombom"));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'escrever'"));
                }
                return Err(runtime_err("handle inválido em 'escrever'"));
            };
            let next_content = value.to_string();
            fs::write(&open_file.path, &next_content).map_err(|err| {
                runtime_err(&format!("falha ao escrever arquivo em 'escrever': {}", err))
            })?;
            open_file.content = next_content;
            Ok(IntrinsicCall::Done(None))
        }
        "escrever_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige 2 argumentos (handle, verso)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige handle bombom",
                ));
            };
            let RuntimeValue::Str(value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige valor em verso",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'escrever_verso'"));
                }
                return Err(runtime_err("handle inválido em 'escrever_verso'"));
            };
            fs::write(&open_file.path, value).map_err(|err| {
                runtime_err(&format!(
                    "falha ao escrever verso em arquivo em 'escrever_verso': {}",
                    err
                ))
            })?;
            open_file.content.clone_from(value);
            Ok(IntrinsicCall::Done(None))
        }
        "truncar_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'truncar_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'truncar_arquivo' exige handle bombom",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'truncar_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'truncar_arquivo'"));
            };
            fs::write(&open_file.path, "").map_err(|err| {
                runtime_err(&format!(
                    "falha ao truncar arquivo em 'truncar_arquivo': {}",
                    err
                ))
            })?;
            open_file.content.clear();
            Ok(IntrinsicCall::Done(None))
        }
        "anexar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'anexar_verso' exige 2 argumentos (handle, verso)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'anexar_verso' exige handle bombom"));
            };
            let RuntimeValue::Str(value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'anexar_verso' exige valor em verso",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'anexar_verso'"));
                }
                return Err(runtime_err("handle inválido em 'anexar_verso'"));
            };
            if !open_file.append_enabled {
                return Err(runtime_err(
                    "handle não foi aberto com 'abrir_anexo' em 'anexar_verso'",
                ));
            }
            let mut file = OpenOptions::new()
                .append(true)
                .open(&open_file.path)
                .map_err(|err| {
                    runtime_err(&format!(
                        "falha ao anexar verso em arquivo em 'anexar_verso': {}",
                        err
                    ))
                })?;
            use std::io::Write as _;
            file.write_all(value.as_bytes()).map_err(|err| {
                runtime_err(&format!(
                    "falha ao anexar verso em arquivo em 'anexar_verso': {}",
                    err
                ))
            })?;
            open_file.content.push_str(value);
            Ok(IntrinsicCall::Done(None))
        }
        "fechar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'fechar' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'fechar' exige handle bombom"));
            };
            if io_state.open_files.remove(&handle).is_none() {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'fechar'"));
                }
                return Err(runtime_err("handle inválido em 'fechar'"));
            }
            io_state.closed_handles.insert(handle);
            Ok(IntrinsicCall::Done(None))
        }
        "juntar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(lhs) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(rhs) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(format!(
                "{}{}",
                lhs, rhs
            )))))
        }
        "tamanho_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tamanho_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tamanho_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                value.chars().count() as u64,
            ))))
        }
        "indice_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige 2 argumentos (verso, bombom)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige segundo argumento em bombom",
                ));
            };
            let Some(ch) = value.chars().nth(index as usize) else {
                return Err(runtime_err(
                    "índice fora da faixa em 'indice_verso' para o verso informado",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(ch.to_string()))))
        }
        "fatiar_verso" => {
            /// Converte um índice em Unicode scalar values para uma fronteira UTF-8,
            /// incluindo a fronteira final do texto.
            fn verso_codepoint_byte_offset(texto: &str, index: u64) -> Option<usize> {
                let mut logical = 0_u64;
                for (byte_offset, _) in texto.char_indices() {
                    if logical == index {
                        return Some(byte_offset);
                    }
                    logical = logical.checked_add(1)?;
                }
                (logical == index).then_some(texto.len())
            }

            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'fatiar_verso' exige 3 argumentos (verso, bombom, bombom)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'fatiar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Int(start) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'fatiar_verso' exige segundo argumento em bombom",
                ));
            };
            let RuntimeValue::Int(end) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'fatiar_verso' exige terceiro argumento em bombom",
                ));
            };
            if start > end {
                return Err(runtime_err(
                    "intervalo inválido em 'fatiar_verso': início maior que fim",
                ));
            }
            let length = u64::try_from(value.chars().count()).map_err(|_| {
                runtime_err("comprimento textual excede a faixa de índice de 'fatiar_verso'")
            })?;
            if start > length {
                return Err(runtime_err(
                    "índice inicial fora da faixa em 'fatiar_verso'",
                ));
            }
            if end > length {
                return Err(runtime_err("índice final fora da faixa em 'fatiar_verso'"));
            }
            let start_byte = verso_codepoint_byte_offset(value, start)
                .ok_or_else(|| runtime_err("falha interna ao resolver início de 'fatiar_verso'"))?;
            let end_byte = verso_codepoint_byte_offset(value, end)
                .ok_or_else(|| runtime_err("falha interna ao resolver fim de 'fatiar_verso'"))?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                value[start_byte..end_byte].to_string(),
            ))))
        }
        "contem_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(trecho) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.contains(trecho),
            ))))
        }
        "comeca_com" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(prefixo) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.starts_with(prefixo),
            ))))
        }
        "termina_com" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sufixo) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.ends_with(sufixo),
            ))))
        }
        "igual_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(lhs) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(rhs) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(lhs == rhs))))
        }
        "vazio_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'vazio_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'vazio_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.is_empty(),
            ))))
        }
        "aparar_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aparar_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aparar_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.trim().to_string(),
            ))))
        }
        // Parte E2: SHA256(verso) = SHA256(UTF8_BYTES(verso)).
        //
        // `as_bytes()` são os bytes UTF-8 exatos — nunca `chars()`, nunca
        // `tamanho_verso` (que conta codepoints), nunca normalização Unicode.
        // Dado já em memória não pode falhar, então não devolve `Resultado`.
        nome if crate::sha256::e_acessor(nome) => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'sha256_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'sha256_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                pinker_sha256_contract::sha256_hex(texto.as_bytes()),
            ))))
        }
        "minusculo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'minusculo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'minusculo_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.to_lowercase(),
            ))))
        }
        "maiusculo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'maiusculo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'maiusculo_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.to_uppercase(),
            ))))
        }
        "indice_verso_em" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(trecho) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige segundo argumento em verso",
                ));
            };
            let pos = texto.find(trecho).map_or(u64::MAX, |v| v as u64);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(pos))))
        }
        // Fase 140 — buscar_verso(texto, padrao) -> bombom
        "buscar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(padrao) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige segundo argumento em verso",
                ));
            };
            if padrao.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' não aceita padrão vazio",
                ));
            }
            let pos = texto.find(padrao).map_or(u64::MAX, |v| v as u64);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(pos))))
        }
        "nao_vazio_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'nao_vazio_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'nao_vazio_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                !texto.is_empty(),
            ))))
        }
        // Fase 137 — dividir_verso_em(texto, sep, indice) -> verso
        "dividir_verso_em" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige 3 argumentos (verso, verso, bombom)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Int(indice) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige terceiro argumento em bombom",
                ));
            };
            if sep.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' não aceita separador vazio",
                ));
            }
            let partes: Vec<&str> = texto.split(sep.as_str()).collect();
            let Some(parte) = partes.get(indice as usize) else {
                return Err(runtime_err(
                    "índice fora da faixa em 'dividir_verso_em' para o verso informado",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                parte.to_string(),
            ))))
        }
        // Fase 137 — dividir_verso_contar(texto, sep) -> bombom
        "dividir_verso_contar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige segundo argumento em verso",
                ));
            };
            if sep.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' não aceita separador vazio",
                ));
            }
            let count = texto.split(sep.as_str()).count() as u64;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(count))))
        }
        // Fase 138 — substituir_verso(texto, de, para) -> verso
        "substituir_verso" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige 3 argumentos (verso, verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(de) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Str(para) = &args[2] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige terceiro argumento em verso",
                ));
            };
            if de.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' não aceita padrão vazio",
                ));
            }
            let resultado = texto.replace(de.as_str(), para.as_str());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        // Fase 139 — juntar_verso_com(a, sep, b) -> verso
        "juntar_verso_com" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige 3 argumentos (verso, verso, verso)",
                ));
            }
            let RuntimeValue::Str(a) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Str(b) = &args[2] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige terceiro argumento em verso",
                ));
            };
            let resultado = format!("{}{}{}", a, sep, b);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        "formatar_verso" => {
            if args.len() < 2 {
                return Err(runtime_err(
                    "intrínseca 'formatar_verso' exige pelo menos 2 argumentos (modelo verso, args...)",
                ));
            }
            let RuntimeValue::Str(modelo) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'formatar_verso' exige modelo em verso",
                ));
            };
            let resultado = formatar_verso_runtime(modelo, &args[1..])?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        "__ternario" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca '__ternario' exige 3 argumentos (condição, valor_verdade, valor_falso)",
                ));
            }
            let cond = match &args[0] {
                RuntimeValue::Bool(b) => *b,
                _ => {
                    return Err(runtime_err("intrínseca '__ternario' exige condição logica"));
                }
            };
            let result = if cond {
                args[1].clone()
            } else {
                args[2].clone()
            };
            Ok(IntrinsicCall::Done(Some(result)))
        }
        "ler_linha_csv_bombom" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige 2 argumentos (linha verso, separador verso)",
                ));
            }
            let RuntimeValue::Str(linha) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige linha em verso",
                ));
            };
            let RuntimeValue::Str(separador) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige separador em verso",
                ));
            };
            let separador = validar_separador_csv("ler_linha_csv_bombom", separador)?;
            if linha.contains('\n') || linha.contains('\r') {
                return Err(runtime_err(
                    "linha inválida em 'ler_linha_csv_bombom': multiline fora do recorte",
                ));
            }
            if linha.contains('"') {
                return Err(runtime_err(
                    "linha inválida em 'ler_linha_csv_bombom': quoting fora do recorte",
                ));
            }

            let handle = list_state.next_list_handle;
            list_state.next_list_handle += 1;
            let mut itens = Vec::new();
            for campo in linha.split(separador) {
                let Ok(valor) = campo.parse::<u64>() else {
                    return Err(runtime_err(
                        "campo inválido em 'ler_linha_csv_bombom': esperado bombom simples sem quoting",
                    ));
                };
                itens.push(valor);
            }
            list_state.lists_bombom.insert(handle, itens);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListBombom(handle))))
        }
        "emitir_linha_csv_bombom" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige 2 argumentos (lista<bombom>, separador verso)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(separador) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige separador em verso no segundo argumento",
                ));
            };
            let separador = validar_separador_csv("emitir_linha_csv_bombom", separador)?;
            let Some(itens) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de lista inválido em 'emitir_linha_csv_bombom'",
                ));
            };
            let linha = itens
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(separador);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(linha))))
        }
        "ler_json_plano_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_json_plano_bombom' exige 1 argumento (json verso)",
                ));
            }
            let RuntimeValue::Str(json) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_json_plano_bombom' exige json em verso",
                ));
            };
            let handle = map_state.next_map_handle;
            map_state.next_map_handle += 1;
            let mapa = parse_json_plano_bombom(json)?;
            map_state.maps_verso_bombom.insert(handle, mapa);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoBombom(
                handle,
            ))))
        }
        "emitir_json_plano_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'emitir_json_plano_bombom' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_json_plano_bombom' exige mapa<verso,bombom> no argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'emitir_json_plano_bombom'",
                ));
            };
            let json = emit_json_plano_bombom(mapa)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(json))))
        }
        // @pinker-nav:end interpreter.intrinsecos.io-arquivo-texto
        // @pinker-nav:start interpreter.intrinsecos.falha-operacional
        // @pinker-nav:domain erros
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Despacho hospedado das superfícies falíveis da Parte B: o braço reconhece a chamada consultando `falha_operacional::superficie` e decide por `OperacaoFalivel`, nunca por nome literal — o nome público continua declarado só na autoridade. Leitura de arquivo por caminho, spawn de processo e conversão de texto para número devolvem `Resultado<T,E>` construído pelo mesmo `enum_values` de qualquer leque do usuário. A falha recuperável carrega a causa em `verso`; aridade, tipo de argumento e invariantes internas continuam fatais.
        nome if crate::falha_operacional::superficie(nome).is_some() => {
            let superficie = crate::falha_operacional::superficie(nome)
                .expect("o guarda acima já resolveu a superfície");
            executar_superficie_falivel(superficie, args, map_state, list_state)
        }
        // @pinker-nav:end interpreter.intrinsecos.falha-operacional

        // @pinker-nav:start interpreter.intrinsecos.tempo-processos-ambiente
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Agrupa intrínsecas hospedadas contíguas de relógio, processos, argumentos CLI, ambiente, caminhos, status de saída, assertivas, espera e cópia ou renomeação, com efeitos reais no host como `Command`, pipes, diretório atual, variáveis de ambiente, sono e filesystem; devolve resultados ao interpretador sem prometer modo freestanding.
        "tempo_unix" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'tempo_unix' exige 0 argumentos"));
            }
            let agora = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| {
                    runtime_err("intrínseca 'tempo_unix' não suporta tempo anterior à época Unix")
                })?
                .as_secs();
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(agora))))
        }
        "formatar_tempo_unix" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'formatar_tempo_unix' exige 1 argumento (timestamp bombom)",
                ));
            }
            let RuntimeValue::Int(timestamp) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'formatar_tempo_unix' exige timestamp em bombom",
                ));
            };
            let texto = formatar_tempo_unix_iso_utc(timestamp)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(texto))))
        }
        "executar_processo" => {
            if !(1..=2).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'executar_processo' exige 1 ou 2 argumentos (comando verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'executar_processo' exige comando em verso",
                ));
            };
            let explicit_argv = match args.get(1) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'executar_processo' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let exit_code = executar_processo_minimo(command_name, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(exit_code))))
        }
        "executar_com_entrada" => {
            if !(2..=3).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'executar_com_entrada' exige 2 ou 3 argumentos (comando verso, entrada verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'executar_com_entrada' exige comando em verso",
                ));
            };
            let RuntimeValue::Str(input_text) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'executar_com_entrada' exige entrada em verso",
                ));
            };
            let explicit_argv = match args.get(2) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'executar_com_entrada' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let exit_code = executar_com_entrada_minimo(command_name, input_text, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(exit_code))))
        }
        "pipeline_minimo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'pipeline_minimo' exige 2 argumentos (produtor verso, consumidor verso)",
                ));
            }
            let RuntimeValue::Str(producer_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'pipeline_minimo' exige produtor em verso",
                ));
            };
            let RuntimeValue::Str(consumer_name) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'pipeline_minimo' exige consumidor em verso",
                ));
            };
            let exit_code = pipeline_minimo(producer_name, consumer_name)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(exit_code))))
        }
        "capturar_stdout" => {
            if !(1..=2).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'capturar_stdout' exige 1 ou 2 argumentos (comando verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'capturar_stdout' exige comando em verso",
                ));
            };
            let explicit_argv = match args.get(1) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'capturar_stdout' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let stdout = capturar_stdout_minimo(command_name, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(stdout))))
        }
        "capturar_stderr" => {
            if !(1..=2).contains(&args.len()) {
                return Err(runtime_err(
                    "intrínseca 'capturar_stderr' exige 1 ou 2 argumentos (comando verso[, argv1 verso])",
                ));
            }
            let RuntimeValue::Str(command_name) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'capturar_stderr' exige comando em verso",
                ));
            };
            let explicit_argv = match args.get(1) {
                Some(RuntimeValue::Str(arg)) => Some(arg.as_str()),
                Some(_) => {
                    return Err(runtime_err(
                        "intrínseca 'capturar_stderr' exige argv1 em verso",
                    ));
                }
                None => None,
            };
            let stderr = capturar_stderr_minimo(command_name, explicit_argv)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(stderr))))
        }
        "argumento" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'argumento' exige 1 argumento (índice bombom)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err("intrínseca 'argumento' exige índice bombom"));
            };
            let Some(arg) = io_state.cli_args.get(index as usize) else {
                return Err(runtime_err("índice fora da faixa em 'argumento'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(arg.clone()))))
        }
        "argumento_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'argumento_ou' exige 2 argumentos (índice bombom, padrão verso)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err("intrínseca 'argumento_ou' exige índice bombom"));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'argumento_ou' exige valor padrão em verso",
                ));
            };
            let value = io_state
                .cli_args
                .get(index as usize)
                .cloned()
                .unwrap_or_else(|| default_value.clone());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        intrinsic_name @ ("tem_chave" | "tem_argumento_nomeado") => {
            if args.len() != 1 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 1 argumento (chave verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, key)?;
            let found = pinker_argv_contract::chave_tem_valor(&io_state.cli_args, key);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(found))))
        }
        intrinsic_name @ ("pedir_argumento" | "argumento_nomeado_ou") => {
            if args.len() != 2 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 2 argumentos (chave verso, padrão verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige valor padrão em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, key)?;
            let estado = pinker_argv_contract::estado_da_chave(&io_state.cli_args, key);
            match pinker_argv_contract::resolver_pedido(estado) {
                pinker_argv_contract::Pedido::Valor(value) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(value.to_string()),
                ))),
                pinker_argv_contract::Pedido::Padrao => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(default_value.clone()),
                ))),
                pinker_argv_contract::Pedido::ChaveSemValor => Err(runtime_err(
                    &pinker_argv_contract::mensagem_chave_sem_valor(intrinsic_name, key),
                )),
            }
        }
        "tem_flag" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tem_flag' exige 1 argumento (chave verso)",
                ));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err("intrínseca 'tem_flag' exige chave em verso"));
            };
            ensure_named_arg_key_valid("tem_flag", key)?;
            let found = pinker_argv_contract::contem_token_exato(&io_state.cli_args, key);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(found))))
        }
        "ambiente_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'ambiente_ou' exige 2 argumentos (chave verso, padrão verso)",
                ));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err("intrínseca 'ambiente_ou' exige chave em verso"));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'ambiente_ou' exige valor padrão em verso",
                ));
            };
            // A chave vazia é recusada como no resto da família. O nativo já
            // recusava; o interpretador devolvia o padrão em silêncio, e
            // `env::var("")` nunca teria sucesso — o padrão não era fallback,
            // era o único desfecho possível de uma chamada inválida. A grafia
            // da mensagem é a genérica, e não a de ambiente, porque
            // `ambiente_ou` tem uma chave só: não há qual delas desambiguar.
            if key.is_empty() {
                return Err(runtime_err(&pinker_argv_contract::mensagem_chave_vazia(
                    "ambiente_ou",
                )));
            }
            let value = env::var(key).unwrap_or_else(|_| default_value.clone());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        intrinsic_name @ ("buscar_contexto" | "argumento_nomeado_ou_ambiente_ou") => {
            if args.len() != 3 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 3 argumentos (chave_arg verso, chave_env verso, padrão verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(arg_key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave_arg em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(env_key) = &args[1] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave_env em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(default_value) = &args[2] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige valor padrão em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, arg_key)?;
            ensure_env_key_valid(intrinsic_name, env_key)?;
            let estado = pinker_argv_contract::estado_da_chave(&io_state.cli_args, arg_key);
            match pinker_argv_contract::resolver_contexto(estado) {
                pinker_argv_contract::Contexto::Valor(value) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(value.to_string()),
                ))),
                pinker_argv_contract::Contexto::Ambiente => {
                    let value = env::var(env_key).unwrap_or_else(|_| default_value.clone());
                    Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
                }
                pinker_argv_contract::Contexto::ChaveSemValor => Err(runtime_err(
                    &pinker_argv_contract::mensagem_chave_sem_valor(intrinsic_name, arg_key),
                )),
            }
        }
        "caminho_existe" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'caminho_existe' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'caminho_existe' exige caminho em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).exists(),
            ))))
        }
        "e_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'e_arquivo' exige caminho em verso"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).is_file(),
            ))))
        }
        "e_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'e_diretorio' exige caminho em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).is_dir(),
            ))))
        }
        "juntar_caminho" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige 2 argumentos (base verso, trecho verso)",
                ));
            }
            let RuntimeValue::Str(base) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige base em verso",
                ));
            };
            let RuntimeValue::Str(child) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige trecho em verso",
                ));
            };
            let joined = std::path::PathBuf::from(base).join(child);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                joined.to_string_lossy().to_string(),
            ))))
        }
        "tamanho_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige caminho em verso",
                ));
            };
            let metadata = fs::metadata(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao obter metadados em 'tamanho_arquivo': {}",
                    err
                ))
            })?;
            if !metadata.is_file() {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige caminho de arquivo regular",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(metadata.len()))))
        }
        "e_vazio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_vazio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'e_vazio' exige caminho em verso"));
            };
            let metadata = fs::metadata(path).map_err(|err| {
                runtime_err(&format!("falha ao obter metadados em 'e_vazio': {}", err))
            })?;
            if !metadata.is_file() {
                return Err(runtime_err(
                    "intrínseca 'e_vazio' exige caminho de arquivo regular",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                metadata.len() == 0,
            ))))
        }
        "criar_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'criar_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'criar_diretorio' exige caminho em verso",
                ));
            };
            fs::create_dir(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao criar diretório em 'criar_diretorio': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "remover_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'remover_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'remover_arquivo' exige caminho em verso",
                ));
            };
            fs::remove_file(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao remover arquivo em 'remover_arquivo': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "remover_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'remover_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'remover_diretorio' exige caminho em verso",
                ));
            };
            fs::remove_dir(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao remover diretório em 'remover_diretorio': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "diretorio_atual" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'diretorio_atual' exige 0 argumentos",
                ));
            }
            let value = env::current_dir().map_err(|err| {
                runtime_err(&format!(
                    "falha ao obter diretório atual em 'diretorio_atual': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                value.to_string_lossy().to_string(),
            ))))
        }
        "quantos_argumentos" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'quantos_argumentos' exige 0 argumentos",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                io_state.cli_args.len() as u64,
            ))))
        }
        "tem_argumento" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tem_argumento' exige 1 argumento (índice bombom)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tem_argumento' exige índice bombom",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                io_state.cli_args.get(index as usize).is_some(),
            ))))
        }
        "sair" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'sair' exige 1 argumento (código bombom)",
                ));
            }
            let RuntimeValue::Int(code) = args[0] else {
                return Err(runtime_err("intrínseca 'sair' exige código bombom"));
            };
            io_state.exit_status = Some(code.min(i32::MAX as u64) as i32);
            Ok(IntrinsicCall::Done(None))
        }
        "afirmar" => {
            if args.is_empty() || args.len() > 2 {
                return Err(runtime_err(
                    "intrínseca 'afirmar' exige 1 ou 2 argumentos (condição logica [, mensagem verso])",
                ));
            }
            let RuntimeValue::Bool(cond) = args[0] else {
                return Err(runtime_err("intrínseca 'afirmar' exige condição logica"));
            };
            if !cond {
                let msg = if args.len() == 2 {
                    if let RuntimeValue::Str(ref s) = args[1] {
                        format!("afirmação falhou: {}", s)
                    } else {
                        "afirmação falhou".to_string()
                    }
                } else {
                    "afirmação falhou".to_string()
                };
                return Err(runtime_err(&msg));
            }
            Ok(IntrinsicCall::Done(None))
        }
        "dormir" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'dormir' exige 1 argumento (milissegundos bombom)",
                ));
            }
            let RuntimeValue::Int(ms) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dormir' exige milissegundos bombom",
                ));
            };
            std::thread::sleep(std::time::Duration::from_millis(ms));
            Ok(IntrinsicCall::Done(None))
        }
        "copiar_arquivo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'copiar_arquivo' exige 2 argumentos (origem verso, destino verso)",
                ));
            }
            let RuntimeValue::Str(ref origem) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'copiar_arquivo' exige origem verso",
                ));
            };
            let RuntimeValue::Str(ref destino) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'copiar_arquivo' exige destino verso",
                ));
            };
            fs::copy(origem, destino).map_err(|err| {
                runtime_err(&format!(
                    "falha ao copiar '{}' para '{}': {}",
                    origem, destino, err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "renomear_arquivo" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'renomear_arquivo' exige 2 argumentos (de verso, para verso)",
                ));
            }
            let RuntimeValue::Str(ref de) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'renomear_arquivo' exige 'de' verso",
                ));
            };
            let RuntimeValue::Str(ref para) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'renomear_arquivo' exige 'para' verso",
                ));
            };
            fs::rename(de, para).map_err(|err| {
                runtime_err(&format!(
                    "falha ao renomear '{}' para '{}': {}",
                    de, para, err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        // @pinker-nav:end interpreter.intrinsecos.tempo-processos-ambiente

        // @pinker-nav:start interpreter.intrinsecos.conversoes-numero-texto
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Intrínsecas hospedadas de conversão entre número e texto: `verso_para_bombom` (parse de `verso` para `bombom`, com erro em texto inválido) e `bombom_para_verso` (formatação de `bombom` como `verso`). Valida aridade e tipos dos argumentos.
        "verso_para_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'verso_para_bombom' exige 1 argumento (texto verso)",
                ));
            }
            let RuntimeValue::Str(ref texto) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'verso_para_bombom' exige texto verso",
                ));
            };
            let parsed: u64 = texto
                .trim()
                .parse()
                .map_err(|_| runtime_err(&format!("falha ao converter '{}' para bombom", texto)))?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "bombom_para_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'bombom_para_verso' exige 1 argumento (valor bombom)",
                ));
            }
            let RuntimeValue::Int(valor) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'bombom_para_verso' exige valor bombom",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                valor.to_string(),
            ))))
        }
        // @pinker-nav:end interpreter.intrinsecos.conversoes-numero-texto

        // Arm isolado da família `acaso` (ver `interpreter.intrinsecos.acaso`),
        // fisicamente separado dela neste ponto do dispatcher; sem âncora própria.
        "aleatorio_entre" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_entre' exige 3 argumentos (gerador bombom, min bombom, max bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_entre' exige gerador bombom",
                ));
            };
            let RuntimeValue::Int(min) = args[1] else {
                return Err(runtime_err("intrínseca 'aleatorio_entre' exige min bombom"));
            };
            let RuntimeValue::Int(max) = args[2] else {
                return Err(runtime_err("intrínseca 'aleatorio_entre' exige max bombom"));
            };
            if min > max {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_entre': min não pode ser maior que max",
                ));
            }
            let generator = random_state
                .generators
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("intrínseca 'aleatorio_entre': gerador inválido"))?;
            let raw = advance_random_generator(&mut generator.state);
            let range = max - min + 1;
            let result = if range == 0 { raw } else { min + (raw % range) };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(result))))
        }

        // @pinker-nav:start interpreter.intrinsecos.mapas-tipados
        // @pinker-nav:domain intrinsecos
        // @pinker-nav:layer interpreter
        // @pinker-nav:summary Intrínsecas hospedadas das famílias tipadas de mapa `mapa<verso,verso>`, `mapa<bombom,bombom>` e `mapa<bombom,verso>` — cada uma com `criar`/`definir`/`obter`/`tem`/`tamanho`/`remover` e os cursores internos de iteração (`__pinker_internal_..._iterador_criar`/`_proxima_chave`) — mais a remoção residual de `mapa<verso,bombom>`. Opera sobre as tabelas de estado do hospedeiro; valida aridade e tipos.
        "mapa_verso_bombom_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_remover' exige 2 argumentos (mapa, chave verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_remover' exige mapa<verso,bombom>",
                ));
            };
            let RuntimeValue::Str(ref chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_remover' exige chave verso",
                ));
            };
            let mapa = map_state
                .maps_verso_bombom
                .get_mut(&handle)
                .ok_or_else(|| {
                    runtime_err("intrínseca 'mapa_verso_bombom_remover': mapa inválido")
                })?;
            mapa.remove(chave);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_verso_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_verso_verso.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoVerso(
                handle,
            ))))
        }
        "mapa_verso_verso_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige 3 argumentos (mapa<verso,verso>, verso, verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige mapa<verso,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige verso no segundo argumento",
                ));
            };
            let RuntimeValue::Str(ref value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_definir' exige verso no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_verso.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_definir'",
                ));
            };
            mapa.insert(key.clone(), value.clone());
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_verso_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_obter' exige 2 argumentos (mapa<verso,verso>, verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_obter' exige mapa<verso,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_obter' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_obter'",
                ));
            };
            let Some(value) = mapa.get(key) else {
                return Err(runtime_err("chave ausente em 'mapa_verso_verso_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        "mapa_verso_verso_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tem' exige 2 argumentos (mapa<verso,verso>, verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tem' exige mapa<verso,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tem' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(key),
            ))))
        }
        "mapa_verso_verso_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tamanho' exige 1 argumento (mapa<verso,verso>)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_tamanho' exige mapa<verso,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em 'mapa_verso_verso_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "mapa_verso_verso_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_remover' exige 2 argumentos (mapa, chave verso)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_remover' exige mapa<verso,verso>",
                ));
            };
            let RuntimeValue::Str(ref chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_verso_remover' exige chave verso",
                ));
            };
            let mapa = map_state.maps_verso_verso.get_mut(&handle).ok_or_else(|| {
                runtime_err("intrínseca 'mapa_verso_verso_remover': mapa inválido")
            })?;
            mapa.remove(chave);
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_verso_verso_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_criar' exige 1 argumento (mapa<verso,verso>)",
                ));
            }
            let RuntimeValue::MapVersoVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_criar' exige mapa<verso,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,verso> inválido em '__pinker_internal_mapa_verso_verso_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_verso_verso.insert(
                iter_handle,
                RuntimeMapVersoVersoIter {
                    keys_snapshot: mapa.keys().cloned().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_verso_verso_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_verso_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_verso_verso.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_verso_verso_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_verso_verso_iterador_proxima_chave'",
                )
            })?;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(key.clone()))))
        }
        "mapa_bombom_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_bombom_bombom.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapBombomBombom(
                handle,
            ))))
        }
        "mapa_bombom_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige 3 argumentos (mapa<bombom,bombom>, bombom, bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige mapa<bombom,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_bombom.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_definir'",
                ));
            };
            mapa.insert(key, value);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_bombom_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_obter' exige 2 argumentos (mapa<bombom,bombom>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_obter' exige mapa<bombom,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_obter'",
                ));
            };
            let Some(value) = mapa.get(&key) else {
                return Err(runtime_err("chave ausente em 'mapa_bombom_bombom_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "mapa_bombom_bombom_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tem' exige 2 argumentos (mapa<bombom,bombom>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tem' exige mapa<bombom,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tem' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(&key),
            ))))
        }
        "mapa_bombom_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tamanho' exige 1 argumento (mapa<bombom,bombom>)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_tamanho' exige mapa<bombom,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em 'mapa_bombom_bombom_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "mapa_bombom_bombom_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_remover' exige 2 argumentos (mapa, chave bombom)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_remover' exige mapa<bombom,bombom>",
                ));
            };
            let RuntimeValue::Int(chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_bombom_remover' exige chave bombom",
                ));
            };
            let mapa = map_state
                .maps_bombom_bombom
                .get_mut(&handle)
                .ok_or_else(|| {
                    runtime_err("intrínseca 'mapa_bombom_bombom_remover': mapa inválido")
                })?;
            mapa.remove(&chave);
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_bombom_bombom_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_criar' exige 1 argumento (mapa<bombom,bombom>)",
                ));
            }
            let RuntimeValue::MapBombomBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_criar' exige mapa<bombom,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,bombom> inválido em '__pinker_internal_mapa_bombom_bombom_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_bombom_bombom.insert(
                iter_handle,
                RuntimeMapBombomBombomIter {
                    keys_snapshot: mapa.keys().copied().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_bombom_bombom.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave'",
                )
            })?;
            let key_val = *key;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(key_val))))
        }
        "mapa_bombom_verso_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_bombom_verso.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapBombomVerso(
                handle,
            ))))
        }
        "mapa_bombom_verso_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige 3 argumentos (mapa<bombom,verso>, bombom, verso)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige mapa<bombom,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Str(ref value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_definir' exige verso no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_verso.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_definir'",
                ));
            };
            mapa.insert(key, value.clone());
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_bombom_verso_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_obter' exige 2 argumentos (mapa<bombom,verso>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_obter' exige mapa<bombom,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_obter'",
                ));
            };
            let Some(value) = mapa.get(&key) else {
                return Err(runtime_err("chave ausente em 'mapa_bombom_verso_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value.clone()))))
        }
        "mapa_bombom_verso_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tem' exige 2 argumentos (mapa<bombom,verso>, bombom)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tem' exige mapa<bombom,verso> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tem' exige bombom no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(&key),
            ))))
        }
        "mapa_bombom_verso_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tamanho' exige 1 argumento (mapa<bombom,verso>)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_tamanho' exige mapa<bombom,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em 'mapa_bombom_verso_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "mapa_bombom_verso_remover" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_remover' exige 2 argumentos (mapa, chave bombom)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_remover' exige mapa<bombom,verso>",
                ));
            };
            let RuntimeValue::Int(chave) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_bombom_verso_remover' exige chave bombom",
                ));
            };
            let mapa = map_state
                .maps_bombom_verso
                .get_mut(&handle)
                .ok_or_else(|| {
                    runtime_err("intrínseca 'mapa_bombom_verso_remover': mapa inválido")
                })?;
            mapa.remove(&chave);
            Ok(IntrinsicCall::Done(None))
        }
        "__pinker_internal_mapa_bombom_verso_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_criar' exige 1 argumento (mapa<bombom,verso>)",
                ));
            }
            let RuntimeValue::MapBombomVerso(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_criar' exige mapa<bombom,verso> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_bombom_verso.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<bombom,verso> inválido em '__pinker_internal_mapa_bombom_verso_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_bombom_verso.insert(
                iter_handle,
                RuntimeMapBombomVersoIter {
                    keys_snapshot: mapa.keys().copied().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_bombom_verso.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_bombom_verso_iterador_proxima_chave'",
                )
            })?;
            let key_val = *key;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(key_val))))
        }
        // @pinker-nav:end interpreter.intrinsecos.mapas-tipados

        // Arm isolado da família `listas` (ver `interpreter.intrinsecos.listas`),
        // fisicamente separado dela neste ponto do dispatcher; sem âncora própria.
        // Segue o ramo `_ => NotIntrinsic` de encerramento do dispatcher.
        "lista_bombom_inserir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige 3 argumentos (lista, índice bombom, valor bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige lista<bombom>",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige índice bombom",
                ));
            };
            let RuntimeValue::Int(valor) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir' exige valor bombom",
                ));
            };
            let lista = list_state
                .lists_bombom
                .get_mut(&handle)
                .ok_or_else(|| runtime_err("intrínseca 'lista_bombom_inserir': lista inválida"))?;
            let idx = index as usize;
            if idx > lista.len() {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_inserir': índice fora dos limites",
                ));
            }
            lista.insert(idx, valor);
            Ok(IntrinsicCall::Done(None))
        }
        _ => Ok(IntrinsicCall::NotIntrinsic),
    }
}
