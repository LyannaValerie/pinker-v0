mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.filesystem.parte-c-adulto
// @pinker-nav:domain filesystem
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da Parte C: enumeração determinística de entradas imediatas, classificação de tipo sem seguir symlink e metadata mínima atravessam `Resultado<T,E>` com paridade byte a byte entre interpretador e ELF nativo. A matriz cobre diretório vazio como sucesso, ordenação independente da ordem de criação, ocultos, as quatro classes de entrada incluindo `Outro` por socket real, nomes sem path, erro operacional distinto de coleção vazia, argumento symlink recusado, symlink interno para arquivo/diretório/alvo ausente, nome não representável em UTF-8 e composição com `propagar?`. Controles positivos impedem que a matriz passe por falhar em tudo, e a compatibilidade das superfícies históricas que **seguem** symlink é verificada lado a lado com as novas que não seguem.

/// Enumeração pura: quantidade e nomes, um por linha.
const FONTE_LISTAR: &str = r#"
pacote main;

apelido ResLista = Resultado<lista<verso>, verso>;

carinho principal() -> bombom {
    nova raiz: verso = argumento_ou(0, "ausente");
    tentar listar_diretorio(raiz) {
        sucesso ResLista.Ok(nomes) {
            falar(lista_tamanho(nomes));
            para cada nome em nomes {
                falar(nome);
            }
        }
        falha ResLista.Erro(causa) {
            falar("ERRO");
            falar(causa);
        }
    }
    falar("fim");
    mimo 0;
}
"#;

/// Enumeração repetida duas vezes no mesmo processo: a segunda passagem tem de
/// devolver exatamente a mesma sequência.
const FONTE_REPETICAO: &str = r#"
pacote main;

apelido ResLista = Resultado<lista<verso>, verso>;

carinho passagem(raiz: verso) -> bombom {
    tentar listar_diretorio(raiz) {
        sucesso ResLista.Ok(nomes) {
            para cada nome em nomes {
                falar(nome);
            }
        }
        falha ResLista.Erro(causa) { falar(causa); }
    }
    mimo 0;
}

carinho principal() -> bombom {
    nova raiz: verso = argumento_ou(0, "ausente");
    passagem(raiz);
    falar("--");
    passagem(raiz);
    mimo 0;
}
"#;

/// Classificação no-follow de cada entrada, composta por `juntar_caminho`.
const FONTE_TIPOS: &str = r#"
pacote main;

apelido ResLista = Resultado<lista<verso>, verso>;
apelido ResTipo = Resultado<TipoEntrada, verso>;

carinho classificar(caminho: verso) -> bombom {
    tentar tipo_de_entrada(caminho) {
        sucesso ResTipo.Ok(t) {
            encaixe t {
                caso TipoEntrada.Arquivo { falar("arquivo"); }
                caso TipoEntrada.Diretorio { falar("diretorio"); }
                caso TipoEntrada.Symlink { falar("symlink"); }
                caso TipoEntrada.Outro { falar("outro"); }
            }
        }
        falha ResTipo.Erro(causa) { falar(causa); }
    }
    mimo 0;
}

carinho principal() -> bombom {
    nova raiz: verso = argumento_ou(0, "ausente");
    tentar listar_diretorio(raiz) {
        sucesso ResLista.Ok(nomes) {
            para cada nome em nomes {
                falar(nome);
                classificar(juntar_caminho(raiz, nome));
            }
        }
        falha ResLista.Erro(causa) { falar(causa); }
    }
    mimo 0;
}
"#;

/// Metadata mínima como critério de seleção: nomes de arquivos regulares não
/// vazios, sem seguir symlink em nenhum passo.
const FONTE_SELECAO: &str = r#"
pacote main;

apelido ResLista = Resultado<lista<verso>, verso>;
apelido ResTipo = Resultado<TipoEntrada, verso>;
apelido ResNum = Resultado<bombom, verso>;

carinho e_arquivo_regular(caminho: verso) -> logica {
    nova muda achou: logica = falso;
    tentar tipo_de_entrada(caminho) {
        sucesso ResTipo.Ok(t) {
            encaixe t {
                caso TipoEntrada.Arquivo { achou = verdade; }
                caso TipoEntrada.Diretorio { achou = falso; }
                caso TipoEntrada.Symlink { achou = falso; }
                caso TipoEntrada.Outro { achou = falso; }
            }
        }
        falha ResTipo.Erro(causa) { achou = falso; }
    }
    mimo achou;
}

