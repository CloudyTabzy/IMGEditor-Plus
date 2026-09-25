### IMG Editor Plus - Português (Brasil).
###
### Draft translation; pending review by a native speaker.
### Keep message ids and { $variables } exactly as in en.ftl.

## Menu bar: root menus

menu-file = Arquivo
menu-recent = Recentes
menu-edit = Editar
menu-selection = Seleção
menu-view = Exibir
menu-themes = Temas
menu-help = Ajuda
menu-language = Idioma

## File menu

menu-file-new = Novo ({ $shortcut })
menu-file-open = Abrir… ({ $shortcut })
menu-file-save = Salvar ({ $shortcut })
menu-file-save-as = Salvar como… ({ $shortcut })
menu-file-pack = Compactar arquivo
menu-file-set-game-folder = Definir pasta do jogo…
menu-file-reset-game-folder = Redefinir pasta do jogo
menu-file-close-tab = Fechar aba ({ $shortcut })
menu-file-sort-by = Ordenar por…

## Recent menu

menu-recent-empty = Nenhum arquivo recente

## Edit menu

menu-edit-import = Importar ({ $shortcut })
menu-edit-import-folder = Importar pasta
menu-edit-export-all = Exportar tudo ({ $shortcut })
menu-edit-export-selected = Exportar selecionados ({ $shortcut })
menu-edit-export-list = Exportar como lista ({ $shortcut })
menu-edit-compare-list = Comparar com lista ({ $shortcut })
menu-edit-load-agr = Carregar arquivo de animação .agr…

## Selection menu

menu-selection-all = Selecionar tudo ({ $shortcut })
menu-selection-invert = Inverter seleção ({ $shortcut })
menu-selection-clear = Limpar seleção ({ $shortcut })
menu-selection-delete = Excluir selecionados ({ $shortcut })

## View menu (toggles; the on/off marker is added by the code)

menu-view-navigation-gizmo = Gizmo de navegação
menu-view-search-bar = Barra de pesquisa
menu-view-search-selection-context = Contexto da seleção na pesquisa
menu-view-literal-file-types = Tipos de arquivo literais
menu-view-highlight-validator-rows = Destacar linhas do validador
menu-view-context-accumulates = Clique direito adiciona à seleção
menu-view-autoscroll-momentum = Inércia da rolagem automática
menu-view-motion-effects = Efeitos de movimento
menu-view-selection-pulse = Pulsação da seleção
menu-view-click-ripples = Ondas ao clicar
menu-view-icon-micro-motion = Micromovimento dos ícones
menu-view-animation-demo = Demonstração de animação (sintética)
menu-view-explorer-association = Abrir .img/.dir pelo Explorador

## Themes menu (named themes such as Catppuccin Mocha keep their names)

theme-dark = Escuro
theme-light = Claro

## Help menu

menu-help-check-updates = Verificar atualizações ({ $shortcut })
menu-help-repository = Visitar repositório
menu-help-about = Sobre

## Language menu (each language is listed under its own name)

# $language is the detected system language's own name, e.g. "Español".
menu-language-system = Idioma do sistema ({ $language })
menu-language-pseudo = Pseudolocalização (teste de layout)

## Entry context menu (right-click on an archive entry)

# Shown under the entry name when the right-click extends a selection.
context-more-selected =
    { $count ->
        [one] +{ $count } selecionado
       *[other] +{ $count } selecionados
    }
context-play-animation = Reproduzir animação
context-open-3d = Abrir no visualizador 3D
context-open-external = Abrir em visualizador externo
context-view-textures = Ver texturas
context-export-companion-textures = Exportar texturas NFT associadas
context-export-embedded-textures = Exportar texturas incorporadas
context-export = Exportar
context-rename = Renomear
context-copy-name = Copiar nome
context-delete = Excluir

## Shared dialog buttons and labels

button-close = Fechar
button-cancel = Cancelar
button-save = Salvar
button-discard = Descartar
button-apply = Aplicar
button-reset = Redefinir
button-convert = Converter
button-planning = Planejando…
field-format = Formato:
field-name = Nome:
# Stands in for a game name in sentences like "checked against { $target }".
target-unset = o destino
target-unset-selected = o destino selecionado

## About dialog

