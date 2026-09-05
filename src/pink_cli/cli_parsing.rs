//! Parsing e roteamento da CLI (`cli.parsing.subcomandos`,
//! `cli.parsing.roteamento`), unidade MAIN-5 da decomposição física #605.
//!
//! Movimento físico: as decisões, o estado e a ordem são os do entrypoint.
//! `main.rs` continua dono da orquestração; aqui mora só a implementação.

use super::*;

// @pinker-nav:start cli.parsing.subcomandos
// @pinker-nav:domain parsing
// @pinker-nav:layer cli
// @pinker-nav:summary Parsers estritos dos subcomandos, incluindo estado, doctor e verificar: validam flags, posicionais, duplicatas e requisitos cruzados antes de produzir modelos tipados.
fn parse_build_args(binary: &str, args: &[String]) -> Result<BuildConfig, String> {
    let mut input: Option<String> = None;
    let mut out_dir = "build".to_string();
    let mut nativo = false;
    let mut i = 0usize;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return Err(build_usage(binary)),
            "--out-dir" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--out-dir' requer um valor.\n\n{}",
                        build_usage(binary)
                    ));
                }
                out_dir.clone_from(&args[i]);
            }
            "--nativo" => {
                nativo = true;
            }
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida no comando build: '{}'\n\n{}",
                    arg,
                    build_usage(binary)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado em 'build'.\n\n{}",
                        build_usage(binary)
                    ));
                }
                input = Some(arg.clone());
            }
        }
        i += 1;
    }

    let Some(input) = input else {
        return Err(build_usage(binary));
    };
    Ok(BuildConfig {
        input,
        out_dir,
        nativo,
    })
}

fn parse_editor_args(binary: &str, args: &[String]) -> Result<EditorConfig, String> {
    let mut input: Option<String> = None;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => return Err(editor_usage(binary)),
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida no comando editor: '{}'\n\n{}",
                    arg,
                    editor_usage(binary)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado em 'editor'.\n\n{}",
                        editor_usage(binary)
                    ));
                }
                input = Some(arg.clone());
            }
        }
    }

    let Some(input) = input else {
        return Err(editor_usage(binary));
    };
    Ok(EditorConfig { input })
}

fn parse_repl_args(binary: &str, args: &[String]) -> Result<ReplConfig, String> {
    if args.is_empty() {
        return Ok(ReplConfig);
    }

    let arg = &args[0];
    match arg.as_str() {
        "--help" | "-h" => Err(repl_usage(binary)),
        _ if arg.starts_with('-') => Err(format!(
            "Flag desconhecida no comando repl: '{}'\n\n{}",
            arg,
            repl_usage(binary)
        )),
        _ => Err(format!(
            "O comando repl não aceita argumentos posicionais.\n\n{}",
            repl_usage(binary)
        )),
    }
}

