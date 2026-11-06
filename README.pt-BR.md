[Read in English](README.md)

# Pinscher

Pinscher é um gerenciador da barra de menus do macOS que permite ocultar os itens que você não precisa ver o tempo todo.

## Status

Em desenvolvimento inicial. O comportamento e a configuração podem mudar entre versões.

## Recursos

- **Disponível:** Mostrar ou ocultar itens da barra de menus com um clique.
- **Disponível:** Mover o separador com ⌘ e arrastar para escolher quais itens ficam ocultos à esquerda dele.
- **Disponível:** Alternar a visibilidade com ⌥⌘P e configurar ocultação automática e início com o login nas Preferências.
- **Planejado:** Uma segunda barra, um modo sempre oculto e reordenação de itens nas Preferências.

## Requisitos

- macOS 14 ou posterior
- Mac com Apple silicon ou Intel

## Instalação

Baixe uma versão em [GitHub Releases](https://github.com/djbrunoramon/pinscher/releases) e mova `Pinscher.app` para Aplicativos.

Os bundles de release têm assinatura ad-hoc. Se o Gatekeeper bloquear uma versão em que você confia, clique com o botão direito (ou Control-clique) no app e escolha **Abrir**. Também é possível remover a quarentena de um app baixado em que você confia pelo Terminal:

```sh
xattr -dr com.apple.quarantine /Applications/Pinscher.app
```

## Uso

- Clique no ícone do Pinscher na barra de menus para mostrar ou ocultar os itens à esquerda do separador.
- Clique com o botão direito no ícone ou pressione ⌃ enquanto clica para abrir o menu.
- Pressione ⌥⌘P para alternar a visibilidade.
- Segure ⌘ e arraste um item da barra de menus para escolher quais itens permanecem ocultos à esquerda do separador. O separador precisa ficar à esquerda do ícone do Pinscher; caso contrário, o Pinscher mostra um aviso e não oculta os itens.
- Ative **Iniciar com o login** pelo menu. Isso funciona quando o Pinscher é executado a partir do app e pode exigir aprovação nos Ajustes do Sistema.
- As Preferências incluem configurações de ocultação automática e do atalho global.

## Permissões e privacidade

O recurso básico de mostrar e ocultar não solicita permissões de Gravação de Tela nem de Acessibilidade. Recursos avançados que inspecionem ou controlem outros itens da barra de menus podem exigir Gravação de Tela para capturar sua aparência e Acessibilidade para interagir com eles. Esses recursos estão planejados e não fazem parte do comportamento básico atual.

O Pinscher não faz solicitações de rede e não tem telemetria. Capturas usadas por recursos avançados ficam na memória; o Pinscher não salva imagens capturadas.

## Compilar a partir do código-fonte

Instale o Rust com rustup; ele lê a versão fixada em `rust-toolchain.toml`. Depois, execute:

```sh
cargo build
cargo test
./scripts/bundle.sh
```

O script de bundle cria `dist/Pinscher.app` universal e com assinatura ad-hoc.

## Contribuir

Consulte [CONTRIBUTING.md](CONTRIBUTING.md) para ver a configuração do ambiente, os limites do projeto e o processo de pull request. Relate bugs em [GitHub Issues](https://github.com/djbrunoramon/pinscher/issues).

## Licença

O Pinscher é distribuído sob a [Licença MIT](LICENSE).
