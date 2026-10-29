// Browser-only mirror of the Rust preset catalog.
//
// The app always receives presets from the Tauri backend; this file exists so
// that `npm run dev` without Tauri still shows the real catalog. When the
// Rust presets change, regenerate this file from AutoEQ rather than editing
// values by hand.

import type { AutoEqFilter, Preset, PresetCategory, ProcessingParams } from "./types";

const f = (
  kind: AutoEqFilter["kind"],
  freq: number,
  gainDb: number,
  q: number,
): AutoEqFilter => ({ kind, freq, gainDb, q });

const pk = (freq: number, gainDb: number, q: number) => f("peaking", freq, gainDb, q);
const ls = (freq: number, gainDb: number, q: number) => f("lowshelf", freq, gainDb, q);
const hs = (freq: number, gainDb: number, q: number) => f("highshelf", freq, gainDb, q);

export function defaultParams(): ProcessingParams {
  return {
    preampDb: 0,
    eqDb: new Array(10).fill(0),
    bass: { enabled: false, intensity: 0.45, cutoffHz: 90, harmonics: 0.35 },
    tone: { midDb: 0, trebleDb: 0 },
    compressor: {
      enabled: false,
      thresholdDb: -18,
      ratio: 2,
      attackMs: 10,
      releaseMs: 150,
      kneeDb: 6,
      makeupDb: 0,
      sidechainHpf: true,
    },
    limiter: { enabled: true, ceilingDb: -0.3, releaseMs: 80 },
    loudness: { enabled: false, intensity: 0.6 },
    loudSoundReduction: { enabled: false, intensity: 0.6 },
    spatial: { mode: "off", width: 1, room: 0.35 },
    crossfeed: { enabled: false, amount: 0.3, delayMs: 0.3 },
    autoEq: { enabled: true, preampDb: 0, filters: [], name: null },
    adaptiveEq: { enabled: false, intensity: 0.55 },
    earProfile: { enabled: false, preampDb: 0, filters: [] },
    volume: 1,
    bypass: false,
  };
}

function modePreset(
  id: string,
  name: string,
  category: PresetCategory,
  description: string,
  preampDb: number,
  build: (params: ProcessingParams) => void,
): Preset {
  const params = defaultParams();
  params.limiter.enabled = true;
  params.preampDb = preampDb;
  build(params);
  return {
    id,
    name,
    category,
    description,
    brand: null,
    model: null,
    builtIn: true,
    params,
  };
}

function headphonePreset(
  id: string,
  brand: string,
  model: string,
  source: string,
  preampDb: number,
  filters: AutoEqFilter[],
): Preset {
  const params = defaultParams();
  params.autoEq = { enabled: true, preampDb, filters, name: null };
  return {
    id,
    name: `${brand} ${model}`,
    category: "headphone",
    description:
      `Correção medida via AutoEQ (${source}). Alvo de compensação: o resultado varia com a vedação, a unidade e o encaixe.`,
    brand,
    model,
    builtIn: true,
    params,
  };
}