fn parse_doc_args(binary: &str, args: &[String]) -> Result<DocConfigCli, String> {
    let mut repo = ".".to_string();
    let mut corpo: Option<String> = None;
    let mut check = false;
    let mut freeze = false;
    let mut artifact: Option<String> = None;
    let mut json = false;
    let mut limite: Option<usize> = None;
    let mut subcommand: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return Err(doc_usage(binary)),
            "--repo" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                repo.clone_from(&args[i]);
            }
            "--corpo" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--corpo' requer um caminho de arquivo.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                corpo = Some(args[i].clone());
            }
            "--check" => check = true,
            "--freeze" => freeze = true,
            "--artifact" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--artifact' requer um caminho de arquivo.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                artifact = Some(args[i].clone());
            }
            "--json" => json = true,
            "--limite" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--limite' requer um valor.\n\n{}",
                        doc_usage(binary)
                    ));
                }
                let raw = &args[i];
                let value = raw.parse::<usize>().map_err(|_| {
                    format!(
                        "Valor de '--limite' inválido: '{}'\n\n{}",
                        raw,
                        doc_usage(binary)
                    )
                })?;
                limite = Some(value);
            }
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida no comando doc: '{}'\n\n{}",
                    arg,
                    doc_usage(binary)
                ));
            }
            _ => {
                if subcommand.is_none() {
                    subcommand = Some(arg.clone());
                } else {
                    positionals.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    let Some(subcommand) = subcommand else {
        return Err(doc_usage(binary));
    };

    let require_one = |what: &str| -> Result<String, String> {
        if positionals.len() != 1 {
            return Err(format!(
                "O subcomando '{}' requer exatamente um argumento.\n\n{}",
                what,
                doc_usage(binary)
            ));
        }
        Ok(positionals[0].clone())
    };
    let require_none = |what: &str| -> Result<(), String> {
        if !positionals.is_empty() {
            return Err(format!(
                "O subcomando '{}' não aceita argumentos posicionais.\n\n{}",
                what,
                doc_usage(binary)
            ));
        }
        Ok(())
    };

    let sub = match subcommand.as_str() {
        "importar-pr" => {
            let raw = require_one("importar-pr")?;
            let pr = raw.parse::<u64>().map_err(|_| {
                format!("Número de PR inválido: '{}'\n\n{}", raw, doc_usage(binary))
            })?;
            if freeze && check {
                return Err(format!(
                    "Use --freeze ou --check, não ambos.\n\n{}",
                    doc_usage(binary)
                ));
            }
            if freeze && (corpo.is_none() || artifact.is_none()) {
                return Err(format!(
                    "--freeze exige --corpo e --artifact.\n\n{}",
                    doc_usage(binary)
                ));
            }
            if !freeze && artifact.is_some() {
                return Err(format!(
                    "--artifact exige --freeze.\n\n{}",
                    doc_usage(binary)
                ));
            }
            DocSub::ImportarPr {
                pr,
                corpo,
                check,
                freeze,
                artifact,
            }
        }
        "marco" => {
            require_none("marco")?;
            DocSub::Marco
        }
        "mostrar" => DocSub::Mostrar {
            id: require_one("mostrar")?,
        },
        "listar" => DocSub::Listar {
            territorio: require_one("listar")?,
        },
        "buscar" => DocSub::Buscar {
            consulta: positionals.join(" "),
        },
        "rota" => DocSub::Rota {
            consulta: positionals.join(" "),
        },
        "sincronizar" => {
            require_none("sincronizar")?;
            DocSub::Sincronizar
        }
        "verificar" => {
            require_none("verificar")?;
            DocSub::Verificar
        }
        other => {
            return Err(format!(
                "Subcomando doc desconhecido: '{}'\n\n{}",
                other,
                doc_usage(binary)
            ));
        }
    };

    if matches!(sub, DocSub::Buscar { .. } | DocSub::Rota { .. }) && positionals.is_empty() {
        return Err(format!(
            "O subcomando '{}' requer uma consulta.\n\n{}",
            subcommand,
            doc_usage(binary)
        ));
    }

    Ok(DocConfigCli {
        repo,
        json,
        limite,
        sub,
    })
}

fn parse_nav_args(binary: &str, args: &[String]) -> Result<NavConfigCli, String> {
    let mut repo = ".".to_string();
    let mut json = false;
    let mut limite: Option<usize> = None;
    let mut observado = false;
    let mut justificativa: Option<String> = None;
    let mut predecessor: Option<String> = None;
    let mut autorizar: Option<String> = None;
    let mut diff: Option<String> = None;
    let mut subcommand: Option<String> = None;
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0usize;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return Err(nav_usage(binary)),
            "--repo" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                repo.clone_from(&args[i]);
            }
            "--json" => json = true,
            "--diff" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--diff' requer uma referência Git.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                if diff.is_some() {
                    return Err(format!(
                        "A opção '--diff' não pode ser repetida.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                diff = Some(args[i].clone());
            }
            "--observado" => observado = true,
            "--justificativa" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--justificativa' requer um valor.\n\n{}",
                        projection_usage(binary)
                    ));
                }
                justificativa = Some(args[i].clone());
            }
            "--predecessor" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--predecessor' requer um valor.\n\n{}",
                        projection_usage(binary)
                    ));
                }
                predecessor = Some(args[i].clone());
            }
            "--autorizar" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--autorizar' requer um valor.\n\n{}",
                        projection_usage(binary)
                    ));
                }
                autorizar = Some(args[i].clone());
            }
            "--limite" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--limite' requer um valor.\n\n{}",
                        nav_usage(binary)
                    ));
                }
                let raw = &args[i];
                let value = raw.parse::<usize>().map_err(|_| {
                    format!(
                        "Valor de '--limite' inválido: '{}'\n\n{}",
                        raw,
                        nav_usage(binary)
                    )
                })?;
                limite = Some(value);
            }
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida no comando nav: '{}'\n\n{}",
                    arg,
                    nav_usage(binary)
                ));
            }
            _ => {
                if subcommand.is_none() {
                    subcommand = Some(arg.clone());
                } else {
                    positionals.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    let Some(subcommand) = subcommand else {
        return Err(nav_usage(binary));
    };

    let require_one = |what: &str| -> Result<String, String> {
        if positionals.len() != 1 {
            return Err(format!(
                "O subcomando '{}' requer exatamente um argumento.\n\n{}",
                what,
                nav_usage(binary)
            ));
        }
        Ok(positionals[0].clone())
    };
    let require_none = |what: &str| -> Result<(), String> {
        if !positionals.is_empty() {
            return Err(format!(
                "O subcomando '{}' não aceita argumentos posicionais.\n\n{}",
                what,
                nav_usage(binary)
            ));
        }
        Ok(())
    };

    let has_projection_options =
        observado || justificativa.is_some() || predecessor.is_some() || autorizar.is_some();
    let sub = match subcommand.as_str() {
        "mostrar" => NavSub::Mostrar {
            key: require_one("mostrar")?,
        },
        "listar" => NavSub::Listar {
            seletor: require_one("listar")?,
        },
        "buscar" => {
            if positionals.is_empty() {
                return Err(format!(
                    "O subcomando 'buscar' requer uma consulta.\n\n{}",
                    nav_usage(binary)
                ));
            }
            NavSub::Buscar {
                consulta: positionals.join(" "),
            }
        }
        "localizar" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav localizar.\n\n{}",
                    nav_usage(binary)
                ));
            }
            NavSub::Localizar {
                symbol: require_one("localizar")?,
            }
        }
        "cobertura-diff" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav cobertura-diff.\n\n{}",
                    nav_usage(binary)
                ));
            }
            require_none("cobertura-diff")?;
            NavSub::CoberturaDiff
        }
        "impacto" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav impacto.\n\n{}",
                    nav_usage(binary)
                ));
            }
            require_none("impacto")?;
            let diff = diff
                .clone()
                .ok_or_else(|| format!("nav impacto exige --diff REF.\n\n{}", nav_usage(binary)))?;
            NavSub::Impacto { diff }
        }
        "mapa" => NavSub::Mapa {
            filtro: if positionals.is_empty() {
                None
            } else {
                Some(positionals.join(" "))
            },
        },
        "sincronizar" => {
            require_none("sincronizar")?;
            NavSub::Sincronizar
        }
        "verificar" => {
            require_none("verificar")?;
            NavSub::Verificar
        }
        "projecao" => {
            if limite.is_some() {
                return Err(format!(
                    "A opção '--limite' não pertence a nav projecao.\n\n{}",
                    projection_usage(binary)
                ));
            }
            let Some(command) = positionals.first() else {
                return Err(projection_usage(binary));
            };
            let arguments = &positionals[1..];
            let require_projection_id = || -> Result<String, String> {
                if arguments.len() != 1 {
                    return Err(format!(
                        "O subcomando '{}' requer exatamente um ID.\n\n{}",
                        command,
                        projection_subcommand_usage(binary, command)
                    ));
                }
                Ok(arguments[0].clone())
            };
            let projection = match command.as_str() {
                "listar" => {
                    if !arguments.is_empty() || has_projection_options {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Listar
                }
                "mostrar" => {
                    if justificativa.is_some() || predecessor.is_some() || autorizar.is_some() {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Mostrar {
                        id: require_projection_id()?,
                        observado,
                    }
                }
                "verificar" => {
                    if has_projection_options {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    if arguments.len() > 1 {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Verificar {
                        id: arguments.first().cloned(),
                    }
                }
                "preparar" => {
                    if observado {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Preparar {
                        id: require_projection_id()?,
                        justificativa,
                        predecessor,
                        autorizar,
                    }
                }
                "aceitar" => {
                    if observado || justificativa.is_some() || predecessor.is_some() {
                        return Err(projection_subcommand_usage(binary, command));
                    }
                    ProjectionSub::Aceitar {
                        id: require_projection_id()?,
                        autorizar,
                    }
                }
                _ => {
                    return Err(format!(
                        "Subcomando nav projecao desconhecido: '{}'.\n\n{}",
                        command,
                        projection_usage(binary)
                    ))
                }
            };
            NavSub::Projecao(projection)
        }
        other => {
            return Err(format!(
                "Subcomando nav desconhecido: '{}'\n\n{}",
                other,
                nav_usage(binary)
            ));
        }
    };

    if !matches!(sub, NavSub::Impacto { .. }) && diff.is_some() {
        return Err(format!(
            "A opção '--diff' pertence somente a nav impacto.\n\n{}",
            nav_usage(binary)
        ));
    }

    if !matches!(sub, NavSub::Projecao(_)) && has_projection_options {
        return Err(format!(
            "Opção exclusiva de nav projecao usada em '{}'.\n\n{}",
            subcommand,
            nav_usage(binary)
        ));
    }

    Ok(NavConfigCli {
        repo,
        json,
        limite,
        sub,
    })
}

fn parse_state_args(binary: &str, args: &[String]) -> Result<StateConfigCli, String> {
    let mut repo: Option<String> = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err(state_usage(binary)),
            "--repo" => {
                if repo.is_some() {
                    return Err(format!(
                        "A opção '--repo' não pode ser repetida.\n\n{}",
                        state_usage(binary)
                    ));
                }
                i += 1;
                if i >= args.len() || args[i].starts_with('-') {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        state_usage(binary)
                    ));
                }
                repo = Some(args[i].clone());
            }
            "--json" => {
                if json {
                    return Err(format!(
                        "A opção '--json' não pode ser repetida.\n\n{}",
                        state_usage(binary)
                    ));
                }
                json = true;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida no comando estado: '{}'.\n\n{}",
                    value,
                    state_usage(binary)
                ));
            }
            value => {
                return Err(format!(
                    "O comando estado não aceita argumento posicional: '{}'.\n\n{}",
                    value,
                    state_usage(binary)
                ));
            }
        }
        i += 1;
    }
    Ok(StateConfigCli {
        repo: repo.unwrap_or_else(|| ".".to_string()),
        json,
    })
}

