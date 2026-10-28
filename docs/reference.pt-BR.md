# Audio Pulse — referência técnica

> Documentação detalhada do motor, dos estágios de DSP, do perfil por foto e da
> solução de problemas. Para a visão geral do produto, volte ao
> [README em português](../README.pt-BR.md) ou ao [README in English](../README.md).

Aprimoramento de áudio em tempo real para **qualquer fone com fio ou Bluetooth** no Windows, macOS e Linux. O processamento acontece fora do player: o sistema inteiro passa pelo motor DSP do Audio Pulse, que aplica equalização, Bass Boost, dinâmica, compensação de loudness, crossfeed, áudio espacial e equalização adaptativa.

Construído com **Tauri 2 + Rust + React**. O áudio é processado inteiramente em Rust; o webview só desenha a interface e envia parâmetros.

---

## Recursos

- **Equalizador gráfico de 10 bandas** (31 Hz – 16 kHz, ±12 dB, Q 1,41) com curva de resposta em tempo real e nós arrastáveis.
- **Pré-amplificador** (−24 a +12 dB) e **AutoEQ paramétrico** com até 16 filtros PK/LS/HS.
- **Perfil por foto da orelha (estimativa local)**: um modelo de profundidade roda no dispositivo, estima a geometria da concha e do hélice e gera três filtros de 2–8 kHz aplicáveis ao AutoEQ. A foto nunca sai do computador.
- **Tone controls**: médios (peaking em 1,5 kHz) e agudos (high shelf em 6 kHz).
- **Bass Boost / subwoofer virtual** com intensidade, frequência de corte e síntese de harmônicos.
- **Compressor** e **limiter com look-ahead** de 5 ms para evitar distorção e clipping.
- **Compensação de loudness ISO 226** dependente do volume.
- **Áudio espacial**: modos Binaural, **HRTF dinâmico** (ITD/ILD acompanham a imagem estéreo) e Surround 7.1 virtual, com largura do palco e ambiência.
- **Equalização adaptativa**: análise de cinco bandas de oitava a cada 20 ms, com correções de até ±3 dB e proteção preditiva de pico.
- **Crossfeed** para fones abertos ou fechados, com atraso ajustável.
- **Presets**: 8 modos de uso + 12 perfis de fones, criação/exclusão/exportação/importação, AutoEQ.
- **Perfis por dispositivo**: aplica um preset automaticamente ao iniciar com uma saída específica.
- **Medidores reais**: espectro de 56 bandas, níveis L/R, redução de ganho, correção do EQ adaptativo em 5 bandas, CPU, latência e falhas (xruns).
- **Bypass instantâneo**, volume master, atalhos globais, bandeja do sistema e inicialização automática.
- **Volume master = volume do Windows**: o slider do app e o controle de volume do sistema são o mesmo controle, nos dois sentidos (Windows).
- **Bateria do headset sem fio** na barra superior, lida do receptor USB quando o modelo expõe o relatório HID de carga (Windows).
- Interface **dark/light**, **pt-BR e inglês**, responsiva, sem dependência de Web Audio para processar som.

---

## Stack

| Camada | Tecnologia |
| --- | --- |
| Shell desktop | Tauri 2 (Rust) |
| Interface | React 19, TypeScript, Zustand, CSS próprio |
| Build frontend | Vite 8 |
| Áudio | CPAL 0.15 (WASAPI/Core Audio/ALSA) |
| DSP | crate próprio `audio-pulse-dsp`, sem alocações no caminho de áudio |
| Comunicação | comandos Tauri + eventos; parâmetros lock-free com `arc-swap` |
| Buffer | `ringbuf` SPSC + reamostragem linear com correção de clock |
| Persistência | JSON com gravação atômica e debounce |
| Profundidade | ONNX Runtime (`ort`) + Depth Anything V2 Small int8, execução local na CPU |

---

## Arquitetura

```
┌─────────────────────────────┐        comandos/eventos Tauri
│ React (UI)                  │ ◄──────────────────────────────┐
│ Zustand + otimizações       │                                │
└─────────────────────────────┘                                │
                                                               │
┌──────────────────────────────────────────────────────────────┴──┐
│ AppState (Rust)                                                  │
│ parâmetros, presets, configurações, dispositivos, avisos        │
└───────────────┬─────────────────────────────────────────────────┘
                │ ArcSwap<ProcessingParams> (leitura sem lock)
┌───────────────▼─────────────────────────────────────────────────┐
│ Thread do motor (cpal)                                          │
│  entrada → ring buffer → reamostragem → Processor (audio-pulse-dsp) │
│  → saída (fone)                                                 │
└───────────────┬─────────────────────────────────────────────────┘
                │ telemetria a cada ~33 ms / dispositivos a cada 3 s
                ▼
            interface
```