carinho principal() -> bombom {
    nova raiz: verso = argumento_ou(0, "ausente");
    tentar listar_diretorio(raiz) {
        sucesso ResLista.Ok(nomes) {
            para cada nome em nomes {
                nova cheio: verso = juntar_caminho(raiz, nome);
                talvez e_arquivo_regular(cheio) {
                    tentar tamanho_de_entrada(cheio) {
                        sucesso ResNum.Ok(n) {
                            talvez n > 0 {
                                falar(nome);
                                falar(n);
                            }
                        }
                        falha ResNum.Erro(causa) { falar(causa); }
                    }
                }
            }
        }
        falha ResLista.Erro(causa) { falar(causa); }
    }
    falar("fim");
    mimo 0;
}
"#;

/// Composição com `propagar?`: a falha da enumeração atravessa a função sem
/// braço explícito e chega ao consumidor como valor.
const FONTE_PROPAGACAO: &str = r#"
pacote main;

apelido ResLista = Resultado<lista<verso>, verso>;

carinho contar(raiz: verso) -> ResLista {
    propagar? listar_diretorio(raiz) como ResLista.Ok(nomes);
    mimo ResLista.Ok(nomes);
}

carinho principal() -> bombom {
    nova raiz: verso = argumento_ou(0, "ausente");
    tentar contar(raiz) {
        sucesso ResLista.Ok(nomes) { falar(lista_tamanho(nomes)); }
        falha ResLista.Erro(causa) { falar("propagou"); falar(causa); }
    }
    falar("fim");
    mimo 0;
}
"#;

/// Compatibilidade: as superfícies históricas continuam **seguindo** symlink e
/// as novas continuam não seguindo, lado a lado no mesmo programa.
const FONTE_COMPAT_FOLLOW: &str = r#"
pacote main;

apelido ResTipo = Resultado<TipoEntrada, verso>;

carinho principal() -> bombom {
    nova link_dir: verso = argumento_ou(0, "ausente");

    talvez e_diretorio(link_dir) {
        falar("historico_segue_diretorio");
    }

    tentar tipo_de_entrada(link_dir) {
        sucesso ResTipo.Ok(t) {
            encaixe t {
                caso TipoEntrada.Arquivo { falar("novo_arquivo"); }
                caso TipoEntrada.Diretorio { falar("novo_diretorio"); }
                caso TipoEntrada.Symlink { falar("novo_symlink"); }
                caso TipoEntrada.Outro { falar("novo_outro"); }
            }
        }
        falha ResTipo.Erro(causa) { falar(causa); }
    }
    falar("fim");
    mimo 0;
}
"#;

fn escrever_caso(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte Parte C");
    caminho
}

fn rodar_interpretador(caminho: &Path, caso: &str, args: &[String]) -> Output {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
    comando.arg("--run").arg(caminho);
    if !args.is_empty() {
        comando.arg("--");
        for arg in args {
            comando.arg(arg);
        }
    }
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar interpretador Parte C sob envelope")
}

fn compilar_nativo(
    dir: &NativeArtifactDir,
    caminho: &Path,
    runtime_lib: &Path,
    caso: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(caminho)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case(caso)
        .timeout(Duration::from_secs(120))
        .output()
        .expect("compilar Parte C sob envelope")
}

fn rodar_nativo(caminho: &Path, caso: &str, args: &[String]) -> Output {
    let mut comando = Command::new(caminho);
    for arg in args {
        comando.arg(arg);
    }
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar ELF Parte C sob envelope")
}

struct Paridade {
    stdout_interpretador: String,
    stdout_nativo: String,
    stderr_nativo: String,
    exit_interpretador: Option<i32>,
    exit_nativo: Option<i32>,
}

impl Paridade {
    /// Exige o mesmo stdout nos dois backends e exit 0 em ambos.
    ///
    /// A paridade é verificada **antes** do valor esperado: dois backends que
    /// concordassem em algo errado seriam pegos pela asserção de conteúdo, e
    /// dois backends que divergissem seriam pegos aqui mesmo quando um deles
    /// produzisse o texto certo.
    fn exigir_sucesso(&self, nome: &str, stdout_esperado: &str) {
        assert_eq!(
            self.stdout_interpretador, self.stdout_nativo,
            "{nome}: interpretador e nativo divergiram"
        );
        assert_eq!(
            self.stdout_interpretador, stdout_esperado,
            "{nome}: stdout inesperado"
        );
        assert_eq!(
            self.exit_interpretador,
            Some(0),
            "{nome}: exit interpretado"
        );
        assert_eq!(
            self.exit_nativo,
            Some(0),
            "{nome}: exit nativo (stderr: {})",
            self.stderr_nativo
        );
        assert!(
            !self.stderr_nativo.contains("panicked"),
            "{nome}: nativo entrou em pânico: {}",
            self.stderr_nativo
        );
    }