fn parse_doctor_args(binary: &str, args: &[String]) -> Result<DoctorConfigCli, String> {
    let mut repo: Option<String> = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err(doctor_usage(binary)),
            "--repo" => {
                if repo.is_some() {
                    return Err(format!(
                        "A opção '--repo' não pode ser repetida.\n\n{}",
                        doctor_usage(binary)
                    ));
                }
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '--repo' requer um valor.\n\n{}",
                        doctor_usage(binary)
                    ));
                }
                repo = Some(args[i].clone());
            }
            "--json" if !json => json = true,
            "--json" => {
                return Err(format!(
                    "A opção '--json' não pode ser repetida.\n\n{}",
                    doctor_usage(binary)
                ))
            }
            value => {
                return Err(format!(
                    "Argumento desconhecido em doctor: '{}'.\n\n{}",
                    value,
                    doctor_usage(binary)
                ))
            }
        }
        i += 1;
    }
    Ok(DoctorConfigCli {
        repo: repo.unwrap_or_else(|| ".".to_string()),
        json,
    })
}

fn parse_verify_args(binary: &str, args: &[String]) -> Result<VerifyConfigCli, String> {
    let mut repo: Option<String> = None;
    let mut diff: Option<String> = None;
    let mut corpo: Option<PathBuf> = None;
    let mut documentation_frozen = false;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err(verify_usage(binary)),
            "--repo" | "--diff" | "--corpo" => {
                let flag = args[i].clone();
                i += 1;
                if i >= args.len() {
                    return Err(format!(
                        "Flag '{}' requer um valor.\n\n{}",
                        flag,
                        verify_usage(binary)
                    ));
                }
                match flag.as_str() {
                    "--repo" if repo.is_none() => repo = Some(args[i].clone()),
                    "--diff" if diff.is_none() => diff = Some(args[i].clone()),
                    "--corpo" if corpo.is_none() => corpo = Some(PathBuf::from(&args[i])),
                    _ => {
                        return Err(format!(
                            "A opção '{}' não pode ser repetida.\n\n{}",
                            flag,
                            verify_usage(binary)
                        ))
                    }
                }
            }
            "--documentation-frozen" if !documentation_frozen => documentation_frozen = true,
            "--json" if !json => json = true,
            "--documentation-frozen" | "--json" => {
                return Err(format!(
                    "A opção '{}' não pode ser repetida.\n\n{}",
                    args[i],
                    verify_usage(binary)
                ))
            }
            value => {
                return Err(format!(
                    "Argumento desconhecido em verificar: '{}'.\n\n{}",
                    value,
                    verify_usage(binary)
                ))
            }
        }
        i += 1;
    }
    let diff = diff.ok_or_else(|| {
        format!(
            "O comando verificar exige --diff REF.\n\n{}",
            verify_usage(binary)
        )
    })?;
    Ok(VerifyConfigCli {
        repo: repo.unwrap_or_else(|| ".".to_string()),
        diff,
        documentation_frozen,
        corpo,
        json,
    })
}
// @pinker-nav:end cli.parsing.subcomandos