export const MOCK_PRESETS: Preset[] = [
  modePreset(
    "music",
    "Música",
    "music",
    "Curva equilibrada, graves sutis e compensação de loudness moderada.",
    -1.84,
    (p) => {
      p.eqDb = [1, 0.8, 0.3, 0, -0.4, 0, 0.3, 0.6, 0.8, 0.5];
      p.bass = { enabled: true, intensity: 0.25, cutoffHz: 85, harmonics: 0.25 };
      p.loudness = { enabled: true, intensity: 0.5 };
    },
  ),
  modePreset(
    "cinema",
    "Cinema",
    "cinema",
    "Palco amplo, LFE reforçado e diálogos centralizados.",
    -7.16,
    (p) => {
      p.eqDb = [2.5, 2, 1, 0, -0.5, 0, 1, 1.5, 1, 0];
      p.bass = { enabled: true, intensity: 0.5, cutoffHz: 75, harmonics: 0.4 };
      p.spatial = { mode: "surround", width: 1.1, room: 0.5 };
      p.compressor = {
        enabled: true,
        thresholdDb: -20,
        ratio: 2.2,
        attackMs: 15,
        releaseMs: 200,
        kneeDb: 6,
        makeupDb: 2,
        sidechainHpf: true,
      };
      p.loudness = { enabled: true, intensity: 0.4 };
    },
  ),
  modePreset(
    "gaming",
    "Jogos",
    "gaming",
    "Passos nítidos, palco amplo e dinâmica controlada.",
    -1.27,
    (p) => {
      p.eqDb = [0.5, 0.5, 0, -1, 0, 1, 2, 1.5, 0.5, 0];
      p.bass = { enabled: true, intensity: 0.25, cutoffHz: 85, harmonics: 0.2 };
      p.spatial = { mode: "surround", width: 1.3, room: 0.25 };
      p.crossfeed = { enabled: true, amount: 0.15, delayMs: 0.3 };
      p.compressor = {
        enabled: true,
        thresholdDb: -22,
        ratio: 2,
        attackMs: 5,
        releaseMs: 100,
        kneeDb: 6,
        makeupDb: 2,
        sidechainHpf: true,
      };
      p.limiter.ceilingDb = -0.5;
    },
  ),
  modePreset(
    "competitive",
    "Competitivo",
    "gaming",
    "Passos e direções em primeiro plano: graves contidos, presença realçada e reflexões mínimas.",
    -0.7,
    (p) => {
      p.eqDb = [-3, -2, -1, -0.5, 0, 1, 2.5, 3, 1.5, 0];
      p.spatial = { mode: "hrtf", width: 1.1, room: 0.1 };
      p.compressor = {
        enabled: true,
        thresholdDb: -26,
        ratio: 2.5,
        attackMs: 3,
        releaseMs: 90,
        kneeDb: 4,
        makeupDb: 1,
        sidechainHpf: true,
      };
      p.limiter.ceilingDb = -1;
    },
  ),
  modePreset(
    "podcast",
    "Podcast",
    "podcast",
    "Voz à frente, graves reduzidos e nível consistente.",
    -0.05,
    (p) => {
      p.eqDb = [-4, -3, -2, -1, 0, 1, 2.5, 2, 1, 0];
      p.compressor = {
        enabled: true,
        thresholdDb: -24,
        ratio: 3,
        attackMs: 8,
        releaseMs: 120,
        kneeDb: 6,
        makeupDb: 4,
        sidechainHpf: true,
      };
      p.loudness = { enabled: true, intensity: 0.7 };
    },
  ),
  modePreset(
    "vocal",
    "Vocal",
    "vocal",
    "Presença e clareza para voz, mantendo os extremos suaves.",
    -0.18,
    (p) => {
      p.eqDb = [-2, -1.5, -0.5, 0, 0.8, 1.5, 2.5, 2, 1, 0.5];
      p.compressor = {
        enabled: true,
        thresholdDb: -20,
        ratio: 2,
        attackMs: 12,
        releaseMs: 160,
        kneeDb: 3,
        makeupDb: 1.5,
        sidechainHpf: true,
      };
      p.loudness = { enabled: true, intensity: 0.5 };
    },
  ),
  modePreset(
    "clarity",
    "Clareza",
    "clarity",
    "Detalhes e articulação em destaque, com médios-graves limpos e sem compressão pesada.",
    -0.09,
    (p) => {
      p.eqDb = [0, -0.5, -1.5, -1, 0, 0.5, 1.5, 2.5, 2, 1];
      p.loudness = { enabled: true, intensity: 0.35 };
    },
  ),
  modePreset(
    "immersive",
    "Imersivo",
    "immersive",
    "Palco amplo com reflexões e ambiência para filmes, séries e trilhas.",
    -3.86,
    (p) => {
      p.eqDb = [1.5, 1.2, 0.5, 0, -0.3, 0, 0.8, 1.2, 1, 0.5];
      p.bass = { enabled: true, intensity: 0.35, cutoffHz: 80, harmonics: 0.25 };
      p.spatial = { mode: "surround", width: 1.25, room: 0.65 };
      p.loudness = { enabled: true, intensity: 0.45 };
    },
  ),
  modePreset(
    "flat",
    "Flat",
    "flat",
    "Sem equalização ou realce; apenas proteção do limiter.",
    0,
    (p) => {

    },
  ),
  modePreset(
    "bass",
    "Bass",
    "bass",
    "Subwoofer virtual e graves amplos para fones de qualquer tamanho.",
    -4.57,
    (p) => {
      p.eqDb = [1.5, 1.5, 1, 0, -1, 0, 0.5, 0.5, 0, 0];
      p.bass = { enabled: true, intensity: 0.4, cutoffHz: 70, harmonics: 0.2 };
      p.loudness = { enabled: true, intensity: 0.2 };
    },
  ),
  modePreset(
    "night",
    "Noturno",
    "night",
    "Faixa dinâmica comprimida e compensação de loudness forte.",
    0,
    (p) => {
      p.eqDb = [-1, -0.5, 0, 0.5, 0.5, 0.5, 0, 0, 0, 0];
      p.compressor = {
        enabled: true,
        thresholdDb: -30,
        ratio: 5,
        attackMs: 5,
        releaseMs: 250,
        kneeDb: 6,
        makeupDb: 6,
        sidechainHpf: true,
      };
      p.loudness = { enabled: true, intensity: 0.9 };
      p.limiter.ceilingDb = -1;
    },
  ),
  modePreset(
    "protection",
    "Proteção",
    "protection",
    "Graves contidos, dinâmica controlada, redução de sons altos e teto de pico baixo para sessões longas.",
    -2,
    (p) => {
      p.compressor = {
        enabled: true,
        thresholdDb: -24,
        ratio: 3,
        attackMs: 5,
        releaseMs: 250,
        kneeDb: 6,
        makeupDb: 0,
        // Protection must attenuate every frequency evenly.
        sidechainHpf: false,
      };
      p.loudSoundReduction.enabled = true;
      p.limiter.ceilingDb = -3;
    },
  ),
  headphonePreset(
    "hp-jbl-quantum-100",
    "JBL",
    "Quantum 100",
    "AutoEQ · Rtings HMS II.3",
    -6.6,
    [
      ls(105, 1.7, 0.7),
      pk(192, -7.3, 0.73),
      pk(1660, 3.7, 0.18),
      pk(50, 3, 0.91),
      pk(7901, -8.6, 1.68),
      hs(10000, -3.5, 0.7),
      pk(4052, 5, 3.59),
      pk(2856, -4.3, 4.23),
      pk(552, 3.3, 3.18),
      pk(779, -3.3, 3.14),
    ],
  ),
  headphonePreset(
    "hp-jbl-quantum-400",
    "JBL",
    "Quantum 400",
    "AutoEQ · Rtings HMS II.3",
    -6.5,
    [
      ls(105, 18, 0.7),
      pk(8615, -6.4, 0.34),
      pk(47, -20, 0.3),
      pk(4612, 11.1, 1.76),
      pk(291, 6, 0.76),
      hs(10000, 0.3, 0.7),
      pk(1580, 1.1, 1.42),
      pk(2869, -2.7, 3.79),
      pk(3833, 1.5, 4.28),
      pk(766, -0.9, 2.86),
    ],
  ),
  headphonePreset(
    "hp-jbl-quantum-800",
    "JBL",
    "Quantum 800",
    "AutoEQ · Rtings HMS II.3",
    -6.2,
    [
      ls(105, 9.2, 0.7),
      pk(10000, -7.5, 0.4),
      pk(59, -12.1, 0.3),
      pk(4339, 10, 1.93),
      pk(375, 3.8, 0.37),
      hs(10000, 3.6, 0.7),
      pk(9190, -2, 3.27),
      pk(1529, 1.6, 2.86),
      pk(2660, -1.4, 3.7),
      pk(690, -1.1, 3.19),
    ],
  ),
  headphonePreset(
    "hp-jbl-quantum-one",
    "JBL",
    "Quantum ONE",
    "AutoEQ · Rtings HMS II.3",
    -3,
    [
      ls(105, 2.1, 0.7),
      pk(6146, -2.8, 3.65),
      pk(1338, 3.7, 0.76),
      pk(2482, -7, 2.23),
      pk(8616, -4, 2.54),
      hs(10000, -1.7, 0.7),
      pk(46, -1.5, 1.95),
      pk(152, 1.3, 2.2),
      pk(4404, 2.9, 6),
      pk(371, 0.9, 2.51),
    ],
  ),
  headphonePreset(
    "hp-jbl-tune770nc",
    "JBL",
    "Tune 770NC",
    "AutoEQ · Rtings B&K 5128",
    -2.5,
    [
      ls(105, -3.8, 0.7),
      pk(8989, -4.6, 1.76),
      pk(1646, 2.9, 1.03),
      pk(111, 2.9, 1.24),
      pk(2845, -2.6, 2.14),
      hs(10000, -1.8, 0.7),
      pk(514, -2.6, 2.09),
      pk(5044, 3, 4.22),
      pk(806, 1.4, 2.84),
      pk(6424, -2.3, 6),
    ],
  ),
  headphonePreset(
    "hp-sony-wh1000xm4",
    "Sony",
    "WH-1000XM4",
    "AutoEQ · oratory1990",
    -6.1,
    [
      ls(105, -4.2, 0.7),
      pk(143, -5.2, 1.1),
      pk(2289, 6.1, 1.57),
      pk(56, 1.2, 1.19),
      pk(5144, -3.2, 6),
      hs(10000, -1, 0.7),
      pk(407, 1.7, 3.14),
      pk(6715, 3, 5.99),
      pk(1007, 1, 3.41),
      pk(576, -1.2, 3.55),
    ],
  ),
  headphonePreset(
    "hp-sony-wh1000xm5",
    "Sony",
    "WH-1000XM5",
    "AutoEQ · oratory1990",
    -6.2,
    [
      ls(105, -3.2, 0.7),
      pk(2448, 6.9, 2.46),
      pk(173, -5.6, 0.96),
      pk(3028, -5.4, 2.03),
      pk(1327, 3.3, 0.58),
      hs(10000, 4.9, 0.7),
      pk(6110, -2.3, 5.81),
      pk(875, -1.2, 4.07),
      pk(1197, 1, 3.28),
      pk(63, 0.4, 2.13),
    ],
  ),
  headphonePreset(
    "hp-bose-qc45",
    "Bose",
    "QuietComfort 45",
    "AutoEQ · oratory1990",
    -2.2,
    [
      ls(105, -1.4, 0.7),
      pk(161, -2.2, 1.15),
      pk(5490, -7.7, 2.06),
      pk(4242, 3.5, 0.24),
      pk(2328, -5.2, 2.15),
      hs(10000, -7, 0.7),
      pk(8352, 2.9, 1.93),
      pk(390, 0.5, 2.32),
      pk(6508, -1, 5.04),
      pk(70, -0.1, 1.97),
    ],
  ),
  headphonePreset(
    "hp-bose-qc-ultra",
    "Bose",
    "QuietComfort Ultra Headphones",
    "AutoEQ · Rtings B&K 5128",
    -2.8,
    [
      ls(105, -3, 0.7),
      pk(5032, 2.8, 3.69),
      pk(156, 1.5, 1.61),
      pk(94, -2.4, 2.32),
      pk(2373, -2.8, 5.58),
      hs(10000, 2.7, 0.7),
      pk(1633, 2.6, 3.51),
      pk(491, -1.1, 2.5),
      pk(2194, -1, 3.76),
      pk(36, -0.5, 2.83),
    ],
  ),
  headphonePreset(
    "hp-sennheiser-hd600",
    "Sennheiser",
    "HD 600",
    "AutoEQ · oratory1990",
    -6.3,
    [
      ls(105, 6.5, 0.7),
      pk(125, -2.7, 0.55),
      pk(8445, 3.3, 1.61),
      pk(522, 0.7, 1.02),
      pk(1298, -1.2, 2.14),
      hs(10000, -3.1, 0.7),
      pk(3158, -1.8, 3.67),
      pk(2166, 0.9, 3.32),
      pk(6639, 2.2, 5.82),
      pk(5433, -1.2, 5.7),
    ],
  ),
  headphonePreset(
    "hp-sennheiser-momentum4",
    "Sennheiser",
    "Momentum 4 Wireless",
    "AutoEQ · oratory1990",
    -6.4,
    [
      ls(105, -7.4, 0.7),
      pk(195, -3.6, 1.22),
      pk(2393, 6.1, 1.81),
      pk(1726, 1.6, 1.71),
      pk(2893, -3.2, 5.94),
      hs(10000, 0.5, 0.7),
      pk(63, 1.7, 2.86),
      pk(652, 1.2, 2.25),
      pk(952, -1, 3.69),
      pk(316, -0.9, 3.46),
    ],
  ),
  headphonePreset(
    "hp-apple-airpods-max",
    "Apple",
    "AirPods Max",
    "AutoEQ · oratory1990",
    -4.7,
    [
      ls(105, -3, 0.7),
      pk(7273, 3.6, 2.41),
      pk(218, -2.9, 1.41),
      pk(1031, -3.2, 0.99),
      pk(3185, 3.1, 0.56),
      hs(10000, -5.5, 0.7),
      pk(9508, 2.7, 2.2),
      pk(66, 0.6, 1.65),
      pk(4045, 2.1, 5.82),
      pk(4834, -1.8, 6),
    ],
  ),
  headphonePreset(
    "hp-apple-airpods-pro2",
    "Apple",
    "AirPods Pro 2",
    "AutoEQ · Rtings B&K 5128",
    -3.1,
    [
      ls(105, -1.5, 0.7),
      pk(377, -2.5, 0.74),
      pk(3442, 3, 0.67),
      pk(6303, -3.4, 4.47),
      pk(1339, 0.5, 2.65),
      hs(10000, 3.1, 0.7),
      pk(82, 1, 1.92),
      pk(170, -0.6, 1.86),
      pk(8133, -1.1, 2.86),
      pk(311, 0.4, 3.57),
    ],
  ),
  headphonePreset(
    "hp-samsung-buds2-pro",
    "Samsung",
    "Galaxy Buds2 Pro",
    "AutoEQ · oratory1990",
    -2.3,
    [
      ls(105, -2.3, 0.7),
      pk(7526, 2.2, 2.78),
      pk(266, -0.8, 1.35),
      pk(3426, -2.3, 3.7),
      pk(64, 2.2, 1.18),
      hs(10000, -3.8, 0.7),
      pk(8817, 1.6, 2.28),
      pk(2283, 1.2, 3.75),
      pk(4955, -1.2, 5.11),
      pk(977, 1, 5.18),
    ],
  ),
  headphonePreset(
    "hp-ath-m50x",
    "Audio-Technica",
    "ATH-M50x",
    "AutoEQ · oratory1990",
    -3.1,
    [
      ls(105, 0.6, 0.7),
      pk(156, -5.2, 0.73),
      pk(326, 5.3, 1.59),
      pk(7077, 2.8, 2.22),
      pk(3483, 2.1, 5.82),
      hs(10000, -4.1, 0.7),
      pk(45, -1.1, 1.9),
      pk(66, 1.4, 3.59),
      pk(787, -0.5, 1.79),
      pk(1640, 0.9, 3.41),
    ],
  ),
  headphonePreset(
    "hp-beyerdynamic-dt770",
    "Beyerdynamic",
    "DT 770 Pro",
    "AutoEQ · oratory1990",
    -5.1,
    [
      ls(105, -3, 0.7),
      pk(6430, -4.2, 0.97),
      pk(3684, 6.8, 2.88),
      pk(213, 3.5, 2.46),
      pk(139, -3.9, 4.92),
      hs(10000, -5.4, 0.7),
      pk(93, 3.5, 3),
      pk(9561, 2.3, 1.86),
      pk(45, -1.3, 1.74),
      pk(119, -2.1, 5.83),
    ],
  ),
  headphonePreset(
    "hp-hyperx-cloud-2",
    "HyperX",
    "Cloud II",
    "AutoEQ · oratory1990",
    -6.4,
    [
      ls(105, 4.1, 0.7),
      pk(170, -7, 1.06),
      pk(271, 3.7, 0.58),
      pk(3828, 5.4, 3.6),
      pk(57, -4.8, 0.58),
      hs(10000, -7, 0.7),
      pk(8085, -3, 3.98),
      pk(2378, -1.9, 2.78),
      pk(3700, 1.3, 2.71),
      pk(1123, 1.1, 3.29),
    ],
  ),
  headphonePreset(
    "hp-steelseries-arctis-nova-pro",
    "SteelSeries",
    "Arctis Nova Pro",
    "AutoEQ · Rtings B&K 5128",
    -6.7,
    [
      ls(105, -2.2, 0.7),
      pk(141, -5.7, 1.88),
      pk(8720, -7, 0.34),
      pk(4510, 11.9, 1.52),
      pk(359, 4.6, 1.63),
      hs(10000, 6.6, 0.7),
      pk(64, 4, 3.3),
      pk(9316, -2.5, 1.54),
      pk(92, -1.5, 2.55),
      pk(1981, 1, 5.16),
    ],
  ),
  headphonePreset(
    "hp-logitech-g-pro-x",
    "Logitech",
    "G Pro X",
    "AutoEQ · Rtings HMS II.3",
    -5.9,
    [
      ls(105, 1.3, 0.7),
      pk(177, -5.1, 0.7),
      pk(3727, 5.9, 2.52),
      pk(40, 2.9, 1.19),
      pk(381, 3.2, 1.1),
      hs(10000, 4.2, 0.7),
      pk(1060, 2.4, 3.35),
      pk(1557, -2.1, 2.55),
      pk(8739, -1.9, 2.81),
      pk(5189, 1.9, 6),
    ],
  ),
  headphonePreset(
    "hp-razer-blackshark-v2",
    "Razer",
    "BlackShark V2",
    "AutoEQ · Rtings HMS II.3",
    -5.9,
    [
      ls(105, 8.8, 0.7),
      pk(1186, 3.5, 0.18),
      pk(72, -5.1, 0.27),
      pk(8531, -4.4, 1.38),
      pk(2984, -5.8, 1.61),
      hs(10000, -2, 0.7),
      pk(4457, 2.8, 6),
      pk(104, 0.7, 2.16),
      pk(3714, -1.9, 6),
      pk(62, -0.7, 2.34),
    ],
  ),
];