    /// Exige paridade e devolve o stdout comum, para asserções estruturais.
    fn stdout_comum(&self, nome: &str) -> &str {
        assert_eq!(
            self.stdout_interpretador, self.stdout_nativo,
            "{nome}: interpretador e nativo divergiram"
        );
        assert_eq!(
            self.exit_interpretador,
            Some(0),
            "{nome}: exit interpretado"
        );
        assert_eq!(
            self.exit_nativo,
            Some(0),
            "{nome}: exit nativo (stderr: {})",
            self.stderr_nativo
        );
        &self.stdout_interpretador
    }
}

fn paridade(nome: &str, fonte: &str, args: &[String], runtime_lib: &Path) -> Paridade {
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte C");
    let fonte_path = escrever_caso(&dir, nome, fonte);
    let interpretado = rodar_interpretador(&fonte_path, nome, args);
    let compilacao = compilar_nativo(&dir, &fonte_path, runtime_lib, nome);
    assert!(
        compilacao.status.success(),
        "{nome}: build nativo falhou: {}",
        String::from_utf8_lossy(&compilacao.stderr)
    );
    let binario = dir.path().join(nome);
    let nativo = rodar_nativo(&binario, nome, args);
    Paridade {
        stdout_interpretador: String::from_utf8_lossy(&interpretado.stdout).into_owned(),
        stdout_nativo: String::from_utf8_lossy(&nativo.stdout).into_owned(),
        stderr_nativo: String::from_utf8_lossy(&nativo.stderr).into_owned(),
        exit_interpretador: interpretado.status.code(),
        exit_nativo: nativo.status.code(),
    }
}

/// Árvore de fixture com as quatro classes de entrada e ordem de criação
/// deliberadamente diferente da ordem esperada de saída.
///
/// Devolve a raiz e a lista ordenada esperada.
fn montar_arvore(base: &Path) -> (PathBuf, Vec<&'static str>) {
    let raiz = base.join("arvore");
    fs::create_dir(&raiz).expect("criar raiz da árvore");

    // Ordem de criação: zeta, meio, sub, alfa, oculto — nenhuma relação com a
    // ordem esperada. Se a implementação devolvesse a ordem do filesystem, o
    // resultado dependeria dessa sequência.
    fs::write(raiz.join("zeta.txt"), "zzz").expect("zeta");
    fs::write(raiz.join("meio.txt"), "mm").expect("meio");
    fs::create_dir(raiz.join("sub")).expect("sub");
    fs::write(raiz.join("alfa.txt"), "a").expect("alfa");
    fs::write(raiz.join(".oculto"), "o").expect("oculto");
    fs::write(raiz.join("vazio.txt"), "").expect("vazio");

    std::os::unix::fs::symlink("alfa.txt", raiz.join("link_arquivo")).expect("link arquivo");
    std::os::unix::fs::symlink("sub", raiz.join("link_dir")).expect("link dir");
    std::os::unix::fs::symlink("nao_existe", raiz.join("link_quebrado")).expect("link quebrado");

    // `Outro`: um socket unix é reproduzível com a std, sem ferramenta externa.
    let _soquete = UnixListener::bind(raiz.join("soquete")).expect("socket unix");
    std::mem::forget(_soquete);

    let esperado = vec![
        ".oculto",
        "alfa.txt",
        "link_arquivo",
        "link_dir",
        "link_quebrado",
        "meio.txt",
        "soquete",
        "sub",
        "vazio.txt",
        "zeta.txt",
    ];
    (raiz, esperado)
}

fn arg(caminho: &Path) -> Vec<String> {
    vec![caminho.to_string_lossy().into_owned()]
}

