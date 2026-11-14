# Wattly

API de credenciamento da Wattly: a pessoa cria uma conta, abre o dossiê de quem vai oferecer energia, cadastra o ponto no mapa, anexa as referências dos documentos e só então envia para revisão. A equipe aprova com um checklist. O mapa público mostra o ponto com coordenada arredondada. O endereço e o documento completo ficam com o dono e com o administrador.

Esta entrega cobre a área **DevOps, segurança e qualidade** da Global Solution.

## Como rodar

Pré-requisitos: Rust 1.91. Fora do Docker, o processo precisa achar o Oracle Instant Client (`oci.dll` no `PATH`). A imagem Docker já traz o client.

```powershell
cd api
copy .env.example .env
# preencha ORACLE_PASSWORD, JWT_SECRET e WATTLY_ADMIN_PASSWORD
cargo run
```

A API escuta em `http://localhost:8080`. O contrato OpenAPI fica em `http://localhost:8080/openapi.yaml` e a interface em `http://localhost:8080/docs`. O arquivo versionado é `api/openapi.yaml`.

Nesta máquina o Rust padrão procura o linker do Visual Studio, que não está instalado. O compilador GNU já está disponível. Num terminal novo:

```powershell
$env:RUSTUP_TOOLCHAIN = "1.91.1-x86_64-pc-windows-gnu"
cd api
cargo run
```

O Instant Client também já foi instalado. Um terminal aberto antes dessa instalação não enxerga o `oci.dll`; abra outro.

No ambiente local deste projeto o arquivo `api/.env` já está preenchido e está no `.gitignore`. O administrador de demonstração entra com `admin@wattly.dev` e a senha definida em `WATTLY_ADMIN_PASSWORD`.

### Docker

Na raiz do repositório, com o Docker Desktop ligado:

```powershell
docker compose up --build
```

A senha chega só pela variável de ambiente lida de `api/.env`. Ela não é copiada para a imagem.

### Conferir o banco

```powershell
cd api
cargo test -- --ignored oracle_grava_e_le_um_usuario --nocapture
```

Esse teste conecta no Oracle, grava um usuário e um credenciamento e apaga os dois. Os outros testes não usam o banco.

## O que a API guarda

| Tabela | Conteúdo |
| --- | --- |
| `WATTLY_USERS` | Conta, papel, hash da senha, consentimento e status. |
| `WATTLY_CLIENTS` | Dossiê de quem oferece energia, com Pix, situação e decisão. |
| `WATTLY_LOCATIONS` | Endereço privado, coordenada exata, coordenada pública, conector, potência e preço. |
| `WATTLY_WINDOWS` | Dias e horários em que o ponto funciona. |
| `WATTLY_DOCUMENTS` | Referência de identidade, comprovante, foto do carregador e laudo. |
| `WATTLY_CHECKS` | Checklist da revisão: identidade, endereço, parte elétrica e conector. |
| `WATTLY_CRED_EVENTS` | Histórico imutável de cada mudança do credenciamento. |

O schema é criado na subida, no usuário Oracle informado no `.env`.

## Fluxo

1. `POST /api/v1/auth/register` cria motorista (`driver`), anfitrião (`host`) ou os dois (`both`).
2. `POST /api/v1/auth/login` devolve um JWT.
3. `POST /api/v1/clients` abre o dossiê em `draft`.
4. `POST /api/v1/clients/{id}/locations` cadastra o ponto. Preço zero, CEP inválido ou horário invertido são recusados.
5. `POST /api/v1/clients/{id}/documents` registra identidade, comprovante de endereço e foto do carregador.
6. `POST /api/v1/clients/{id}/submit` só segue se o ponto e os três documentos existirem.
7. O administrador chama `POST /api/v1/clients/{id}/review` com `start` e depois `approve`. Aprovar direto de `submitted` é recusado. Aprovar exige os quatro itens do checklist.
8. `GET /api/v1/locations?lat=&lng=&radius_km=` lista só pontos aprovados e ativos, sem rua e sem instrução de acesso.
9. `GET /api/v1/clients/{id}/dossier` junta cliente, documentos, checklist, eventos e pontos.

Papéis e status possíveis da revisão: `draft`, `submitted`, `under_review`, `approved`, `rejected`, `suspended`, `revoked`. Uma recusa volta para rascunho com `POST /api/v1/clients/{id}/reopen`.

## Segurança

- Senha com Argon2id. O texto da senha não é armazenado nem escrito em log.
- Login de e-mail inexistente também verifica um hash, para o tempo de resposta não entregar se a conta existe.
- Token JWT com emissor `wattly` e expiração. Rotas protegidas conferem de novo se a conta continua `active`.
- CPF e CNPJ são validados pelo dígito. Nas respostas mascaradas só o final aparece. O mapa não recebe o documento.
- Coordenada pública tem duas casas decimais, cerca de um quilômetro. Endereço, complemento e código da vaga só voltam para o dono ou para o administrador.
- SQL usa parâmetros vinculados. A senha do Oracle e o segredo do JWT vêm de variável de ambiente. O `Debug` da configuração mascara os dois.

## Testes

```powershell
cd api
cargo test
```

| Teste | Critério |
| --- | --- |
| `health_responde_ok` | Serviço no ar. |
| `cadastro_rejeita_cpf_invalido_e_senha_fraca` | Validação de entrada. |
| `cadastro_conflita_quando_o_email_se_repete` | Conflito e máscara do documento. |
| `login_rejeita_senha_errada` | Autenticação. |
| `fluxo_de_credenciamento_aprova_e_esconde_o_endereco` | Fluxo completo, transição inválida e privacidade do endereço. |
| `usuario_suspenso_nao_entra` | Conta suspensa não autentica. |
| Testes de domínio | CPF, CNPJ, preço, transição, distância e máscaras. |

A saída desse comando é a evidência da execução para o PDF.

## Esteira

O arquivo `.github/workflows/ci.yml` faz, em todo push e pull request:

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all-targets`
4. `cargo build --release`
5. `cargo audit`, registrado mesmo se houver advisory
6. build da imagem Docker, sem enviar para um registro

A senha do Oracle não é secret da esteira. O job de qualidade não abre conexão com a FIAP.

## Declaração de IA generativa

A estrutura da API, o texto deste README e o planejamento foram produzidos com apoio de IA generativa no Cursor. O grupo revisa o código, executa os testes e altera o que não representar a solução entregue.
