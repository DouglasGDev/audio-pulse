import type { AutoEqFilter, FilterKind, ProcessingParams } from "./types";

export const EQ_Q = 1.41;
export const CURVE_RANGE_DB = 15;

interface Coeffs {
  b0: number;
  b1: number;
  b2: number;
  a0: number;
  a1: number;
  a2: number;
}

function peaking(sampleRate: number, freq: number, gainDb: number, q: number): Coeffs {
  const a = Math.pow(10, gainDb / 40);
  const w0 = (2 * Math.PI * Math.max(10, Math.min(freq, sampleRate / 2 - 1))) / sampleRate;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * Math.max(0.05, q));
  return {
    b0: 1 + alpha * a,
    b1: -2 * cos,
    b2: 1 - alpha * a,
    a0: 1 + alpha / a,
    a1: -2 * cos,
    a2: 1 - alpha / a,
  };
}

function lowShelf(sampleRate: number, freq: number, gainDb: number, q: number): Coeffs {
  const a = Math.pow(10, gainDb / 40);
  const w0 = (2 * Math.PI * Math.max(10, Math.min(freq, sampleRate / 2 - 1))) / sampleRate;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * Math.max(0.05, q));
  const sqrt = 2 * Math.sqrt(a) * alpha;
  return {
    b0: a * (a + 1 - (a - 1) * cos + sqrt),
    b1: 2 * a * (a - 1 - (a + 1) * cos),
    b2: a * (a + 1 - (a - 1) * cos - sqrt),
    a0: a + 1 + (a - 1) * cos + sqrt,
    a1: -2 * (a - 1 + (a + 1) * cos),
    a2: a + 1 + (a - 1) * cos - sqrt,
  };
}

function highShelf(sampleRate: number, freq: number, gainDb: number, q: number): Coeffs {
  const a = Math.pow(10, gainDb / 40);
  const w0 = (2 * Math.PI * Math.max(10, Math.min(freq, sampleRate / 2 - 1))) / sampleRate;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * Math.max(0.05, q));
  const sqrt = 2 * Math.sqrt(a) * alpha;
  return {
    b0: a * (a + 1 + (a - 1) * cos + sqrt),
    b1: -2 * a * (a - 1 + (a + 1) * cos),
    b2: a * (a + 1 + (a - 1) * cos - sqrt),
    a0: a + 1 - (a - 1) * cos + sqrt,
    a1: 2 * (a - 1 - (a + 1) * cos),
    a2: a + 1 - (a - 1) * cos - sqrt,
  };
}

function magnitudeDb(coeffs: Coeffs, sampleRate: number, freq: number): number {
  const w = (2 * Math.PI * freq) / sampleRate;
  const cos1 = Math.cos(w);
  const sin1 = -Math.sin(w);
  const cos2 = Math.cos(2 * w);
  const sin2 = -Math.sin(2 * w);
  const numRe = coeffs.b0 + coeffs.b1 * cos1 + coeffs.b2 * cos2;
  const numIm = coeffs.b1 * sin1 + coeffs.b2 * sin2;
  const denRe = coeffs.a0 + coeffs.a1 * cos1 + coeffs.a2 * cos2;
  const denIm = coeffs.a1 * sin1 + coeffs.a2 * sin2;
  const num = Math.hypot(numRe, numIm);
  const den = Math.hypot(denRe, denIm);
  if (den < 1e-12) return 0;
  return 20 * Math.log10(Math.max(1e-9, num / den));
}

function coeffsFor(
  kind: FilterKind,
  sampleRate: number,
  freq: number,
  gainDb: number,
  q: number,
): Coeffs {
  if (kind === "lowshelf") return lowShelf(sampleRate, freq, gainDb, q);
  if (kind === "highshelf") return highShelf(sampleRate, freq, gainDb, q);
  return peaking(sampleRate, freq, gainDb, q);
}

/**
 * Magnitude of every tonal stage in the DSP chain at one frequency. Loudness,
 * spatial and dynamics are intentionally excluded because they depend on level
 * or are not minimum-phase, so a static curve would be misleading.
 */
export function responseDb(
  params: ProcessingParams,
  freq: number,
  sampleRate = 48000,
): number {
  let db = params.preampDb + (params.autoEq.enabled ? params.autoEq.preampDb : 0);

  const bands = [
    31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
  ];
  bands.forEach((band, index) => {
    const gain = params.eqDb[index] ?? 0;
    if (Math.abs(gain) > 0.001) {
      db += magnitudeDb(peaking(sampleRate, band, gain, EQ_Q), sampleRate, freq);
    }
  });

  if (params.autoEq.enabled) {
    for (const filter of params.autoEq.filters) {
      if (Math.abs(filter.gainDb) > 0.001) {
        db += magnitudeDb(
          coeffsFor(filter.kind, sampleRate, filter.freq, filter.gainDb, filter.q),
          sampleRate,
          freq,
        );
      }
    }
  }

  if (Math.abs(params.tone.midDb) > 0.001) {
    db += magnitudeDb(peaking(sampleRate, 1500, params.tone.midDb, 0.75), sampleRate, freq);
  }
  if (Math.abs(params.tone.trebleDb) > 0.001) {
    db += magnitudeDb(
      highShelf(sampleRate, 6000, params.tone.trebleDb, 0.707),
      sampleRate,
      freq,
    );
  }
  if (params.bass.enabled && params.bass.intensity > 0.0001) {
    db += magnitudeDb(
      lowShelf(
        sampleRate,
        params.bass.cutoffHz,
        15 * params.bass.intensity,
        0.707,
      ),
      sampleRate,
      freq,
    );
  }
  return db;
}

/** Response of a stand-alone parametric filter set, e.g. an imported profile. */
export function parametricResponseDb(
  filters: AutoEqFilter[],
  freq: number,
  preampDb = 0,
  sampleRate = 48000,
): number {
  let db = preampDb;
  for (const filter of filters) {
    if (Math.abs(filter.gainDb) > 0.001) {
      db += magnitudeDb(
        coeffsFor(filter.kind, sampleRate, filter.freq, filter.gainDb, filter.q),
        sampleRate,
        freq,
      );
    }
  }
  return db;
}

export interface CurvePoint {
  freq: number;
  db: number;
}

export function curvePoints(
  params: ProcessingParams,
  count = 220,
  minFreq = 20,
  maxFreq = 20000,
): CurvePoint[] {
  const points: CurvePoint[] = [];
  const ratio = Math.log(maxFreq / minFreq);
  for (let i = 0; i < count; i += 1) {
    const freq = minFreq * Math.exp((ratio * i) / (count - 1));
    points.push({ freq, db: responseDb(params, freq) });
  }
  return points;
}

export function logPosition(freq: number, minFreq = 20, maxFreq = 20000): number {
  return Math.log(freq / minFreq) / Math.log(maxFreq / minFreq);
}