#[test]
fn filesystem_adulto_enumera_classifica_e_mede_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório de fixture Parte C");
    let (raiz, esperado) = montar_arvore(dir.path());

    // ---- Enumeração: ordem determinística, ocultos incluídos, sem "." nem
    //      "..", nomes e não paths, ordem independente da criação.
    let listagem = paridade("listar", FONTE_LISTAR, &arg(&raiz), &runtime_lib);
    let mut esperado_stdout = format!("{}\n", esperado.len());
    for nome in &esperado {
        esperado_stdout.push_str(nome);
        esperado_stdout.push('\n');
    }
    esperado_stdout.push_str("fim\n");
    listagem.exigir_sucesso("enumeração determinística", &esperado_stdout);

    let saida = listagem.stdout_comum("enumeração determinística");
    let nomes: Vec<&str> = saida
        .lines()
        .skip(1)
        .take_while(|linha| *linha != "fim")
        .collect();
    for nome in &nomes {
        assert!(
            !nome.contains('/'),
            "a enumeração devolveu path e não nome: {nome}"
        );
    }
    assert!(
        !nomes.contains(&".") && !nomes.contains(&".."),
        "a enumeração incluiu '.' ou '..'"
    );
    assert!(
        nomes.contains(&".oculto"),
        "a enumeração omitiu entrada oculta"
    );
    let mut ordenado = nomes.clone();
    ordenado.sort_unstable();
    assert_eq!(
        nomes, ordenado,
        "a enumeração não veio ordenada pelos bytes UTF-8"
    );

    // ---- Repetição: mesma sequência duas vezes no mesmo processo.
    let repetido = paridade("repeticao", FONTE_REPETICAO, &arg(&raiz), &runtime_lib);
    let saida = repetido.stdout_comum("repetição");
    let partes: Vec<&str> = saida.split("--\n").collect();
    assert_eq!(partes.len(), 2, "repetição não produziu duas passagens");
    assert_eq!(
        partes[0], partes[1],
        "enumerações repetidas divergiram no mesmo processo"
    );

    // ---- Classificação no-follow das quatro classes.
    let tipos = paridade("tipos", FONTE_TIPOS, &arg(&raiz), &runtime_lib);
    let saida = tipos.stdout_comum("classificação");
    let pares: Vec<(&str, &str)> = saida
        .lines()
        .collect::<Vec<_>>()
        .chunks(2)
        .filter(|par| par.len() == 2)
        .map(|par| (par[0], par[1]))
        .collect();
    let classe = |nome: &str| -> &str {
        pares
            .iter()
            .find(|(entrada, _)| *entrada == nome)
            .map(|(_, tipo)| *tipo)
            .unwrap_or_else(|| panic!("entrada '{nome}' ausente da classificação"))
    };
    assert_eq!(classe("alfa.txt"), "arquivo");
    assert_eq!(classe("sub"), "diretorio");
    assert_eq!(
        classe("soquete"),
        "outro",
        "socket unix deveria ser 'outro'"
    );
    // O ponto central do contrato no-follow: os três symlinks são `symlink`,
    // inclusive o que aponta para diretório e o que está quebrado. Se alguma
    // camada seguisse o alvo, estes três valores mudariam.
    assert_eq!(
        classe("link_arquivo"),
        "symlink",
        "symlink para arquivo foi seguido"
    );
    assert_eq!(
        classe("link_dir"),
        "symlink",
        "symlink para diretório foi seguido"
    );
    assert_eq!(
        classe("link_quebrado"),
        "symlink",
        "symlink quebrado não deveria ser erro nem outro tipo"
    );

    // ---- Metadata mínima como critério de seleção.
    let selecao = paridade("selecao", FONTE_SELECAO, &arg(&raiz), &runtime_lib);
    // `.oculto` é arquivo regular não vazio e por isso entra na seleção; os
    // symlinks, o diretório, o socket e `vazio.txt` ficam de fora — cada um por
    // um critério diferente do contrato.
    selecao.exigir_sucesso(
        "seleção por metadata mínima",
        ".oculto\n1\nalfa.txt\n1\nmeio.txt\n2\nzeta.txt\n3\nfim\n",
    );

    // ---- Diretório vazio: sucesso com coleção vazia, jamais erro.
    let vazio = dir.path().join("vazio");
    fs::create_dir(&vazio).expect("criar diretório vazio");
    let listagem_vazia = paridade("vazio", FONTE_LISTAR, &arg(&vazio), &runtime_lib);
    listagem_vazia.exigir_sucesso("diretório vazio", "0\nfim\n");

    // ---- Uma única entrada.
    let unico = dir.path().join("unico");
    fs::create_dir(&unico).expect("criar diretório de uma entrada");
    fs::write(unico.join("so-esta.txt"), "x").expect("única entrada");
    let listagem_unica = paridade("unico", FONTE_LISTAR, &arg(&unico), &runtime_lib);
    listagem_unica.exigir_sucesso("uma entrada", "1\nso-esta.txt\nfim\n");

    // ---- Compatibilidade: histórico segue symlink, novo não segue.
    let link_dir = raiz.join("link_dir");
    let compat = paridade("compat", FONTE_COMPAT_FOLLOW, &arg(&link_dir), &runtime_lib);
    compat.exigir_sucesso(
        "compatibilidade de política de follow",
        "historico_segue_diretorio\nnovo_symlink\nfim\n",
    );
}