- A thread do motor possui os streams CPAL durante toda a vida (streams não são `Send` em todas as plataformas). A UI nunca bloqueia o áudio.
- Os parâmetros são publicados em um `ArcSwap`; os callbacks só trocam o ponteiro e reconstroem coeficientes quando algum bloco muda.
- O fluxo contínuo não aloca: buffers e filtros são pré-alocados. Pequenas alocações só ocorrem quando um bloco de parâmetros muda.
- O espectro é calculado no DSP e enviado por telemetria; o canvas apenas desenha.

### Cadeia de processamento

```
preamp → AutoEQ → EQ 10 bandas → tone → Bass Boost → spatial
→ crossfeed → adaptive EQ → compressor → volume → loudness → limiter
```

O **bypass** retorna a amostra original antes de qualquer estágio (passthrough exato), mantendo apenas o volume master. Alterar o bypass também zera compressor e limiter para evitar resíduos.

Alterações **discretas** (aplicar preset, ligar/desligar bypass, trocar o modo espacial) trocam o bloco inteiro de coeficientes de uma vez. Para isso não estalar, o processador desce a saída em 2 ms, aplica os novos parâmetros em silêncio e sobe em 30 ms. Sliders e volume são contínuos e passam direto, sem fade.

---

## Estrutura do projeto

```
som/
├── crates/audio-pulse-dsp/      # DSP puro, testável, sem dependência de dispositivo
│   └── src/
│       ├── biquad.rs            # filtros RBJ (peaking, shelves, LP, HP)
│       ├── equalizer.rs         # EQ gráfico, paramétrico e tone
│       ├── bass.rs              # low shelf + síntese de harmônicos
│       ├── spatial.rs           # 8 canais virtuais, ITD, sombra, reverb
│       ├── crossfeed.rs         # mistura interaural com atraso
│       ├── adaptive_eq.rs       # FFT de 20 ms, 5 bandas, headroom preditivo
│       ├── dynamics.rs          # compressor e limiter look-ahead
│       ├── loudness.rs          # curvas ISO 226:2003
│       ├── chain.rs             # cadeia completa + análise
│       ├── spectrum.rs          # 56 bandas, RMS e picos
│       ├── autoeq.rs            # parser ParametricEQ.txt
│       ├── presets.rs           # presets embutidos
│       ├── params.rs            # parâmetros serializáveis e sanitização
│       └── lib.rs
├── src-tauri/
│   ├── src/
│   │   ├── audio/
│   │   │   ├── engine.rs        # streams, buffers, previews, telemetria
│   │   │   ├── devices.rs       # enumeração e detecção de cabo virtual
│   │   │   ├── system_default.rs # troca/restauração da saída padrão (Windows)
│   │   │   └── mod.rs
│   │   ├── ear/                 # modelo de profundidade, geometria, overlay e mapeamento
│   │   ├── commands.rs          # superfície de comandos Tauri
│   │   ├── config.rs            # persistência atômica
│   │   ├── models.rs            # Settings, AppView, Telemetry...
│   │   ├── provision.rs         # instalação do cabo virtual por plataforma
│   │   ├── state.rs             # regras de presets, perfis, motor
│   │   ├── shortcuts.rs         # atalhos globais
│   │   ├── tray.rs              # ícone e menu de bandeja
│   │   └── lib.rs / main.rs
│   ├── capabilities/            # permissões Tauri
│   └── tauri.conf.json
├── src/
│   ├── components/              # shell, controles, gráficos, recorte da orelha, modais
│   ├── pages/                   # Painel, Equalizador, Espacial, Dinâmica, Presets, Orelha, Ajustes
│   ├── store/appStore.ts        # estado, debounce e updates otimistas
│   ├── lib/api.ts               # ponte Tauri + backend simulado do navegador
│   ├── lib/dsp.ts               # curva de resposta idêntica ao DSP
│   ├── lib/i18n.ts              # pt-BR / inglês
│   ├── lib/mock.ts              # prévia no navegador, sem áudio real
│   ├── styles.css               # design system
│   └── App.tsx / main.tsx
├── scripts/generate-icons.mjs   # ícones do app
└── README.md
```

---

## Pré-requisitos

### Todos os sistemas