// @pinker-nav:start cli.parsing.roteamento
// @pinker-nav:domain parsing
// @pinker-nav:layer cli
// @pinker-nav:summary parse_args resolve ajuda e versão, separa runtime tail e despacha os nove comandos — incluindo doctor e verificar — ou análise, com erros uniformes de uso.
pub(super) fn parse_args() -> Result<CliCommand, String> {
    let mut input: Option<String> = None;
    let mut print_tokens = false;
    let mut print_ast = false;
    let mut print_json_ast = false;
    let mut print_ir = false;
    let mut print_cfg_ir = false;
    let mut print_selected = false;
    let mut print_machine = false;
    let mut print_pseudo_asm = false;
    let mut run_program = false;
    let mut print_asm_s = false;
    let mut check_only = false;

    let raw_args: Vec<String> = env::args().collect();
    let program = program_name(raw_args.first());
    let cli_args = &raw_args[1..];
    let mut cli_tail_start = cli_args.len();
    for (i, arg) in cli_args.iter().enumerate() {
        if arg == "--" {
            cli_tail_start = i;
            break;
        }
    }
    let flag_args = &cli_args[..cli_tail_start];
    let runtime_tail = if cli_tail_start < cli_args.len() {
        &cli_args[(cli_tail_start + 1)..]
    } else {
        &[]
    };

    if matches!(flag_args.first().map(String::as_str), Some("--help" | "-h")) {
        return Ok(CliCommand::Help(usage(&program)));
    }
    if matches!(
        flag_args.first().map(String::as_str),
        Some("--version" | "-V")
    ) {
        if flag_args.len() == 1 && runtime_tail.is_empty() {
            return Ok(CliCommand::Version);
        }
        return Err(format!(
            "A opção de versão não aceita argumentos.\n\n{}",
            usage(&program)
        ));
    }
    if flag_args.first().map(String::as_str) == Some("--version-json") {
        if flag_args.len() == 1 && runtime_tail.is_empty() {
            return Ok(CliCommand::VersionJson);
        }
        return Err(format!(
            "A opção de identidade não aceita argumentos.\n\n{}",
            usage(&program)
        ));
    }
    if flag_args.first().map(String::as_str) == Some("version") {
        return Err(format!(
            "Comando 'version' desconhecido. Use '--version' ou '-V'.\n\n{}",
            usage(&program)
        ));
    }
    if flag_args.first().map(String::as_str) == Some("help") {
        return match &flag_args[1..] {
            [] if runtime_tail.is_empty() => Ok(CliCommand::Help(usage(&program))),
            [command] if runtime_tail.is_empty() => help_for_command(&program, command)
                .map(CliCommand::Help)
                .ok_or_else(|| {
                    format!(
                        "Comando desconhecido para ajuda: '{}'.\n\n{}",
                        command,
                        usage(&program)
                    )
                }),
            _ => Err(format!(
                "O comando 'help' aceita no máximo um COMANDO.\n\n{}",
                usage(&program)
            )),
        };
    }

    if let Some(cmd) = flag_args.first() {
        if cmd == "nav"
            && flag_args.get(1).map(String::as_str) == Some("projecao")
            && flag_args[2..]
                .iter()
                .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        {
            let help = flag_args
                .get(2)
                .filter(|value| !value.starts_with('-'))
                .map_or_else(
                    || projection_usage(&program),
                    |command| projection_subcommand_usage(&program, command),
                );
            return Ok(CliCommand::Help(help));
        }
        if let Some(help) = help_for_command(&program, cmd) {
            if flag_args[1..]
                .iter()
                .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
            {
                return Ok(CliCommand::Help(help));
            }
        }
        if cmd == "build" {
            return parse_build_args(&program, &flag_args[1..]).map(CliCommand::Build);
        }
        if cmd == "editor" {
            return parse_editor_args(&program, &flag_args[1..]).map(CliCommand::Editor);
        }
        if cmd == "repl" {
            return parse_repl_args(&program, &flag_args[1..]).map(CliCommand::Repl);
        }
        if cmd == "doc" {
            return parse_doc_args(&program, &flag_args[1..]).map(CliCommand::Doc);
        }
        if cmd == "nav" {
            return parse_nav_args(&program, &flag_args[1..]).map(CliCommand::Nav);
        }
        if cmd == "estado" {
            if cli_tail_start < cli_args.len() {
                return Err(format!(
                    "O comando estado não aceita argumentos após '--'.\n\n{}",
                    state_usage(&program)
                ));
            }
            return parse_state_args(&program, &flag_args[1..]).map(CliCommand::State);
        }
        if cmd == "doctor" {
            if cli_tail_start < cli_args.len() {
                return Err(format!(
                    "O comando doctor não aceita argumentos após '--'.\n\n{}",
                    doctor_usage(&program)
                ));
            }
            return parse_doctor_args(&program, &flag_args[1..]).map(CliCommand::Doctor);
        }
        if cmd == "verificar" {
            if cli_tail_start < cli_args.len() {
                return Err(format!(
                    "O comando verificar não aceita argumentos após '--'.\n\n{}",
                    verify_usage(&program)
                ));
            }
            return parse_verify_args(&program, &flag_args[1..]).map(CliCommand::Verify);
        }
    }

    for arg in flag_args {
        match arg.as_str() {
            "--tokens" => print_tokens = true,
            "--ast" => print_ast = true,
            "--json-ast" => print_json_ast = true,
            "--ir" => print_ir = true,
            "--cfg-ir" => print_cfg_ir = true,
            "--selected" => print_selected = true,
            "--machine" => print_machine = true,
            "--pseudo-asm" => print_pseudo_asm = true,
            "--asm" | "--asm-s" | "--s" => print_asm_s = true,
            "--run" => run_program = true,
            "--check" => check_only = true,
            "--help" | "-h" => return Ok(CliCommand::Help(usage(&program))),
            "--version" | "-V" => {
                return Err(format!(
                    "A opção de versão deve ser usada sem ARQUIVO.\n\n{}",
                    usage(&program)
                ));
            }
            _ if arg.starts_with('-') => {
                return Err(format!(
                    "Flag desconhecida: '{}'\n\n{}",
                    arg,
                    usage(&program)
                ));
            }
            _ => {
                if input.is_some() {
                    return Err(format!(
                        "Apenas um arquivo de entrada é suportado.\n\n{}",
                        usage(&program)
                    ));
                }
                input = Some(arg.clone());
            }
        }
    }

    let Some(input) = input else {
        return Err(format!(
            "Uso inválido: nenhum argumento informado.\n\n{}",
            usage(&program)
        ));
    };
    if !run_program && !runtime_tail.is_empty() {
        return Err(format!(
            "Argumentos após '--' exigem '--run'.\n\n{}",
            usage(&program)
        ));
    }

    Ok(CliCommand::Analyze(Config {
        input,
        print_tokens,
        print_ast,
        print_json_ast,
        print_ir,
        print_cfg_ir,
        print_selected,
        print_machine,
        print_pseudo_asm,
        run_program,
        run_args: runtime_tail.to_vec(),
        print_asm_s,
        check_only,
    }))
}
// @pinker-nav:end cli.parsing.roteamento