#[test]
fn falha_operacional_de_filesystem_e_valor_e_nunca_colecao_vazia() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório de erro Parte C");
    let (raiz, _) = montar_arvore(dir.path());

    // Controle positivo: a mesma fonte produz sucesso num diretório real. Sem
    // isto, a matriz de erros passaria mesmo se tudo falhasse sempre.
    let vazio = dir.path().join("controle-vazio");
    fs::create_dir(&vazio).expect("controle");
    paridade("controle", FONTE_LISTAR, &arg(&vazio), &runtime_lib)
        .exigir_sucesso("controle positivo", "0\nfim\n");

    // Cada caso abaixo tem de produzir ERRO — nunca "0" (coleção vazia).
    let casos: Vec<(&str, PathBuf)> = vec![
        ("ausente", dir.path().join("nao-existe-mesmo")),
        ("nao_diretorio", raiz.join("alfa.txt")),
        ("argumento_symlink_dir", raiz.join("link_dir")),
        ("argumento_symlink_quebrado", raiz.join("link_quebrado")),
    ];

    for (nome, caminho) in casos {
        let resultado = paridade(nome, FONTE_LISTAR, &arg(&caminho), &runtime_lib);
        let saida = resultado.stdout_comum(nome);
        assert!(
            saida.starts_with("ERRO\n"),
            "{nome}: falha operacional deveria virar Erro, obtido: {saida}"
        );
        assert!(
            !saida.starts_with("0\n"),
            "{nome}: ERRO_TRANSFORMADO_EM_COLECAO_VAZIA"
        );
        assert!(
            saida.contains("falha ao listar diretório"),
            "{nome}: causa não chegou ao consumidor: {saida}"
        );
        assert!(
            saida.ends_with("fim\n"),
            "{nome}: o programa deveria continuar após a falha"
        );
    }

    // O argumento symlink é recusado por política, com causa própria — não é o
    // erro genérico do sistema operacional.
    let symlink_dir = paridade(
        "explica_symlink",
        FONTE_LISTAR,
        &arg(&raiz.join("link_dir")),
        &runtime_lib,
    );
    assert!(
        symlink_dir
            .stdout_comum("explica_symlink")
            .contains("link simbólico"),
        "a recusa do argumento symlink deveria nomear a política"
    );

    // Composição: a falha atravessa `propagar?` e chega ao consumidor.
    let propagado = paridade(
        "propagacao",
        FONTE_PROPAGACAO,
        &arg(&dir.path().join("nao-existe-mesmo")),
        &runtime_lib,
    );
    let saida = propagado.stdout_comum("propagação");
    assert!(
        saida.starts_with("propagou\n"),
        "a falha não atravessou propagar?: {saida}"
    );
    assert!(saida.ends_with("fim\n"), "o consumidor não retomou");

    // Controle: a mesma função propaga sucesso normalmente.
    paridade(
        "propagacao_ok",
        FONTE_PROPAGACAO,
        &arg(&vazio),
        &runtime_lib,
    )
    .exigir_sucesso("propagação de sucesso", "0\nfim\n");
}