- **Node.js 20.19+** (ou 22.12+) e npm
- **Rust stable** (edition 2021) — instale via [rustup](https://rustup.rs)
- **Tauri CLI** vem como dependência de desenvolvimento (`npm run tauri`)

### Windows

- Windows 10/11 x64
- **Visual Studio Build Tools** com “Desktop development with C++” e Windows SDK
- **WebView2** (já presente no Windows 11; instalável pelo runtime da Microsoft no Windows 10)

### macOS

- macOS 11+
- **Xcode Command Line Tools** (`xcode-select --install`)
- Áudio do sistema via **BlackHole 2ch** para o modo rota

### Linux

Pacotes de desenvolvimento do Tauri 2 e áudio (exemplo Debian/Ubuntu):

```bash
sudo apt update
sudo apt install build-essential curl wget file libssl-dev libxdo-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev
```

Fedora/openSUSE usam os equivalentes (`webkit2gtk4.1-devel`, `alsa-lib-devel` etc.). O backend usa CPAL/ALSA; em sistemas PipeWire ou PulseAudio isso funciona pela camada de compatibilidade.

---

## Instalação e execução

```bash
# dependências do frontend
npm install

# desenvolver com o app desktop real (hot reload)
npm run tauri dev

# gerar executável/instalador
npm run tauri build
```

Artefatos do build ficam em `target/release/` (executável) e `target/release/bundle/` (instalador).

### Prévia da interface no navegador

```bash
npm run dev
# http://127.0.0.1:1420
```

Sem o runtime Tauri, a UI usa um **backend simulado**: navegação, presets, tema, idioma, curva de EQ e telemetria fake funcionam, mas **não há processamento de áudio**. Isso serve para desenvolver a interface, não para avaliar som.

### Testes

```bash
cargo test --workspace     # DSP, parser AutoEQ, limites, resampler, sanitização
npm run build              # tsc + build de produção do frontend
```

Os testes cobrem o processamento e a lógica de configuração; não abrem dispositivos de áudio reais.

---

## Roteamento do som do sistema

O Audio Pulse não inclui driver próprio. Para processar **todo** o áudio, use um cabo virtual: o sistema envia o som para o cabo, o Audio Pulse captura o cabo e entrega o áudio processado ao fone. Em **Ajustes → Áudio**, o botão **Instalar cabo virtual** baixa o instalador oficial, verifica a assinatura digital e o executa por você — resta apenas aprovar o pedido de administrador do sistema.

### Windows — VB-CABLE

1. Em **Ajustes → Áudio**, clique em **Instalar cabo virtual** e aprove o UAC. O app baixa o pacote oficial do VB-Audio, confere a assinatura Authenticode e instala em modo silencioso; a captura passa a apontar para `CABLE Output` automaticamente.
2. No Audio Pulse, em **Ajustes → Áudio**, escolha **Dispositivo de saída**: seu fone/headset. O app define `CABLE Input` como saída padrão do Windows enquanto a rota está ativa e restaura a saída anterior ao parar ou fechar — não é preciso mexer nas configurações de som.
3. Inicie o motor em **Rota de áudio** (ele também inicia sozinho ao abrir o app quando captura e saída já estão configuradas).

A instalação automática exige internet e aprovação de administrador. Se preferir, instale manualmente pelo [site do VB-Audio](https://vb-audio.com/Cable/) e reinicie.

O app detecta automaticamente um dispositivo virtual quando a captura está em “Padrão do sistema”. `CABLE Output` é a ponta de **gravação** do cabo; `CABLE Input`, a de reprodução.

### Volume master e o volume do Windows

No Windows, o **volume master do app é o mesmo controle do volume do sistema**:

- Mover o slider do app, o controle de volume do Windows, a tecla de volume ou o flyout move o outro lado — a mudança do sistema é adotada em menos de meio segundo.
- Na rota ativa, o Windows mostra o master em `CABLE Input` (a saída padrão do momento), mas o cabo não aplica esse controle no sinal que entrega à captura — então a atenuação é feita pelo DSP, e o fone fica em 100 % para não atenuar duas vezes. Ao parar a rota, o master é devolvido ao fone antes da troca de saída.
- Volume zero silencia a saída pelo mudo do Windows, preservando o nível para quando o som voltar.
- O loudness ISO 226 continua usando o volume master como referência de escuta.

### Bateria do headset

Headsets sem fio cujo receptor USB expõe o relatório HID de carga (ex.: linha JBL Quantum Wireless) mostram a porcentagem ao lado do bypass, atualizada a cada 10 segundos. A leitura é feita pelo contêiner PnP do receptor, sem depender do modelo. Fones com fio e modelos que não publicam esse relatório simplesmente não exibem o chip.

### macOS — BlackHole

1. Em **Ajustes → Áudio**, clique em **Instalar cabo virtual** e informe a senha de administrador. O app baixa o **BlackHole 2ch** oficial, confere o checksum e a assinatura do pacote e instala.
2. Defina **BlackHole 2ch** como saída de som do macOS (a troca automática da saída padrão existe apenas no Windows).
3. No Audio Pulse: captura `BlackHole 2ch` (selecionada automaticamente), saída = seus fones; inicie a rota.
4. Se quiser ouvir também sem passar pelo app, crie um **Multi-Output Device** no Audio MIDI Setup — mas escolha apenas um caminho de saída para evitar eco.

### Linux — PipeWire/PulseAudio

A instalação automática está disponível no Windows e no macOS. No Linux, crie um sink nulo e capture o monitor:

```bash
pactl load-module module-null-sink sink_name=audiopulse \
  sink_properties=device.description=AudioPulse
# defina "Audio Pulse" como saída padrão do sistema
```

No app, selecione **Monitor of Audio Pulse** como captura e o fone como saída. Em PipeWire, `pactl` funciona pela compatibilidade PulseAudio; também é possível usar `pw-loopback`.

### Sem cabo virtual: sinais de teste

Na tela **Painel**, os botões **Ruído rosa**, **Varredura** e **Arquivo WAV** iniciam uma prévia processada direto no fone. Isso permite avaliar EQ, graves e espacial sem instalar cabo virtual. WAV é decodificado e reamostrado na taxa do dispositivo.

---

## Uso

1. Escolha a **saída** (fone) e, para rota, a **captura** (cabo virtual). Na primeira execução o app aplica o preset **Música**, para que a rota já soe processada em vez de neutra.
2. Clique em **Iniciar** → **Rota de áudio**. Com captura e saída configuradas, a rota também inicia ao abrir o app.
3. Ajuste o som nas páginas:
   - **Equalizador** — arraste os nós da curva ou use os sliders verticais; pré-amplificador, AutoEQ, tone, Bass Boost, loudness, crossfeed e EQ adaptativo.
   - **Espacial** — desligado, binaural, HRTF dinâmico ou surround 7.1; largura e ambiência.
   - **Dinâmica** — compressor e limiter, com curvas e redução de ganho em tempo real.
   - **Presets** — presets de modo, perfis de fone, presets personalizados e importação AutoEQ.
   - **Orelha** — foto local, enquadramento, análise de profundidade, curva estimada de 2–8 kHz e aplicação/restauração do AutoEQ.
   - **Ajustes** — tema, idioma, buffer, dispositivos, perfis por dispositivo, atalhos e inicialização.
4. **Bypass** na barra superior (ou atalho) compara instantaneamente com o som original.
5. Ao alterar qualquer parâmetro de modelagem, o preset ativo passa a **Personalizado**; volume e bypass nunca são sobrescritos por um preset.

### Presets e perfis

- **8 modos embutidos**: Música, Cinema, Jogos, Podcast, Vocal, Flat, Bass e Noturno.
- **12 perfis de fone**: Sony WH-1000XM4/XM5, Bose QC45/QC Ultra, Sennheiser HD 600/Momentum 4, Apple AirPods Max/Pro 2ª ger., JBL Tune 770NC, Samsung Galaxy Buds2 Pro, Audio-Technica ATH-M50x e Beyerdynamic DT 770 Pro.

> Os perfis embutidos são **curvas de referência Audio Pulse**, não medições de laboratório. Para correção específica do seu exemplar, importe um perfil AutoEQ medido.

### AutoEQ

Importe um arquivo `ParametricEQ.txt` do projeto AutoEQ:

```text
# AutoEQ profile
Preamp: -6.2 dB
Filter 1: ON PK Fc 105 Hz Gain 5.5 dB Q 0.70
Filter 2: ON LS Fc 80 Hz Gain -2.0 dB Q 0.70
Filter 3: OFF PK Fc 2000 Hz Gain -4.0 dB Q 1.20
```

O parser aceita `PK`, `LS`/`LSC` e `HS`/`HSC`, ignora filtros `OFF` e comentários, aplica o preamp e limita a 16 filtros. **GraphicEQ e convolução FIR não são suportados.** A importação liga o AutoEQ e desmarca o preset ativo, pois a correção passa a definir o som.

### Perfil por foto da orelha (estimativa)

1. Abra **Orelha** e escolha uma foto nítida do pavilhão inteiro, com luz difusa e sem contraluz. Cabelo sobre a concha atrapalha a leitura.
2. Ajuste **zoom**, **rotação** e posição dentro da guia. O recorte é sempre 512×512 e nada é enviado para a rede.
3. Na primeira vez que a página é aberta, o modelo de profundidade (~26 MB) é baixado automaticamente; o progresso aparece no cartão **Modelo**. Se o download falhar, use o botão para tentar de novo. **Analisar foto** mostra o recorte com a concha destacada, a confiança, quatro métricas e a curva estimada.
4. Se a confiança for suficiente, **Aplicar ao AutoEQ** substitui a curva atual. O AutoEQ anterior é salvo automaticamente; **Remover perfil** desliga a correção e **Restaurar AutoEQ anterior** devolve exatamente o estado anterior.

A análise e o backup ficam salvos entre sessões. Aplicar nunca acontece sozinho, e avisos de baixa confiança, enquadramento ou iluminação bloqueiam a aplicação.

### Exportar/importar presets

Exportar gera um JSON com envelope:

```json
{
  "format": "audiopulse-preset",
  "version": 1,
  "preset": {
    "id": "custom-123",
    "name": "Meu fone",
    "category": "custom",
    "builtIn": false,
    "params": { "...": "..." }
  }
}
```

A importação aceita o envelope ou um preset bruto, valida `format`/`version` e gera um novo ID. Parâmetros são sanitizados ao carregar.

### Perfis por dispositivo

Em **Ajustes → Perfis por dispositivo**, associe uma saída a um preset. O perfil é aplicado quando o motor inicia em modo rota. A lista de dispositivos é atualizada a cada 3 segundos; trocar a saída com o motor ligado reinicia o stream, mas o preset só é reaplicado na próxima inicialização da rota.

### Atalhos padrão

| Ação | Atalho |
| --- | --- |
| Bypass | `Ctrl/Cmd + Alt + B` |
| Presets Música/Cinema/Jogos/Podcast | `Ctrl/Cmd + Alt + 1/2/3/4` |
| Alternar áudio espacial | `Ctrl/Cmd + Alt + S` |
| Volume +/− | `Ctrl/Cmd + Alt + ↑/↓` |
| Abrir janela | `Ctrl/Cmd + Alt + O` |

Todos podem ser redefinidos em **Ajustes → Atalhos**. Conflitos com outros aplicativos geram um aviso na interface.

### Bandeja e inicialização

O menu da bandeja oferece bypass, volume, modo espacial e presets. Fechar a janela minimiza para a bandeja por padrão. A inicialização automática usa o plugin oficial do Tauri; com `--minimized`, o app inicia oculto quando a opção correspondente está ativa.

---

## Como funciona o Bass Boost

O grave percebido é construído em duas camadas:

1. **Reforço físico** — um low shelf de até **+15 dB × intensidade** na frequência de corte (30–250 Hz, Q 0,707). Isso reforça o fundamental que o fone consegue reproduzir.
2. **Síntese de harmônicos** — o sinal grave é extraído por um low-pass de 4ª ordem em ~1,6× o corte e saturado com `tanh` com ganho de entrada `1,5 + 5 × intensidade`, normalizado para não alterar o volume. A componente original é subtraída, sobrando apenas os harmônicos; um high-pass em ~1,35× o corte remove o fundamental remanescente, um DC blocker limpa o offset e um low-pass em 220 Hz suaviza o timbre. A mistura final é `0,9 × harmônicos × intensidade`.

O objetivo é o **fenômeno do fundamental ausente**: fones pequenos não reproduzem 40–80 Hz com energia, mas reproduzem bem 100–240 Hz. O cérebro infere o fundamental a partir dos harmônicos gerados, percebendo mais grave com menos excursão do driver e menos distorção mecânica do que um boost puro.

Harmônicos, corte e intensidade são independentes; com harmônicos em 0% resta apenas o shelf.

## Como funciona o Spatial Audio

O modo **Surround** faz upmix de estéreo para 8 canais virtuais — L, R, C, LFE, Ls, Rs, Lb, Rb — com níveis e posições fixas. Cada alto-falante virtual é renderizado com um modelo de cabeça esférica:

- **ITD (Woodworth)**: atraso interaural `(r/c)(θ + sen θ)`, com `r = 8,75 cm` e `c = 343 m/s`, aplicado em delay fracionário.
- **Sombra contralateral**: low-pass dependente do ângulo, com corte `750 + 8500·cos(θ)` Hz, além de atenuação de até ~12 dB na orelha oposta.
- **Notch de orelha**: peaking em 7,3 kHz (frontal) ou 5,4 kHz (traseiro), simulando a filtragem da pinna.
- **Laterais e traseiras**: taps de delay de 9/12 ms (sides) e 19/23 ms (rears), com inversão de polaridade para decorrelação.
- **Ambiência**: reverb Schroeder estéreo (4 combs + 2 allpasses por orelha, pre-delay de 8 ms) alimentado pelo sinal ambiente; a intensidade escala com o controle **Ambiente**. O parâmetro **Amplitude** multiplica o azimute dos alto-falantes (0–1,5). Um trim final de −1,7 dB (surround) evita clipping.

O modo **HRTF dinâmico** usa o mesmo modelo de cabeça, mas em vez de posições fixas acompanha a imagem estéreo continuamente. Um seguidor de envelopes separa mid e side (ataque de 12 ms, liberação de 120 ms) e mede o quanto o mix está aberto:

```
lateral = 2 · side / (mid + side)
```

Conteúdo mono tem `lateral ≈ 0` e o par virtual fica em ±20°; conforme a decorrelação cresce, os azimutes abrem até ±78°, tudo multiplicado pelo controle **Amplitude** e limitado a 88°. A abertura é suavizada (ataque de 35 ms, liberação de 320 ms) e a geometria é recalculada a cada 2 ms:

- **ITD** por Woodworth em cada azimute — o canal mais distante atrasa mais;
- **Sombra contralateral** recalculada por `750 + 8500·cos(θ)` Hz — o lado oposto perde agudos;
- **Notch da pinna** reposicionado conforme o ângulo.

O resultado é uma imagem que abre e fecha com o conteúdo — um coral largo ocupa o palco, uma voz central fica à frente — sem o espalhamento fixo de um upmix. Os coeficientes são atualizados preservando o estado dos filtros, então a varredura de azimute não estala.

O modo **Binaural** não faz upmix: renderiza L/R com pequeno âncora central, preservando o mix original e adicionando distância, sombra e reflexões.

Nenhum dos modos usa uma HRTF medida: o modelo é físico e genérico, então funciona em qualquer fone sem medições individuais.

O modo **Crossfeed** é separado do espacial: mistura uma cópia do canal oposto com low-pass em 700 Hz e atraso de 0,05–1 ms (cerca de −5 dB no máximo), reduzindo a separação extrema típica de fones e aproximando a imagem de caixas.

## Como funciona a Equalização Adaptativa

A cada **20 ms** o estágio tira uma FFT de 2048 amostras com janela de Hann e soma a energia de cinco bandas de oitava centradas em **80 Hz, 300 Hz, 1 kHz, 3,5 kHz e 11 kHz**. Somar os bins — em vez de ler a magnitude de um único bin — integra a energia da oitava, então material musical, que cai cerca de 3 dB por oitava, lê aproximadamente plano nesses cinco pontos.

O alvo não é plano: usa uma inclinação suave de **+2 dB nos graves a −2 dB nos agudos** (`[+2, +1, 0, −1, −2] dB`) relativa à **mediana das bandas ativas**. Isso reflete a distribuição natural de música e cinema; um alvo plano deixaria quase todo material com brilho excessivo. Ancorar na mediana garante que as correções apenas redistribuem energia entre bandas — o estágio nunca muda o nível global.

O controlador é determinístico e auditável, **não é uma rede neural**:

- **Zona morta de 1,2 dB**: erros menores não geram correção, então o estágio não persegue ruído de medição.
- **Limite de ±3 dB** por banda, multiplicado pelo slider de **Intensidade**.
- **Bandas vazias ignoradas**: bandas mais de 30 dB abaixo da mais forte não são reforçadas, para não levantar hiss ou ruído de sala.
- **Silêncio congela a adaptação**: abaixo de −62 dBFS (ou pico < 10⁻⁴) os ganhos não se movem.
- **Suavização assimétrica**: 0,5 no ataque e 0,2 na liberação do envelope espectral; 0,18 por quadro nos ganhos. A correção soa musical em vez de pulsar.
- **Headroom preditivo**: o maior ganho positivo é comparado ao pico de amostra medido na janela. Se `pico + ganho` passaria de −1 dBFS, todos os reforços são escalados para baixo; cortes continuam permitidos. Um sinal já no teto não recebe ganho positivo nenhum — o limiter de look-ahead continua como última linha de defesa, não como primeira.

O áudio em si passa por cinco filtros peaking (Q 0,8–1,0) no caminho direto, com coeficientes recalculados a cada 20 ms sem zerar o estado dos filtros e **sem adicionar latência**. A FFT roda em paralelo, apenas para atualizar os ganhos.

O medidor na página do equalizador mostra os cinco ganhos em tempo real, com o centro da barra em 0 dB e escala de ±3 dB. Desligar o estágio zera os ganhos e devolve o sinal sem alteração.

---

## Como funciona o perfil por foto da orelha

A análise é **local e sob demanda**: a foto é decodificada no backend, recortada em 512×512 e passada uma vez pelo modelo **Depth Anything V2 Small** (ONNX int8, entrada 518×518, normalização ImageNet). A saída é **profundidade inversa relativa** — valores maiores significam mais perto da câmera. O modelo roda na CPU com 1–4 threads e não participa do caminho de áudio; neste ambiente a análise levou cerca de 1,5–2,7 s.

A geometria vem da imagem, não de marcos anatômicos pontuais:

- Uma máscara de pele em YCbCr descarta cabelo, fundo e roupa; a busca do fundo e as estatísticas de borda usam apenas pele.
- A concha é a região conectada mais profunda dentro de uma zona elíptica na parte inferior do recorte. O crescimento parte do pixel de pele mais profundo e usa uma banda adaptativa de 10–22% da variação de profundidade.
- `concha_size` é a área do fundo dividida pela área da guia; `concha_depth` é a diferença entre a mediana da borda (hélice/anti-hélice) e a média do fundo, normalizada por 0,5; `helix_prominence` compara os percentis 90 e 50 da borda superior.
- A confiança combina contraste de profundidade, profundidade do fundo e centralização, com penalidades para fundo encostado no limite, ausência de fundo e pouca pele. Os avisos `no_ear`, `no_basin`, `off_center`, `low_contrast`, `dark` e `overexposed` aparecem na interface e bloqueiam a aplicação quando o resultado não é confiável.

O mapeamento para áudio é uma **heurística paramétrica**, não uma HRTF medida. A direção vem de estudos de antropometria da pinna (concha maior e mais profunda tende a deslocar e reforçar a primeira ressonância da concha), e a correção move três filtros peaking em 2–8 kHz na direção da média da população. O pré-amplificador compensa o maior ganho positivo. O resultado é composto por três filtros `PK` e um preamp — **não é convolução, não é reverb e não é uma rede neural no áudio**.

O que não existe aqui: detecção dos marcos hélice/anti-hélice/lóbulo como pontos nomeados, HRTF individual, modelagem do canal auditivo, orelha interna ou sombra da cabeça. Uma foto 2D não contém essas informações. O perfil é um ponto de partida para experimentar, não substitui uma medição AutoEQ do fone.

**Modelo e licenças.** O arquivo é o `onnx/model_int8.onnx` de [`onnx-community/depth-anything-v2-small`](https://huggingface.co/onnx-community/depth-anything-v2-small) — 27.258.801 bytes, SHA-256 `01aa7a23de3f4a0ee1a2bb9997e6918104c85a9f95dea46d27b9b3fb0c6b9001`, licença **Apache-2.0**. O download acontece automaticamente na primeira vez que a página **Orelha** é aberta (o botão serve para repetir em caso de falha), e o arquivo fica na pasta `models/` ao lado do `config.json`. O ONNX Runtime é vinculado estaticamente ao executável; a análise não faz chamadas de rede, e a foto nunca é enviada.

---

## Latência, buffer e CPU

- O buffer é configurável de **10 a 200 ms** (padrão 25 ms) e é convertido em frames na taxa do dispositivo.
- A latência exibida é uma **estimativa**: rota = `3 × buffer + 5 ms` (captura + fila + saída + look-ahead); prévia = `buffer + 5 ms`. Drivers, o cabo virtual e o Bluetooth adicionam latência que o app não mede.
- A captura e a saída podem ter taxas diferentes; a conversão é linear com correção de drift de ±0,15% baseada no preenchimento do ring buffer. O ring mantém **duas rodadas de buffer** de folga: uma cobre o intervalo normal entre callbacks e a outra absorve um callback atrasado. Acima de quatro rodadas, as amostras mais antigas são descartadas de volta ao nível de folga e o evento conta um **xrun**.
- A CPU mostrada é a carga do DSP dividida pela duração do bloco processado, suavizada — não é o uso total do processo.
- **HRTF dinâmico e EQ adaptativo não adicionam latência**: a análise (retune a cada 2 ms / FFT de 20 ms) roda em paralelo; o áudio segue por biquads no caminho direto.
- Para Bluetooth, prefira 30–50 ms de buffer. Para uso com fio, 10–20 ms costuma ser estável.

---

## Persistência

Configuração, presets personalizados e perfis ficam em:

| Sistema | Caminho |
| --- | --- |
| Windows | `%APPDATA%\AudioPulse\config.json` |
| macOS | `~/Library/Application Support/AudioPulse/config.json` |
| Linux | `~/.config/AudioPulse/config.json` |

A gravação é atômica (arquivo temporário + rename) e sofre debounce para não escrever a cada movimento de slider. Se o JSON estiver corrompido, ele é renomeado para `config.corrupt-<timestamp>.json` e o app inicia com padrões.

Na primeira execução do Audio Pulse, se ainda não existir `config.json` na pasta nova, o app copia automaticamente a configuração, o `route-default.json` e a pasta `models/` de `%APPDATA%\Audio Pulse` (ou do diretório equivalente no macOS/Linux). A pasta antiga não é apagada, então voltar para uma versão anterior continua funcionando.

O perfil da orelha (última análise e AutoEQ anterior) fica no `localStorage` do webview, na chave `audiopulse.ear-profile`; aplicar o perfil grava o AutoEQ em `config.json` pelo caminho normal de parâmetros. O modelo de profundidade baixado fica em `models/depth-anything-v2-small-int8.onnx` ao lado do `config.json`; apagar esse arquivo força um novo download na próxima análise.

Enquanto a rota está ativa, o endpoint anterior fica registrado em `route-default.json` ao lado do `config.json`. Se o app for encerrado à força, esse registro é usado na próxima abertura para devolver a saída padrão e o volume ao fone.

---

## Limitações conhecidas

- **Roteamento exige cabo virtual.** Não há driver de áudio próprio (APO no Windows, Audio Server Plugin no macOS, filtro nativo no PipeWire). O app não captura o áudio do sistema sem um dispositivo virtual. No Windows e no macOS ele instala o cabo oficial por você (requer internet e aprovação de administrador); no Linux, a criação do sink é manual. A troca automática da saída padrão existe apenas no Windows; no macOS e no Linux, selecione o cabo/sink manualmente.
- **Sem WASAPI exclusivo nem ASIO.** O backend usa CPAL em modo compartilhado; não há acesso exclusivo ao dispositivo nem baixa latência de ASIO.
- **Sem entrada 7.1 real.** O surround é um upmix de estéreo, não decodificação de faixas multicanal.
- **HRTF genérica.** O modo HRTF dinâmico adapta os ângulos ao conteúdo, mas o modelo esférico não usa medições individuais; a externalização varia entre pessoas e fones.
- **EQ adaptativo não é correção de sala nem de fone.** Ele equilibra o material em cinco bandas de oitava; não mede a resposta do fone e não substitui AutoEQ. Não há aprendizado de máquina: o controlador é preditivo e determinístico.
- **ISO 226 aproximada.** A compensação usa o volume master como proxy do nível de escuta, sem calibração de SPL.
- **AutoEQ apenas paramétrico.** Arquivos GraphicEQ e respostas de impulso (FIR) não são importados.
- **O perfil por foto é uma estimativa geométrica, não uma HRTF.** Ele depende de enquadramento, iluminação e da máscara de pele, e o mapeamento para três filtros é heurístico. Foi validado com o modelo real em duas fotos de orelha; não há validação clínica nem medição individual. Trate a curva como ponto de partida e use **Restaurar AutoEQ anterior** para voltar atrás.
- **Sem marcos anatômicos nomeados e sem redes neurais no áudio.** O app não detecta hélice/anti-hélice/lóbulo como pontos (o MediaPipe Face Landmarker não modela a pinna), não gera HRTF individual e não usa LSTM/Transformer nem convolução: o perfil por foto produz somente três filtros peaking e um preamp.
- **Perfis de fone embutidos são curvas de referência**, não medições.
- **Bluetooth** adiciona latência fora do controle do app; sincronizar vídeo pode exigir ajustes no player.
- **Volume sincronizado e bateria são recursos do Windows.** No macOS e no Linux o volume master permanece dentro do DSP e não há leitura de bateria; o chip não aparece.
- **Bateria depende do relatório do fabricante.** O app lê o relatório HID de carga quando existe; modelos que só expõem a carga no aplicativo do fabricante não mostram o nível.
- **Prévia no navegador não processa áudio**, apenas simula o backend.
- **macOS/Linux** não foram validados em hardware neste ambiente de desenvolvimento; o código usa APIs portáveis do CPAL e do Tauri. A reprodução com cabo virtual e fones também não foi testada em hardware aqui — a verificação foi feita por testes automatizados e pela interface.

---

## Solução de problemas

**“Nenhum cabo virtual detectado”**
Em **Ajustes → Áudio**, clique em **Instalar cabo virtual** e aprove a elevação. Se a instalação automática não estiver disponível (Linux ou falha de rede), instale o VB-Cable/BlackHole/sink nulo manualmente, clique em **Atualizar** e selecione a captura. O detector reconhece nomes como Cable Output, VB-Audio, VoiceMeeter, BlackHole, Soundflower, “Monitor of”, PipeWire e null sink.

**Motor inicia, mas não há som**
No Windows, o app troca a saída padrão para `CABLE Input` sozinho enquanto a rota está ativa e a restaura ao parar. Confira se a captura selecionada é a ponta correta (ex.: `CABLE Output`), se o fone está na saída do app, se o volume master não está zerado e se o bypass não está ativo. Se o app for encerrado à força, a saída anterior é restaurada na próxima abertura.

**Estalos, cortes ou xruns**
Aumente o buffer em Ajustes → Áudio. Evite taxas diferentes entre cabo e fone; se possível, use 48 kHz em ambos. Bluetooth costuma exigir 30 ms ou mais.

**Atalho não funciona**
Outro aplicativo pode ter registrado a combinação. Troque em Ajustes → Atalhos; o erro aparece nos avisos.

**App fecha e some**
Ele foi para a bandeja. Abra pelo ícone ou use o atalho de janela; desative “Fechar para a bandeja” para fechar de verdade.

**Configuração corrompida**
O app cria um backup `config.corrupt-*.json`. Para zerar tudo, feche o app e remova `config.json` do caminho correspondente.

---

## Licença

GPL-3.0-or-later. Veja [LICENSE](../LICENSE) e [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