dialog-about-title = Sobre
about-body =
    IMG Editor Plus v{ $version }

    Um editor de desktop em Rust puro para arquivos IMG do GTA.

    Feito por CloudyTabzy e agentes
    Baseado no IMG Editor original de Grinch_
    (https://github.com/user-grinch/IMGEditor)

    Formatos compatíveis:
    - GTA III
    - GTA Vice City
    - GTA San Andreas
    - Bully Scholarship Edition
about-visit-repository = Visitar repositório

## Welcome dialog

dialog-welcome-title = Boas-vindas
welcome-heading = Boas-vindas ao { $app } v{ $version }
welcome-tagline = Um editor de arquivos GTA para III, VC, San Andreas e Bully SE.
welcome-dont-show = Não mostrar esta mensagem novamente
welcome-disable-updates = Desativar verificação de atualizações
welcome-get-started = Começar

## Unsupported format dialog

dialog-unsupported-title = Formato não compatível
unsupported-body = Formato IMG não compatível.
unsupported-path = Caminho: { $path }
unsupported-supported = Formatos compatíveis: GTA III, Vice City, San Andreas, Bully SE.

## Texture converter dialogs (replace texture, image as TXD, bulk convert)

format-option-native = { $format } (nativo)
format-option-opt-in = { $format } (opcional)
dxt-high-quality = DXT de alta qualidade (cluster fit, mais lento)
preview-full-quality = Ver em qualidade máxima
preview-full-quality-title = { $label } — qualidade máxima
preview-navigation-hint = Role para ampliar · arraste para mover · Esc para fechar
preview-current = Atual
preview-after = Depois (codificado)
dialog-target = Destino: { $target }
dialog-target-archive = Destino: { $target } · { $archive }
dialog-replace-title = Substituir textura
replace-summary = Substituindo “{ $texture }” - origem: { $source } ({ $width }x{ $height })
replace-override-note = Armazenada na memória como substituição; o arquivo só muda quando você salvar.
replace-confirm = Substituir textura
dialog-new-txd-title = Importar imagem como TXD
new-txd-summary = Novo TXD a partir de { $source } ({ $width }x{ $height })
new-txd-name-placeholder = nome da textura
new-txd-confirm = Adicionar ao arquivo
dialog-bulk-title = Converter para o dialeto do destino
bulk-entry =
    { $entry } - { $count ->
        [one] { $count } textura
       *[other] { $count } texturas
    } -> { $formats }
bulk-more-entries =
    { $count ->
        [one] … e mais { $count } entrada
       *[other] … e mais { $count } entradas
    }
bulk-summary =
    { $textures ->
        [one] { $textures } textura
       *[other] { $textures } texturas
    } em { $entries ->
        [one] { $entries } entrada
       *[other] { $entries } entradas
    } de { $source } serão recodificadas para o destino.
bulk-skipped = { $skipped } já nativas (ignoradas), { $failed } ilegíveis (ignoradas).
bulk-ignored =
    { $count ->
        [one] { $count } das entradas selecionadas não é um contêiner TXD e permanecerá intacta.
       *[other] { $count } das entradas selecionadas não são contêineres TXD e permanecerão intactas.
    }
bulk-verbatim-note = Texturas e nomes intactos permanecem como estão; o arquivo só muda quando você salvar.

## Compare with list dialog

dialog-compare-title = Comparar com lista
compare-manifest = Manifesto: { $path }
compare-archive =
    Arquivo: { $archive } · { $count ->
        [one] { $count } entrada no arquivo
       *[other] { $count } entradas no arquivo
    }
compare-stats =
    { $names ->
        [one] { $names } nome no manifesto
       *[other] { $names } nomes no manifesto
    } ({ $unique } únicos) · { $matched } correspondentes · { $missing } ausentes ({ $missing-unique } únicos)
compare-case-sensitive = Diferenciar maiúsculas e minúsculas
compare-show-archive-only = Mostrar entradas exclusivas do arquivo
compare-duplicate-lines =
    { $count ->
        [one] { $count } linha duplicada no manifesto
       *[other] { $count } linhas duplicadas no manifesto
    }
compare-blank-lines =
    { $count ->
        [one] { $count } linha em branco ignorada
       *[other] { $count } linhas em branco ignoradas
    }
compare-missing-heading = Ausentes no arquivo ({ $count })
compare-no-missing = Nenhuma entrada ausente encontrada.
compare-more-missing =
    { $count ->
        [one] … e mais { $count } nome ausente
       *[other] … e mais { $count } nomes ausentes
    }
compare-archive-only-heading = Entradas exclusivas do arquivo ({ $count })
compare-no-archive-only = Nenhuma entrada exclusiva do arquivo encontrada.
compare-more-archive-only =
    { $count ->
        [one] … e mais { $count } nome exclusivo do arquivo
       *[other] … e mais { $count } nomes exclusivos do arquivo
    }
compare-copy-missing = Copiar nomes ausentes
compare-running-heading = Comparando a lista de entradas
compare-running-body = Lendo { $manifest } e comparando com { $archive }…

## Save check dialog

dialog-save-check-title = Verificação ao salvar
save-check-summary =
    { $textures ->
        [0] { $textures } texturas
        [one] { $textures } textura
       *[other] { $textures } texturas
    }, { $entries ->
        [0] { $entries } entradas
        [one] { $entries } entrada
       *[other] { $entries } entradas
    } - verificado em relação a { $target }.
save-check-counts = { $fine } nativas/compatíveis · { $convertible } conversíveis (sem perdas) · { $incompatible } incompatíveis · { $unknown } não verificadas
# $note is a technical detail and stays in English.
save-check-container = Contêiner: { $note }
save-check-anomaly = { $code }: { $count } (ex.: { $example })
save-check-broken-headers = Cabeçalhos corrompidos (corrigíveis sem recodificar):
save-check-warnings =
    { $count ->
        [one] { $count } anomalia de nível de aviso (reportada, não bloqueante).
       *[other] { $count } anomalias de nível de aviso (reportadas, não bloqueantes).
    }
save-check-repair =
    Corrigir cabeçalhos DXT inconsistentes antes de salvar ({ $count ->
        [one] { $count } relatório
       *[other] { $count } relatórios
    }, sem perdas)
save-check-repair-note = A correção ajusta os campos do cabeçalho DXT no lugar; nenhum pixel é recodificado.
save-check-verbatim-note = Salvar grava todas as entradas como estão; nenhuma textura é recodificada ou convertida.
save-check-fix-and-save = Corrigir e salvar
save-check-save-anyway = Salvar mesmo assim

## Unsaved changes dialog

dialog-unsaved-title = Alterações não salvas
unsaved-archive = “{ $archive }” tem alterações não salvas.
unsaved-archive-note = Fechar sem salvar descarta essas alterações; o arquivo no disco permanece intacto.
unsaved-window =
    { $count ->
        [one] { $count } arquivo tem alterações não salvas: { $archives }
       *[other] { $count } arquivos têm alterações não salvas: { $archives }
    }
unsaved-window-note = Sair agora descarta essas alterações; os arquivos no disco permanecem intactos.
unsaved-discard-and-quit = Descartar alterações e sair

## Import check dialog

dialog-import-check-title = Verificação de importação
import-check-offender = { $name }: { $verdict }
import-check-offender-note = { $name }: { $verdict } - { $note }
import-check-file =
    { $count ->
        [one] { $count } textura
       *[other] { $count } texturas
    }: { $detail }
import-check-more =
    { $count ->
        [one] …e mais { $count } arquivo sinalizado.
       *[other] …e mais { $count } arquivos sinalizados.
    }
import-check-summary =
    Há decisões pendentes para { $target }: { $flagged } de { $total ->
        [one] { $total } arquivo
       *[other] { $total } arquivos
    } — { $incompatible ->
        [0] { $incompatible } texturas incompatíveis
        [one] { $incompatible } textura incompatível
       *[other] { $incompatible } texturas incompatíveis
    } e { $unknown } não verificadas.
import-check-note = As importações são gravadas como estão de qualquer forma - o formato só importa se o jogo precisar carregar essas texturas.
import-check-import-anyway = Importar mesmo assim
import-check-cancel = Cancelar importação

## Import folder dialog

dialog-folder-import-title = Importar pasta
folder-import-path = Pasta: { $path }
folder-import-summary =
    { $count ->
        [0] { $count } arquivos comuns
        [one] { $count } arquivo comum
       *[other] { $count } arquivos comuns
    } • { $size }
folder-import-top-level = Apenas os arquivos diretamente dentro desta pasta são incluídos; subpastas não são verificadas.
folder-import-no-duplicates = Nenhum nome duplicado detectado.
folder-import-duplicates =
    { $count ->
        [one] { $count } nome duplicado detectado. Escolha como lidar com ele.
       *[other] { $count } nomes duplicados detectados. Escolha como lidar com eles.
    }
folder-import-skipped =
    { $count ->
        [one] { $count } item não pôde ser inspecionado e será ignorado.
       *[other] { $count } itens não puderam ser inspecionados e serão ignorados.
    }
folder-import-files = Importar arquivos
folder-import-skip-duplicates = Importar (ignorar duplicados)
folder-import-replace-duplicates = Substituir duplicados

## Update check dialog

dialog-update-title = Verificação de atualizações
update-available = Atualização disponível: { $version }
update-latest = Você está usando a versão mais recente.
# $error is a technical detail and stays in English.
update-failed = Falha ao verificar atualizações: { $error }
update-dont-show = Não mostrar esta mensagem novamente
update-open-releases = Abrir versões

## Validate textures dialog

dialog-validator-title = Validar texturas
validator-intro = Escolha o jogo de destino deste arquivo. O validador sinaliza cada textura fora dos formatos aceitos pelo motor original e lista os itens desconhecidos que poderiam carregar, mas não são nativos do jogo.
validator-last-run = Última execução: { $counts }
validator-no-textures = nenhuma textura
validator-current-target = destino atual
validator-validate-for = Validar para { $game }
validator-native-heading = Nativos (verificados no original):
validator-unknown-heading = Desconhecidos / não nativos do jogo:
validator-hint-likely = O conteúdo parece ser de { $game }
validator-hint-possible = O conteúdo possivelmente é de { $game }
validator-hint-current = destino atual: { $game }
validator-use-as-target = Usar { $game } como destino
validator-already-target = (já é o destino)
validator-highlight-rows = Destacar linhas
legend-native = nativo
legend-native-description = Criado por este motor - nenhuma ação necessária.
legend-supported = compatível / conversível
legend-supported-description = Carrega, mas não é o dialeto de dados do jogo; uma conversão sem perdas pode ser oferecida.
legend-lossy = conversão com perdas
legend-lossy-description = Só pode ser usado após uma conversão que altera pixels (compressão ou quantização).
legend-unknown = desconhecido
legend-unknown-description = Sem evidências em nenhum sentido - não se sabe incompatível. Use com cuidado.
legend-incompatible = incompatível
legend-incompatible-description = O motor selecionado não consegue consumir este formato.

## Sort by dialog

dialog-sort-title = Ordenar por — { $archive }
dialog-sort-title-no-archive = Ordenar por — (nenhum arquivo aberto)
sort-empty = Nenhum critério ainda. Adicione um critério abaixo para começar a ordenar.
sort-intro = Defina regras de prioridade e aplique-as ao arquivo atual.
sort-select-key = Selecionar critério…
sort-add-key = + Adicionar critério
sort-add-key-max = + Adicionar critério (limite atingido)
sort-keys-active =
    { $active } de { $total ->
        [one] { $total } critério ativo
       *[other] { $total } critérios ativos
    }
sort-preview-heading = Pré-visualização ao vivo (10 primeiras entradas)
sort-preview-empty = (nenhuma entrada no arquivo atual)
sort-apply-preset = Aplicar predefinição…
sort-ascending-short = Asc ▲
sort-descending-short = Desc ▼
sort-key-name = Nome
sort-key-extension = Extensão
sort-key-type = Tipo
sort-key-size = Tamanho
sort-key-offset = Deslocamento
sort-key-ide-file = Arquivo IDE
sort-key-col-file = Arquivo COL
sort-preset-name-az = Nome (A→Z)
sort-preset-name-za = Nome (Z→A)
sort-preset-type-then-name = Tipo, depois nome
sort-preset-size-desc = Tamanho (maior → menor)
sort-preset-offset-asc = Deslocamento (menor → maior)

## Windows file dialogs (titles and file-type filters)

file-dialog-open-archive = Abrir arquivo IMG
file-dialog-filter-img = Arquivo IMG
file-dialog-compare = Comparar com lista de entradas
file-dialog-filter-entry-list = Lista de entradas IMG
file-dialog-open-agr = Abrir grupo de animação do Bully
file-dialog-filter-agr = Grupo de animação do Bully
file-dialog-import-files = Importar arquivos
file-dialog-filter-importable = Arquivos importáveis
file-dialog-import-folder = Selecionar pasta para importar
file-dialog-game-folder = Selecionar a pasta do jogo
file-dialog-choose-image = Escolher uma imagem
file-dialog-filter-images = Imagens
file-dialog-save-archive = Salvar arquivo IMG
file-dialog-export-list = Exportar lista de entradas
file-dialog-export-folder = Selecionar pasta de exportação

## Toasts: archives and selection

toast-no-archive-selected = Nenhum arquivo selecionado.
toast-archive-closed = O arquivo não está mais aberto.
toast-target-archive-closed = O arquivo de destino não está mais aberto.
toast-target-archive-deselected = O arquivo de destino não está mais selecionado.
toast-validated-archive-closed = O arquivo validado foi fechado.
toast-entry-unavailable = A entrada selecionada não está mais disponível.
toast-task-running = Outra tarefa ainda está em execução.
toast-operation-running = Uma operação de arquivo já está em execução.
toast-open-archive-to-validate = Abra um arquivo primeiro para validá-lo.
toast-open-archive-to-import = Abra um arquivo primeiro para importar para ele.
toast-open-archive-to-import-folder = Abra um arquivo primeiro para importar uma pasta.
toast-open-img-for-model = Abra um arquivo IMG primeiro; o modelo vem dele.
toast-already-open = Já está aberto: { $path }
# $error is a technical detail and stays in English.
toast-open-failed = Falha ao abrir o arquivo: { $error }
toast-file-gone = O arquivo não existe mais: { $path }
toast-file-not-found = Arquivo não encontrado: { $path }
toast-compare-empty-archive = O arquivo selecionado não tem entradas para comparar.

## Toasts: saving and packing

toast-archive-saved = Arquivo salvo.
toast-save-cancelled = Salvamento cancelado.
toast-save-failed = Falha ao salvar: { $error }
toast-save-before-pack = Salve o arquivo antes de compactá-lo.
toast-packed = Arquivo compactado — { $reclaimed } recuperados ({ $size } em disco).
toast-packed-nothing = Arquivo compactado — nenhum espaço recuperado ({ $size } em disco).
toast-pack-failed = Falha ao compactar: { $error }
toast-headers-repaired =
    { $count ->
        [one] { $count } cabeçalho de textura corrigido; salvando.
       *[other] { $count } cabeçalhos de textura corrigidos; salvando.
    }

## Toasts: game folder

toast-game-folder-set = Pasta do jogo para { $archive }: { $path }
toast-game-folder-automatic = Pasta do jogo para { $archive }: { $path } (automática)
toast-game-folder-none = { $archive } não tem pasta do jogo.
toast-game-folder-needs-save = Salve o arquivo primeiro; a pasta do jogo é armazenada por arquivo.

## Toasts: 3D viewer and animation

toast-select-model = Selecione primeiro uma entrada NIF, DFF ou COL.
toast-viewer-unsupported = O visualizador 3D integrado é compatível com .nif, .dff e .col ({ $entry }).
toast-select-ifp = Selecione primeiro uma entrada .ifp.
toast-no-model-for-agr = Nenhum modelo .nif correspondente encontrado para { $animation } neste arquivo.
toast-no-model-for-agr-open = Nenhum modelo .nif correspondente encontrado para { $animation } no arquivo aberto.
toast-loading-animation = Carregando { $animation } em { $model }…
toast-loading-animation-hxd = Carregando { $animation } em { $model }… (catálogo HXD encontrado)
toast-playback-stalled = Reprodução pausada após uma longa interrupção.
toast-demo-loaded = Demonstração de animação sintética carregada (sem dados do jogo).
toast-demo-closed = Demonstração de animação fechada.
toast-animation-ready = Animação pronta: { $summary }
toast-animation-failed = Falha ao carregar a animação: { $error }
toast-3d-load-failed = Falha ao carregar em 3D: { $error }
anim-summary-ifp =
    { $animation } em { $model } ({ $clips ->
        [one] { $clips } clipe
       *[other] { $clips } clipes
    }, GTA IFP)
anim-summary-agr =
    { $animation } em { $model } ({ $clips ->
        [one] { $clips } clipe
       *[other] { $clips } clipes
    })
anim-summary-agr-named =
    { $animation } em { $model } ({ $clips ->
        [one] { $clips } clipe
       *[other] { $clips } clipes
    }, nomes do HXD)

## Toasts: textures and conversion

toast-select-texture = Selecione primeiro uma entrada de textura.
toast-no-target = Nenhum destino definido para este arquivo. Escolha o jogo dele em Validar texturas.
toast-target-error = { $error } Escolha o jogo deste arquivo em Validar texturas.
toast-replace-busy = Uma substituição já está sendo preparada.
toast-preparing-replacement = Preparando substituição…
toast-replace-failed = Falha ao substituir: { $error }
toast-import-busy = Uma importação já está sendo preparada.
toast-preparing-import = Preparando importação…
toast-txd-name-required = Dê um nome ao novo TXD.
toast-entry-exists = Já existe uma entrada chamada “{ $name }”.
toast-entry-added = “{ $name }” adicionada - salve o arquivo para gravar.
toast-set-target-first = Defina primeiro um jogo de destino (Validar texturas).
toast-select-entries-to-convert = Selecione primeiro as entradas a converter.
toast-archive-changed-planning = O arquivo mudou durante o planejamento da conversão; tente novamente.
toast-convert-only-txd = Apenas entradas TXD podem ser convertidas - nenhuma das entradas selecionadas é um contêiner de texturas TXD.
toast-convert-all-native = Todas as texturas selecionadas já são nativas para o destino.
toast-archive-changed-after-plan = O arquivo mudou depois que esta conversão foi planejada; tente novamente.
toast-archive-changed-during = O arquivo mudou durante a conversão; resultados obsoletos foram descartados.
toast-converted =
    { $count ->
        [one] { $count } entrada convertida - salve o arquivo para gravá-la.
       *[other] { $count } entradas convertidas - salve o arquivo para gravá-las.
    }
toast-conversion-failed = Falha na conversão: { $error }
toast-no-decoded-textures = Nenhuma textura decodificada para exportar.
toast-decoded =
    { $count ->
        [one] { $count } textura decodificada
       *[other] { $count } texturas decodificadas
    }
toast-decoded-not-retained =
    { $count ->
        [one] { $count } textura decodificada, mas a pré-visualização não pôde ser mantida
       *[other] { $count } texturas decodificadas, mas a pré-visualização não pôde ser mantida
    }
toast-pick-embedded-folder = Escolha uma pasta para exportar as texturas incorporadas de { $model }
toast-basename-unknown = Não foi possível determinar o nome base de { $entry }
toast-read-failed = Falha ao ler { $name }: { $error }

## Toasts: import and export

toast-import-cancelled = Importação cancelada.
toast-imported =
    { $count ->
        [one] { $count } arquivo importado.
       *[other] { $count } arquivos importados.
    }
toast-imported-unchecked =
    { $count ->
        [one] { $count } arquivo importado - nenhum destino de validação definido, os formatos não foram verificados.
       *[other] { $count } arquivos importados - nenhum destino de validação definido, os formatos não foram verificados.
    }
toast-import-failed = Falha ao importar: { $error }
toast-no-files-in-folder = Nenhum arquivo comum encontrado em { $folder }.
toast-folder-scan-failed = Falha ao verificar a pasta: { $error }
toast-folder-import-failed = Falha ao importar a pasta: { $error }
toast-folder-import-done = Importação da pasta concluída: { $imported } importados, { $skipped } ignorados, { $failed } com falha.
toast-folder-import-cancelled = Importação da pasta cancelada: { $imported } importados, { $skipped } ignorados, { $failed } com falha.
toast-see-log = Consulte o log do arquivo para mais detalhes.
toast-exported =
    { $count ->
        [one] { $count } entrada exportada.
       *[other] { $count } entradas exportadas.
    }
toast-export-failed = Falha ao exportar: { $error }
toast-entry-list-exported =
    { $count ->
        [one] { $count } nome de entrada exportado para { $path }.
       *[other] { $count } nomes de entrada exportados para { $path }.
    }
toast-entry-list-export-failed = Falha ao exportar a lista de entradas: { $error }
toast-compare-failed = Falha na comparação da lista de entradas: { $error }
toast-no-missing-to-copy = Não há entradas ausentes para copiar.
toast-copied-missing =
    { $count ->
        [one] { $count } nome de entrada ausente copiado.
       *[other] { $count } nomes de entradas ausentes copiados.
    }

## Toasts: clipboard, dragging and other actions

toast-copied-entry-details = Detalhes da entrada selecionada copiados.
toast-copied-logs = Log copiado.
toast-copied-name = Nome copiado: { $name }
toast-drag-cancelled = Arrasto cancelado.
toast-moved-entries =
    { $count ->
        [one] { $count } entrada movida para o arquivo #{ $archive }.
       *[other] { $count } entradas movidas para o arquivo #{ $archive }.
    }
toast-autoscroll = Rolagem automática ativa: mova o ponteiro para rolar. Clique, clique com o botão do meio, clique com o botão direito, use a roda ou pressione uma tecla para parar.
toast-association-added = O IMG Editor Plus agora aparece em “Abrir com” do Explorador para .img/.dir. Para torná-lo o padrão, selecione-o na página de Configurações que foi aberta.
toast-association-removed = A associação de .img/.dir foi removida.
toast-association-failed = Falha na associação de arquivos: { $error }
toast-validation-cancelled = Validação cancelada.
toast-validation-failed = Falha na validação: { $error }
toast-no-compat-issues = Nenhum problema de compatibilidade encontrado - { $summary }
# $verdicts is the per-verdict count list, e.g. "native 12, untested 1".
validation-summary =
    Validado { $txds ->
        [0] { $txds } TXDs
        [one] { $txds } TXD
       *[other] { $txds } TXDs
    } ({ $textures ->
        [0] { $textures } texturas
        [one] { $textures } textura
       *[other] { $textures } texturas
    }) para { $game }: { $verdicts }; { $errors ->
        [0] { $errors } erros
        [one] { $errors } erro
       *[other] { $errors } erros
    }, { $warnings ->
        [0] { $warnings } avisos
        [one] { $warnings } aviso
       *[other] { $warnings } avisos
    }

## Archive log (the Logs box in the Export tab)

log-viewer-ready = Visualizador 3D integrado pronto
log-viewer-ready-cached = Visualizador 3D integrado pronto (em cache)
log-viewer-opened = Visualizador 3D aberto: { $name }
log-viewer-failed = Falha no visualizador 3D: { $reason }
log-viewer-closed = Visualizador 3D fechado
log-external-viewer = Abrindo visualizador 3D externo para { $name }
log-exported =
    { $count ->
        [one] { $count } entrada exportada
       *[other] { $count } entradas exportadas
    }
log-export-failed = Falha ao exportar: { $error }
log-entry-list-exported =
    { $count ->
        [one] Lista de entradas exportada ({ $count } nome) para { $path }
       *[other] Lista de entradas exportada ({ $count } nomes) para { $path }
    }
log-compat-check = Verificação de compatibilidade: { $summary }
log-decoded =
    { $count ->
        [one] { $count } pré-visualização de textura decodificada
       *[other] { $count } pré-visualizações de textura decodificadas
    }
log-texture-export-failed = Falha ao exportar { $model }: { $error }
# "Exported <what>" in the Export tab's recent list.
recent-exported = Exportação: { $what }
recent-exported-files =
    { $count ->
        [one] { $count } arquivo
       *[other] { $count } arquivos
    }

## Empty workspace pro tips (they name menus and keys; keep those in step
## with the translated menu labels)

pro-tip-label = Dica:
pro-tip-search = Pressione Ctrl+F para focar a Pesquisa, depois use ↑/↓ e Enter para ir até um resultado.
pro-tip-search-context = As sugestões de pesquisa revelam um resultado no contexto do arquivo; Exibir → Contexto da seleção na pesquisa ativa resultados isolados.
pro-tip-context-menu = Clique com o botão direito em uma entrada para ver em 3D, texturas, exportar, renomear e outras ações.
pro-tip-autoscroll = Clique com o botão do meio na lista de entradas para rolagem automática estilo navegador; a inércia opcional fica em Exibir.
pro-tip-tab-keys = Pressione 1, 2 ou 3 para alternar entre Exportar, Vista 3D e Textura.
pro-tip-close-tab = Clique com o botão do meio em uma aba de arquivo para fechá-la rapidamente.
pro-tip-uv-overlay = As sobreposições de UV da textura ficam disponíveis quando o modelo selecionado fornece geometria correspondente.
pro-tip-wire-grid = Na Vista 3D, a sobreposição Arestas expõe as bordas dos triângulos e o Piso em grade ajuda a avaliar a escala.
pro-tip-save-keys = Use Ctrl+S para salvar rapidamente e Ctrl+Shift+S para salvar o arquivo com um novo nome.
pro-tip-unique-exports = As texturas exportadas usam nomes de arquivo únicos automaticamente, então exportações em lote nunca se sobrescrevem.
pro-tip-agr = Editar → Carregar arquivo de animação .agr reproduz a animação na Vista 3D; Espaço alterna a reprodução e ←/→ percorre os quadros.
pro-tip-model-picker = Enquanto uma animação é reproduzida, o seletor Modelo do painel a reproduz em qualquer modelo compatível do arquivo.
pro-tip-fullscreen-preview = O ícone de expandir na pré-visualização de importação ou substituição a abre em tela cheia: role para ampliar, arraste para mover e Esc para fechar.
pro-tip-bulk-convert = Converter seleção para o dialeto do destino recodifica em lote os TXDs selecionados — escolha o jogo em Validar texturas primeiro.
pro-tip-entry-lists = Ctrl+L exporta uma lista de entradas e Ctrl+P compara uma com o arquivo para encontrar nomes ausentes.
toast-no-dff-to-animate = Nenhum modelo DFF encontrado neste arquivo para animar.
toast-replace-needs-txd = A substituição funciona em entradas TXD; essa entrada não é uma.
toast-texture-replaced = Textura substituída - salve o arquivo para gravá-la.
toast-bully-texture-writing = A gravação de texturas do Bully (Gamebryo) ainda não é compatível.
toast-archive-file-missing = O arquivo não existe mais. Use Salvar como… primeiro.
toast-folder-scan-target-changed = O arquivo de destino mudou enquanto a pasta era verificada.
toast-folder-import-discarded = A importação da pasta foi descartada porque o arquivo de destino mudou.
toast-compare-discarded = A comparação foi descartada porque o arquivo mudou; tente novamente.
toast-drop-needs-archive = Abra um arquivo primeiro para soltar arquivos que não sejam IMG nele.
toast-no-game-root = Não foi possível determinar a pasta do jogo a partir do caminho do arquivo.
toast-textures-exported =
    { $count ->
        [one] { $count } textura exportada.
       *[other] { $count } texturas exportadas.
    }
toast-textures-export-failed = Falha ao exportar texturas: { $error }
error-write-file = Falha ao gravar { $path }: { $error }
error-read-entry = Falha ao ler a entrada: { $error }
error-texture-decode = Falha ao decodificar a textura: { $error }
error-texture-preview-unsupported = A pré-visualização de texturas é compatível com entradas TXD e NFT; “{ $entry }” não é um contêiner de texturas compatível.

## Entry table and search

table-name = Nome
table-size = Tamanho
# Size column for entries still read from the archive (sectors × 2 KB).
table-size-kb = { $size } KB
table-no-matches = Nenhuma entrada corresponde ao filtro atual.
search-label = Pesquisar:
search-did-you-mean = Você quis dizer:
sort-tip-name-asc = Ordenado por nome de arquivo (A → Z).
sort-tip-name-desc = Ordenado por nome de arquivo (Z → A).
sort-tip-name-inactive = Ordenar por nome de arquivo (A → Z).
sort-tip-type-primary = Ordenado por tipo de arquivo, { $type } primeiro.
sort-tip-type-alphabetical = Ordenado por tipo de arquivo em ordem alfabética.
sort-tip-type-inactive = Ordenar por tipo de arquivo (ordem alfabética).
sort-tip-size-desc = Ordenado por tamanho (maiores primeiro).
sort-tip-size-asc = Ordenado por tamanho (menores primeiro).
sort-tip-size-inactive = Ordenar por tamanho (maiores primeiro).
version-unknown = Desconhecido

## Toolbar tooltips

toolbar-new = Novo
toolbar-open = Abrir
toolbar-save = Salvar
toolbar-pack = Compactar arquivo
toolbar-import = Importar
toolbar-import-folder = Importar pasta
toolbar-export-selected = Exportar selecionados
toolbar-delete-selected = Excluir selecionados
toolbar-validate = Validar texturas
toolbar-image-as-txd = Importar imagem como TXD
toolbar-convert-selection = Converter seleção para o dialeto do destino

## Empty workspace and status bar

empty-heading = Abra ou crie um arquivo para começar.
empty-drop-hint = Ou arraste e solte um arquivo .img ou .dir aqui para abri-lo.
status-selected = Selecionadas: { $count }

## Inspector tabs

tab-export = Exportar
tab-3d-view = Vista 3D
tab-texture = Textura

## Export tab

export-format = Formato
export-entries = Entradas
export-entries-value = { $total } (visíveis: { $visible })
export-game-folder = Pasta do jogo
export-game-folder-automatic = { $path } (automática)
export-game-folder-none = nenhuma
export-game-folder-unsaved = nenhuma (arquivo não salvo)
export-progress = Progresso
export-ready = Pronto para exportar
export-open-folder = Abrir pasta de exportação
export-selected-entry = Entrada selecionada:
export-logs = Logs:
export-recent = Exportações recentes:
button-copy = Copiar

## Entry details (Export tab, and the Copy button's clipboard text)

inspect-name = Nome
inspect-type = Tipo
inspect-size = Tamanho
inspect-offset = Deslocamento
inspect-source = Origem
inspect-size-mb = { $mb } MB ({ $bytes } bytes, { $sectors } setores)
inspect-size-kb = { $kb } KB ({ $bytes } bytes, { $sectors } setores)
inspect-size-bytes = { $bytes } bytes ({ $sectors } setores)
inspect-offset-value = setor { $sector } (byte { $byte })
inspect-hex-preview = Pré-visualização (hex):

## 3D view tab

viewer-no-archive = Nenhum arquivo aberto.
viewer-try-demo = Experimente a demonstração de animação sintética
viewer-select-model = Selecione uma entrada .nif, .dff ou .col para visualizá-la em 3D.
viewer-gpu-unavailable = Visualizador de GPU indisponível
viewer-gpu-hint = Tente limpar a pré-visualização ou selecionar um modelo menor.
viewer-clear-error = Limpar erro do visualizador
viewer-selected-model = modelo selecionado
viewer-preparing = Preparando a pré-visualização 3D
viewer-preparing-detail = Lendo a geometria e resolvendo as texturas…
viewer-preparing-cache-note = As próximas pré-visualizações deste modelo serão instantâneas.
viewer-ready = Pronto para visualizar este modelo em 3D.
viewer-unsupported-entry = O visualizador integrado renderiza entradas .nif, .dff e .col. { $entry } não é um modelo compatível — use o menu de clique direito para outro visualizador.
viewer-load-selected-hint = Use ‘{ viewer-load-selected }’ acima para visualizar este modelo.
viewer-right-click-hint = Selecione uma entrada .nif, .dff ou .col e clique com o botão direito → { context-open-3d }.
viewer-toolbar-label = 3D:
viewer-preparing-selected = Preparando o modelo selecionado…
viewer-load-selected = Carregar selecionado
viewer-load-selected-tip = Carregar o modelo selecionado no visualizador 3D.
viewer-reset = Redefinir vista
viewer-reset-tip = Reajustar a câmera ao modelo. Atalho: R
viewer-clear = Limpar
viewer-clear-tip = Descartar a cena carregada
viewer-wireframe = Arestas
viewer-wireframe-tip = Mostrar as arestas dos triângulos sobre o modelo sombreado.
viewer-cull = Ocultar faces traseiras
viewer-cull-tip = Ocultar triângulos voltados para trás para inspecionar a orientação da superfície.
viewer-textured = Com texturas
viewer-textured-tip = Usar as texturas decodificadas do modelo em vez de um material neutro.
viewer-alpha = Mistura alfa
viewer-alpha-tip = Respeitar o alfa das texturas para recortes e materiais transparentes.
viewer-alpha-unavailable-tip = Ative { viewer-textured } em um modelo com texturas para usar a mistura alfa.
viewer-center = Centralizar origem
viewer-center-tip = Recentralizar o modelo para inspeção; desative para preservar as coordenadas do mundo.
viewer-grid = Piso em grade
viewer-grid-tip = Mostrar a grade de referência do mundo e os eixos XYZ.
viewer-stats =
    { $vertices ->
        [0] { $vertices } vértices
        [one] { $vertices } vértice
       *[other] { $vertices } vértices
    }   { $triangles ->
        [0] { $triangles } triângulos
        [one] { $triangles } triângulo
       *[other] { $triangles } triângulos
    }   { $textures ->
        [0] { $textures } texturas
        [one] { $textures } textura
       *[other] { $textures } texturas
    }   { $width }×{ $height }   { $orientation }   { $origin }
viewer-origin-centered = centralizada
viewer-origin-world = mundo
viewer-preparing-entry = Preparando { $entry }…
viewer-no-scene = Nenhuma cena carregada

## Animation dock

anim-preparing = Preparando animação
anim-preparing-detail = Decodificando clipes e resolvendo texturas…
anim-preparing-cache-note = As próximas reproduções deste par serão instantâneas.
anim-preparing-label = Preparando { $label }…
anim-title = Animação
anim-title-demo = Demonstração de animação
anim-demo-note = dados sintéticos — sem dados do jogo
anim-exit-demo = Sair da demonstração
anim-loop = Repetir a reprodução
anim-speed = Velocidade
anim-pack = Pacote de animação
anim-model-tip = Reproduzir esta animação em outro modelo
anim-clip = Clipe
anim-play = Reproduzir (Espaço)
anim-pause = Pausar (Espaço)
anim-jump-start = Ir para o início (Home)
anim-step-back = Voltar um quadro (←)
anim-step-forward = Avançar um quadro (→)
anim-jump-end = Ir para o fim (End)
anim-stop = Parar
anim-rate-source = fonte de { $fps } fps
anim-rate-preview = pré-visualização de { $fps } fps
anim-frame = Enquadramento
anim-frame-rest = Repouso
anim-frame-rest-tip = Enquadrar a pose de repouso
anim-frame-pose = Pose
anim-frame-pose-tip = Enquadrar a pose atual
anim-frame-motion = Movimento
anim-frame-motion-tip = Enquadrar o movimento completo
anim-in-place-tip = Raiz no lugar — descartar a translação da raiz
anim-follow-tip = Acompanhar o movimento da raiz com a câmera
anim-skeleton-tip = Mostrar a sobreposição do esqueleto
anim-motion-path-tip = Mostrar o caminho do movimento da raiz
anim-ground-tip = Fixar o ponto mais baixo do clipe no chão
anim-crossfade-tip = Fazer transição suave ao trocar de clipe
anim-keys-hint = Espaço reproduzir/pausar · ←/→ quadro a quadro · Home/End extremos · arraste para percorrer

## Texture tab

texture-no-archive = Nenhum arquivo aberto.
texture-select-entry = Selecione uma entrada TXD, NFT, NIF ou DFF para visualizar as texturas.
texture-not-container = { $entry } não é um contêiner de texturas. A pré-visualização está disponível para entradas TXD, NFT ou modelos renderizados.
texture-no-companions = Nenhuma textura associada foi resolvida para { $entry }.
texture-load-model-hint = Carregue o modelo selecionado para resolver as texturas dele.
texture-load-model = Carregar modelo selecionado
texture-not-decoded = { $kind } { $entry } ainda não foi decodificado.
texture-load-textures = Carregar texturas do { $kind } selecionado
texture-none-decodable = Nenhuma textura decodificável neste contêiner.
texture-animation-model = Modelo de animação { $entry } — troque de modelo no painel 3D
texture-export =
    { $count ->
        [one] Exportar textura ({ $count })
       *[other] Exportar texturas ({ $count })
    }
texture-slot = Textura { $index }/{ $count }
texture-alpha = Alfa:
texture-yes = Sim
texture-no = Não
texture-verdict-default = Veredito para um destino { $game }.
texture-pal8-ready = Pronta para PAL8 ({ $colors } cores)
texture-pal8-tip = Cada pixel é uma dessas cores distintas, então uma paleta de 8 bits armazena esta textura sem quantização.
texture-replace = Substituir textura…
texture-replace-hint = Importa PNG/DDS/BMP/TGA e recodifica para o destino do arquivo.
texture-uv-standalone-tip = Pré-visualização apenas da textura. Selecione um modelo DFF ou NIF correspondente para ativar o mapeamento UV.
texture-uv-unavailable-tip = O mapeamento UV fica disponível após carregar uma geometria de modelo correspondente.
texture-uv-tip = Mostrar os triângulos de UV da geometria do modelo correspondente.
texture-uv = Mostrar mapa UV
texture-uv-standalone = Pré-visualização apenas da textura · o mapa UV precisa de geometria correspondente
texture-uv-unavailable = Carregue a geometria do modelo correspondente para ativar as UVs
texture-uv-triangles =
    { $count ->
        [0] { $count } triângulos
        [one] { $count } triângulo
       *[other] { $count } triângulos
    }
texture-only-badge = Pré-visualização apenas da textura
texture-grid = Grade
texture-grid-tip = Mostrar uma grade de referência sobre a pré-visualização da textura.
texture-grid-size-tip = Usar uma grade de referência de { $size }×{ $size } para a textura.
texture-grid-size = Tamanho:
texture-fullscreen-tip = Ver em tela cheia (qualidade máxima)
texture-model-companion = Textura do modelo

## Entry table: the Type column and curated file-type names
## (the English names double as sort and grouping keys; only the
## displayed text is translated)

table-type = Tipo
# $arrow is ↑ or ↓; $primary is the file type sorted first.
table-type-sorted = Tipo { $arrow } { $primary }
file-type-model = Modelo
file-type-texture = Textura
file-type-collision = Colisão
file-type-animation = Animação
file-type-placement = Posicionamento
file-type-definition = Definição
file-type-data = Dados
archive-untitled = Sem título

## Archive log (Export tab)

log-archive-created = Arquivo criado
log-archive-saved = Arquivo salvo
log-archive-packed =
    { $entries ->
        [one] Arquivo compactado: { $entries } entrada, { $reclaimed } recuperados
       *[other] Arquivo compactado: { $entries } entradas, { $reclaimed } recuperados
    }
log-imported-entries =
    { $count ->
        [one] { $count } entrada importada
       *[other] { $count } entradas importadas
    }
log-folder-import = Importação de pasta: { $imported } importados, { $skipped } ignorados, { $failed } com falha
log-folder-import-detail = Detalhes da importação de pasta: { $detail }
folder-import-cancelled-detail = Importação cancelada; os arquivos restantes foram ignorados.
folder-import-duplicate = { $name }: duplicado ignorado

## Entry details: source and format summary

inspect-source-imported = Importada
inspect-source-imported-from = Importada de { $path }
inspect-source-archive = Arquivo { $archive }, setor { $sector }
inspect-key-format = Formato
inspect-key-version = Versão
inspect-key-clump-size = Tamanho do clump
inspect-key-endian = Endian
inspect-key-user-version = Versão do usuário
inspect-key-lines = Linhas
inspect-key-atomics = Atômicos
inspect-key-textures = Texturas
inspect-key-entries = Entradas
inspect-key-mesh-vertices = Vértices da malha
inspect-key-mesh-faces = Faces da malha
inspect-key-spheres = Esferas
inspect-key-boxes = Caixas
inspect-key-shadow-mesh = Malha de sombra
inspect-rw-truncated = RenderWare (truncado)
inspect-rw-clump = Clump (modelo)
inspect-rw-txd = Dicionário de texturas
inspect-rw-pi-txd = Dicionário de texturas independente de plataforma
inspect-rw-animation = Animação
inspect-rw-uv-animation = Animação de UV
inspect-rw-stream = Fluxo RenderWare
inspect-bytes =
    { $count ->
        [one] { $count } byte
       *[other] { $count } bytes
    }
inspect-col-unknown = Colisão desconhecida
inspect-endian-big = Big
inspect-endian-little = Little
inspect-nif-truncated = NIF (truncado)
inspect-lines-value = { $lines } ({ $nonempty } não vazias)
inspect-format-scm = Script do GTA (main.scm)
inspect-format-ipl = Posicionamento de itens do GTA
inspect-format-ide = Definição de itens do GTA
inspect-texture-count =
    { $count ->
        [0] { $count } texturas
        [one] { $count } textura
       *[other] { $count } texturas
    }

## Embedded-texture export (NIF → NFT)

texture-export-none = Nenhuma textura incorporada encontrada
texture-export-done =
    { $count ->
        [one] { $count } textura incorporada exportada
       *[other] { $count } texturas incorporadas exportadas
    }
texture-export-partial = Texturas exportadas: { $written }, falhas: { $failures }
texture-export-no-nft = Nenhum NFT encontrado para “{ $name }”

## Manifest comparison errors

compare-manifest-inspect = Não foi possível inspecionar o manifesto “{ $path }”: { $error }
compare-manifest-too-large = O manifesto “{ $path }” é grande demais ({ $size }; o limite é { $limit }).
compare-manifest-read = Não foi possível ler o manifesto “{ $path }”: { $error }
compare-manifest-grew = O manifesto “{ $path }” ultrapassou o limite de { $limit } durante a leitura.
compare-manifest-utf8 = O manifesto “{ $path }” não é UTF-8 válido: { $error }

## Compatibility verdicts (lowercase: they appear mid-sentence)

verdict-native = nativo
verdict-supported = compatível
verdict-convertible-lossless = conversível (sem perdas)
verdict-convertible-lossy = conversível (com perdas)
verdict-unsupported = incompatível
verdict-untested = não verificado

## Compatibility evidence notes
## These cite measurements of the retail games. Keep format names
## (DXT1, PAL8, X8R8G8B8, D3D8, pp=1 …) and file names untranslated.
## "Raster" is one texture image inside a TXD; "dialect" is the set of
## texture formats a game's own files use.

compat-class-other-nif = outros formatos NiPixelData
compat-cat-iii-pal = 96,5% das texturas do mundo original
compat-cat-iii-888 = 6.806 rasters, incl. conjunto de jogador/veículos
compat-cat-iii-8888 = 1.121 rasters
compat-cat-iii-1555 = 24 rasters
compat-cat-iii-dxt1 = o original não inclui nenhum; o hardware D3D8 é compatível
compat-cat-iii-dxt = valores de compressão D3D8 1-5; o original não inclui nenhum
compat-cat-depth24 = formato documentado de profundidade 24; stride/ordem não verificados
compat-cat-iii-565 = mapeado pelo driver; o III não inclui nenhum
compat-cat-555-lum8 = o driver mapeia C555 e LUM8; o original não inclui nenhum
compat-cat-a8l8 = D3D9/decodificadores são compatíveis; caminho nibble do RW não verificado
compat-cat-vc-dxt1 = dialeto do mundo original; D3D8 pp=1 (mais de 10 mil rasters, nibbles obsoletos)
compat-cat-vc-dxt3 = dialeto alfa original; D3D8 pp=3 (1.149 rasters)
compat-cat-vc-pal = 27 rasters; aceito, mas raro
compat-cat-vc-888 = 1 raster
compat-cat-vc-8888 = o III inclui; o próprio VC não inclui nenhum
compat-cat-vc-565 = os rótulos 565 do original são dados DXT1; nenhum R565 real medido
compat-cat-vc-16bit = os rótulos do original são dados DXT1/DXT3; formas cruas de 16 bits não medidas
compat-cat-vc-dxt = existem valores de compressão D3D8; o original inclui apenas 1 e 3
compat-cat-sa-dxt1 = 28.807 rasters nos quatro arquivos
compat-cat-sa-dxt3 = 2.098 rasters
compat-cat-sa-888 = 1.015 rasters, principalmente skins do player.img
compat-cat-sa-8888 = 237 rasters
compat-cat-sa-dxt5 = o original não inclui nenhum; o D3D9 é compatível
compat-cat-sa-dxt24 = a palavra de formato D3D9 os carrega; alfa pré-multiplicado
compat-cat-sa-pal = as fontes divergem; o original não inclui nenhum; analisar e preservar
compat-cat-sa-16bit = mapeado pelo driver; o original não inclui 16 bits descomprimidos
compat-cat-sa-a8l8 = o Magic.TXD o lista para SA PC; caminho RW não verificado
compat-cat-bully-dxt1 = 31.714 rasters
compat-cat-bully-dxt5 = 3.526 rasters
compat-cat-bully-rgb = 138 / 134 rasters
compat-cat-bully-pal = 127 / 1 rasters
compat-cat-bully-dxt3 = o Gamebryo é compatível; o original não inclui nenhum
compat-cat-bully-other = 15 rasters indecifráveis

compat-note-bully-not-rw = os recursos do Bully são Gamebryo NIF/NFT, não nativos do RenderWare
compat-note-platform-rewrite = um raster platform-{ $platform } em um arquivo de { $game } precisa de uma reescrita de plataforma/versão
compat-note-no-profile = ainda não há tabela de perfil
compat-note-nft-not-rw = os rasters NFT do Gamebryo não são nativos do RenderWare
compat-note-bully-dxt1 = Bully original: 31.714 rasters DXT1
compat-note-bully-dxt5 = Bully original: 3.526 rasters DXT5
compat-note-bully-rgb = o Bully original inclui RGB/RGBA cru (138/134)
compat-note-bully-pal = o Bully original inclui rasters com paleta (127 PAL + 1 PALA)
compat-note-bully-dxt3 = o Gamebryo é compatível com DXT3, mas o Bully original não inclui nenhum
compat-note-sa-dxt = SA original: 28.807 DXT1 + 2.098 DXT3 rasters
compat-note-sa-dxt24 = o formato nativo D3D9 carrega DXT2/DXT4 (pré-multiplicados); o SA original não inclui nenhum
compat-note-sa-dxt5 = o SA original não inclui nenhum; o DXT5 conta com o suporte do D3D9 (as ferramentas de mod usam)
compat-note-sa-pal = as fontes divergem sobre as paletas do SA; o SA original não inclui nenhum; analisar e preservar
compat-note-sa-depth24 = formato documentado R8G8B8 de profundidade 24; o SA original não inclui nenhum; execução não verificada
compat-note-sa-16bit = o driver mapeia 1555/565/4444; o SA original não inclui 16 bits descomprimidos
compat-note-c555 = o driver mapeia C555 para X1R5G5B5; o original não inclui nenhum
compat-note-lum8 = o driver mapeia LUM8 para D3DFMT_L8; o original não inclui nenhum
compat-note-sa-a8l8 = o D3D9 carrega A8L8 e decodificadores independentes são compatíveis; o mapeamento nibble comum do RW não foi verificado
compat-note-iii-pal = III original: 96,5% PAL8; VC original: 27 rasters - aceito, mas raro
compat-note-vc-dxt1 = dialeto do mundo do VC original: DXT1 com D3D8 pp=1; o nibble do raster está obsoleto
compat-note-iii-dxt1 = o III original não inclui rasters comprimidos (0/15.372); o hardware D3D8 é compatível com DXT1
compat-note-vc-dxt3 = dialeto alfa do VC original: DXT3 com D3D8 pp=3 (1.149 rasters)
compat-note-iii-dxt = os valores de compressão D3D8 1-5 correspondem a DXT1-5 (DXT2/4 pré-multiplicados); o III+VC originais incluem apenas 1 e 3
compat-note-iii-depth24 = formato documentado R8G8B8 de profundidade 24; stride/ordem/execução não verificados
compat-note-iii-8888 = o III original inclui 8888 nos dois arquivos (1.121 rasters)
compat-note-vc-565 = os rasters rotulados como 565 no VC original são dados DXT1; nenhum R565 real medido
compat-note-iii-565 = forma de 16 bits mapeada pelo driver; o III não inclui nenhum
compat-note-vc-1555 = os rótulos 1555 do VC original são dados DXT1; nenhum R1555 real medido
compat-note-vc-4444 = os rótulos 4444 do VC original são dados DXT3; nenhum R4444 real medido
compat-note-iii-4444 = o III não inclui nenhum; 16 bits com alfa da era D3D8
compat-note-iii-a8l8 = o Magic.TXD lista A8L8 para SA PC; o D3D9 o carrega; o caminho RW comum não foi verificado

## Output-format choices (import and replace dialogs)

compat-choice-iii-888 = X8R8G8B8 de 32 bits, sem perdas; padrão do txd.img do III original
compat-choice-iii-8888 = A8R8G8B8, mantém o alfa; o III original inclui 1.121
compat-choice-iii-pal8 = paleta de 8 bits, quantiza as cores; dialeto do mundo original (96,5%)
compat-choice-pal4 = paleta de 4 bits, quantiza bastante; apenas arte de 16 cores
compat-choice-iii-1555 = 16 bits com alfa de 1 bit; o original inclui 24
compat-choice-iii-dxt = compatível com hardware, mas não usado no III; com perdas
compat-choice-vc-dxt1 = dialeto do mundo do VC original (D3D8 pp=1); compressão com perdas
compat-choice-vc-dxt3 = dialeto alfa do VC original (D3D8 pp=3); compressão com perdas
compat-choice-vc-888 = X8R8G8B8 de 32 bits, sem perdas; o VC original inclui um
compat-choice-vc-8888 = A8R8G8B8, mantém o alfa; forma da era D3D8
compat-choice-vc-pal8 = paleta de 8 bits, quantiza as cores; o VC original inclui 27
compat-choice-vc-565 = 16 bits; o VC original rotula 565 como dados DXT, forma crua não medida
compat-choice-vc-4444 = 16 bits com alfa; o VC original rotula 4444 como dados DXT3
compat-choice-player-888 = X8R8G8B8 de 32 bits; o player.img original inclui 269
compat-choice-player-8888 = A8R8G8B8, mantém o alfa; o player.img original inclui 125
compat-choice-player-dxt = compatível em toda parte; com perdas (o player.img não inclui nenhum)
compat-choice-sa-dxt1 = dialeto do mundo do SA original; compressão com perdas
compat-choice-sa-dxt3 = dialeto alfa do SA original; compressão com perdas
compat-choice-sa-8888 = A8R8G8B8, sem perdas; o SA original o inclui no player.img
compat-choice-sa-pal8 = quantiza as cores; o SA original não inclui nenhum raster com paleta

## Conversion warnings and errors

compat-warn-alpha-discarded = { $source } tem alfa, mas { $format } não consegue armazená-lo - o canal alfa será descartado.
compat-warn-dxt-lossy = a compressão { $format } tem perdas; a pré-visualização mostra o resultado codificado.
compat-warn-palette-exact =
    { $colors ->
        [one] { $format } armazena a imagem exatamente ({ $colors } cor).
       *[other] { $format } armazena a imagem exatamente ({ $colors } cores).
    }
compat-warn-palette-quantize = As cores serão quantizadas para no máximo { $cap } entradas.
compat-warn-stream-budget = { $width }x{ $height } ultrapassa o limite de streaming de 1024 px do SA; o jogo pode não carregá-la por streaming.
# $verdict is one of the verdict-* labels above.
compat-warn-verdict = { $format } é { $verdict } para { $game }: { $note }
compat-error-image-format = formato de imagem não reconhecido: { $error }
compat-error-image-kind = imagens { $kind } não são compatíveis; use PNG, DDS, BMP ou TGA
compat-error-decode = falha ao decodificar { $label }: { $error }
compat-error-image-size = tamanho de imagem não compatível { $width }x{ $height }
compat-error-unreadable-texture = “{ $name }” não pode ser decodificada ({ $error }); a conversão precisa de pixels legíveis
compat-error-no-dimensions = a textura não tem dimensões
compat-error-texture-index = o índice de textura { $index } está fora do intervalo
compat-error-unknown-target = destino de jogo desconhecido “{ $id }”

## Save check: container conventions

compat-container-v2-expects-v1 = contêiner IMG v2, mas { $game } espera IMG v1 - o jogo não verá esses arquivos.
compat-container-v1-sa = contêiner IMG v1; o San Andreas original usa IMG v2 (o v1 só carrega quando listado no gta.dat).
compat-container-xbox = empacotamento Xbox 360; { $game } espera um contêiner de PC.

## Import check notes

compat-scan-unreadable = não foi possível ler: { $error }
compat-scan-bad-nif = cabeçalho NIF ilegível: { $error }
compat-scan-empty-nif = nenhum bloco NiPixelData (stub vazio)
compat-scan-not-texture = não é uma textura TXD ou Gamebryo

## Target-game suggestion evidence ($share is a percentage such as "92%")

compat-hint-gamebryo = { $total } entradas Gamebryo ({ $nft } NFT, { $nif } NIF)
compat-hint-d3d9 = plataforma 9 (D3D9) em { $share } de { $total } rasters amostrados
compat-hint-pal = PAL8/PAL4 em { $share } dos rasters amostrados
compat-hint-vc16 = 16 bits 565/4444/1555 em { $share } dos rasters amostrados
compat-hint-d3d8 = plataforma 8 (D3D8)