#[test]
fn nome_nao_representavel_como_verso_falha_sem_conversao_lossy() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório UTF-8 Parte C");
    let raiz = dir.path().join("utf8");
    fs::create_dir(&raiz).expect("criar raiz utf8");

    // Controle positivo: com nomes válidos, incluindo não-ASCII, a enumeração
    // funciona. Sem isto o caso negativo passaria por motivo errado.
    fs::write(raiz.join("acentuação.txt"), "x").expect("nome unicode válido");
    fs::write(raiz.join("simples.txt"), "y").expect("nome ascii");
    paridade("utf8_valido", FONTE_LISTAR, &arg(&raiz), &runtime_lib).exigir_sucesso(
        "nomes UTF-8 válidos",
        "2\nacentuação.txt\nsimples.txt\nfim\n",
    );

    // Agora um nome que não é UTF-8 válido. Se o host não aceitar criá-lo, o
    // caso é classificado explicitamente em vez de silenciosamente pulado.
    let invalido = std::ffi::OsStr::from_bytes(b"invalido-\xff\xfe");
    let caminho_invalido = raiz.join(invalido);
    if fs::write(caminho_invalido, "z").is_err() {
        eprintln!(
            "{{\"event\":\"utf8_evidence\",\"status\":\"unavailable\",\
             \"reason\":\"host_rejeitou_nome_nao_utf8\"}}"
        );
        return;
    }

    let resultado = paridade("utf8_invalido", FONTE_LISTAR, &arg(&raiz), &runtime_lib);
    let saida = resultado.stdout_comum("nome não representável");
    assert!(
        saida.starts_with("ERRO\n"),
        "nome não representável deveria falhar a operação inteira: {saida}"
    );
    assert!(
        saida.contains("UTF-8 inválido"),
        "a causa deveria nomear a razão: {saida}"
    );
    // O ponto do contrato: a entrada não foi pulada nem substituída. Se tivesse
    // sido pulada, a listagem teria sucedido com 2 nomes; se tivesse sido
    // convertida, teria sucedido com 3 e um nome com U+FFFD.
    assert!(
        !saida.starts_with("2\n") && !saida.starts_with("3\n"),
        "INVALID_UTF8_FILENAME virou lossy ou foi pulado: {saida}"
    );
    assert!(
        !saida.contains('\u{FFFD}'),
        "houve conversão lossy do nome inválido: {saida}"
    );
}

/// A autoridade da taxonomia é única: os nomes públicos do leque e das suas
/// variantes não são redeclarados por nenhuma camada.
#[test]
fn taxonomia_de_entrada_tem_autoridade_unica() {
    use pinker_v0::tipo_entrada::{TipoEntrada, LEQUE_TIPO_ENTRADA, VARIANTES};

    // Os discriminantes são a ordem de declaração e estão fixados: reordenar
    // VARIANTES muda valores observáveis nos dois backends.
    assert_eq!(TipoEntrada::Arquivo.discriminante(), 0);
    assert_eq!(TipoEntrada::Diretorio.discriminante(), 1);
    assert_eq!(TipoEntrada::Symlink.discriminante(), 2);
    assert_eq!(TipoEntrada::Outro.discriminante(), 3);
    assert_eq!(VARIANTES.len(), 4);

    // A superfície que devolve a taxonomia deriva o nome do leque da autoridade
    // — não de um literal próprio.
    let classificadora = pinker_v0::falha_operacional::SUPERFICIES_FALIVEIS
        .iter()
        .find(|superficie| superficie.intrinseca == "tipo_de_entrada")
        .expect("superfície de classificação declarada");
    assert_eq!(
        classificadora.sucesso.chave(),
        LEQUE_TIPO_ENTRADA,
        "a carga de sucesso deveria ser o leque da autoridade"
    );

    // Nenhum arquivo de `src/` fora da autoridade declara os nomes das
    // variantes. Guarda regressiva no mesmo espírito da Parte B.
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut fontes = Vec::new();
    fontes_rust(&raiz, &mut fontes);
    let autoridade = raiz.join("tipo_entrada.rs");
    let mut controle_positivo = false;
    for fonte in fontes {
        let texto = fs::read_to_string(&fonte).expect("ler fonte");
        if fonte == autoridade {
            controle_positivo = texto.contains("\"Diretorio\"");
            continue;
        }
        assert!(
            !texto.contains("\"Diretorio\""),
            "o nome público de variante reapareceu fora da autoridade: {}",
            fonte.display()
        );
    }
    assert!(
        controle_positivo,
        "o teste não localizou o nome na própria autoridade; está lendo o alvo errado"
    );
}

fn fontes_rust(raiz: &Path, destino: &mut Vec<PathBuf>) {
    for entrada in fs::read_dir(raiz).expect("diretório legível") {
        let caminho = entrada.expect("entrada de diretório").path();
        if caminho.is_dir() {
            fontes_rust(&caminho, destino);
        } else if caminho.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            destino.push(caminho);
        }
    }
}

// @pinker-nav:end evidencia.filesystem.parte-c-adulto
