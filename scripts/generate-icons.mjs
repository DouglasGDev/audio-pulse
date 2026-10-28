import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "src-tauri", "icons");
mkdirSync(outDir, { recursive: true });

const MASTER = 1024;
const SS = 2; // supersampling factor
const W = MASTER * SS;

function clamp01(v) {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}
function mix(a, b, t) {
  return a + (b - a) * t;
}
function smoothstep(edge0, edge1, x) {
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}
function colorLerp(c0, c1, t) {
  return [mix(c0[0], c1[0], t), mix(c0[1], c1[1], t), mix(c0[2], c1[2], t)];
}

// Rounded-rectangle signed distance (in pixels).
function sdRoundRect(x, y, cx, cy, hw, hh, r) {
  const qx = Math.abs(x - cx) - (hw - r);
  const qy = Math.abs(y - cy) - (hh - r);
  const ax = Math.max(qx, 0);
  const ay = Math.max(qy, 0);
  return Math.sqrt(ax * ax + ay * ay) + Math.min(Math.max(qx, qy), 0) - r;
}

// Rounded vertical bar mask (capsule with rounded corners).
function sdRoundBar(x, y, cx, cy, hw, hh, r) {
  return sdRoundRect(x, y, cx, cy, hw, hh, r);
}

const bars = [
  // x offset, half-height (normalized to icon edge), width
  [-0.275, 0.062, 0.052],
  [-0.187, 0.115, 0.052],
  [-0.098, 0.185, 0.052],
  [0.0, 0.262, 0.056],
  [0.098, 0.155, 0.052],
  [0.187, 0.215, 0.052],
  [0.275, 0.095, 0.052],
];

function renderMaster() {
  const buf = Buffer.alloc(W * W * 4);
  const half = W / 2;
  const radius = 0.215 * W;
  const margin = 0.028 * W;
  const ringCx = half;
  const ringCy = half * 1.02;
  const ringR = 0.318 * W;
  const ringW = 0.012 * W;
  const glowCyan = [79, 214, 224];
  const glowViolet = [158, 112, 245];
  const accentA = [92, 242, 212];
  const accentB = [115, 161, 255];
  const accentC = [184, 128, 255];

  for (let py = 0; py < W; py++) {
    for (let px = 0; px < W; px++) {
      const x = px + 0.5;
      const y = py + 0.5;
      const idx = (py * W + px) * 4;

      const sd = sdRoundRect(x, y, half, half, half - margin, half - margin, radius);
      const coverage = clamp01(0.5 - sd);
      if (coverage <= 0) {
        buf[idx + 3] = 0;
        continue;
      }

      // Deep graphite base with a diagonal lift.
      const diag = clamp01((x / W) * 0.55 + (1 - y / W) * 0.45);
      let col = colorLerp([9, 12, 20], [22, 25, 40], diag * 0.9);

      // Soft colored ambience.
      const dx1 = (x / W - 0.24) / 0.62;
      const dy1 = (y / W - 0.18) / 0.62;
      const g1 = clamp01(1 - Math.sqrt(dx1 * dx1 + dy1 * dy1)) ** 2.2;
      col = colorLerp(col, glowCyan, g1 * 0.22);

      const dx2 = (x / W - 0.82) / 0.58;
      const dy2 = (y / W - 0.88) / 0.58;
      const g2 = clamp01(1 - Math.sqrt(dx2 * dx2 + dy2 * dy2)) ** 2.2;
      col = colorLerp(col, glowViolet, g2 * 0.26);

      // Thin inner edge highlight.
      const innerEdge = clamp01(1 - Math.abs(sd + margin * 0.16) / (W * 0.004));
      col = colorLerp(col, [255, 255, 255], innerEdge * 0.16);

      // Orbit ring.
      const rd = Math.abs(Math.hypot(x - ringCx, y - ringCy) - ringR);
      const ring = clamp01(1 - rd / ringW);
      col = colorLerp(col, [200, 225, 255], ring * 0.13);

      // Cyan aura behind the equalizer.
      const auraDx = (x - half) / (0.34 * W);
      const auraDy = (y - ringCy) / (0.30 * W);
      const aura = clamp01(1 - Math.sqrt(auraDx * auraDx + auraDy * auraDy)) ** 2.4;
      col = colorLerp(col, [110, 240, 235], aura * 0.1);

      // Equalizer bars, vertical gradient cyan -> blue -> violet.
      let barMask = 0;
      let barY = 0;
      for (let i = 0; i < bars.length; i++) {
        const [ox, hhNorm, wNorm] = bars[i];
        const bx = half + ox * W;
        const by = ringCy;
        const hh = hhNorm * W;
        const hw = (wNorm * W) / 2;
        const d = sdRoundBar(x, y, bx, by, hw, hh, hw * 0.9);
        const m = clamp01(0.5 - d);
        if (m > 0) {
          barMask = Math.max(barMask, m);
          barY = (y - (by - hh)) / (2 * hh);
        }
      }
      if (barMask > 0) {
        const t = clamp01(barY);
        let bc = t < 0.5 ? colorLerp(accentA, accentB, t / 0.5) : colorLerp(accentB, accentC, (t - 0.5) / 0.5);
        // Bright top edge.
        const topGlow = smoothstep(0.0, 0.16, t) * (1 - smoothstep(0.16, 0.42, t));
        bc = colorLerp(bc, [240, 255, 255], topGlow * 0.35);
        col = colorLerp(col, bc, barMask);
      }

      // Fine film grain to avoid banding in 8-bit gradients.
      const n = (Math.sin(px * 12.9898 + py * 78.233) * 43758.5453) % 1;
      const grain = (n - Math.floor(n) - 0.5) * 2.2;

      buf[idx] = Math.max(0, Math.min(255, Math.round(col[0] + grain)));
      buf[idx + 1] = Math.max(0, Math.min(255, Math.round(col[1] + grain)));
      buf[idx + 2] = Math.max(0, Math.min(255, Math.round(col[2] + grain)));
      buf[idx + 3] = Math.round(coverage * 255);
    }
  }
  return buf;
}

function downsample(src, srcSize, dstSize) {
  if (dstSize === srcSize) return src;
  const out = Buffer.alloc(dstSize * dstSize * 4);
  const scale = srcSize / dstSize;
  for (let y = 0; y < dstSize; y++) {
    for (let x = 0; x < dstSize; x++) {
      let r = 0,
        g = 0,
        b = 0,
        a = 0,
        n = 0;
      const y0 = Math.floor(y * scale);
      const y1 = Math.max(y0 + 1, Math.floor((y + 1) * scale));
      const x0 = Math.floor(x * scale);
      const x1 = Math.max(x0 + 1, Math.floor((x + 1) * scale));
      for (let sy = y0; sy < y1; sy++) {
        for (let sx = x0; sx < x1; sx++) {
          const i = (sy * srcSize + sx) * 4;
          const alpha = src[i + 3] / 255;
          // Premultiplied averaging avoids dark fringes on transparent corners.
          r += src[i] * alpha;
          g += src[i + 1] * alpha;
          b += src[i + 2] * alpha;
          a += src[i + 3];
          n++;
        }
      }
      const o = (y * dstSize + x) * 4;
      const aAvg = a / n;
      const aNorm = aAvg > 0 ? aAvg / 255 : 0;
      out[o] = Math.round(r / n / (aNorm || 1));
      out[o + 1] = Math.round(g / n / (aNorm || 1));
      out[o + 2] = Math.round(b / n / (aNorm || 1));
      out[o + 3] = Math.round(aAvg);
    }
  }
  return out;
}

// --- PNG encoder -----------------------------------------------------------
const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crc]);
}
function encodePng(rgba, size) {
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    rgba.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// --- ICO / ICNS wrappers ---------------------------------------------------
function encodeIco(pngs) {
  const entries = pngs.map(({ size, data }) => ({ size, data }));
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  const dir = Buffer.alloc(entries.length * 16);
  let offset = 6 + dir.length;
  entries.forEach((e, i) => {
    const o = i * 16;
    dir[o] = e.size >= 256 ? 0 : e.size;
    dir[o + 1] = e.size >= 256 ? 0 : e.size;
    dir[o + 2] = 0;
    dir[o + 3] = 0;
    dir.writeUInt16LE(1, o + 4);
    dir.writeUInt16LE(32, o + 6);
    dir.writeUInt32LE(e.data.length, o + 8);
    dir.writeUInt32LE(offset, o + 12);
    offset += e.data.length;
  });
  return Buffer.concat([header, dir, ...entries.map((e) => e.data)]);
}

function encodeIcns(entries) {
  const parts = entries.map(({ type, data }) => {
    const head = Buffer.alloc(8);
    head.write(type, 0, 4, "ascii");
    head.writeUInt32BE(8 + data.length, 4);
    return Buffer.concat([head, data]);
  });
  const body = Buffer.concat(parts);
  const head = Buffer.alloc(8);
  head.write("icns", 0, 4, "ascii");
  head.writeUInt32BE(8 + body.length, 4);
  return Buffer.concat([head, body]);
}

// --- Build -----------------------------------------------------------------
console.log("Rendering icon master…");
const master = downsample(renderMaster(), W, MASTER);
const sizes = [16, 32, 48, 64, 128, 256, 512, 1024];
const outputs = new Map();
for (const size of sizes) {
  console.log(`  ${size}x${size}`);
  const rgba = downsample(master, MASTER, size);
  outputs.set(size, encodePng(rgba, size));
}

const files = {
  "32x32.png": outputs.get(32),
  "128x128.png": outputs.get(128),
  "128x128@2x.png": outputs.get(256),
  "icon.png": outputs.get(512),
  "icon.ico": encodeIco(
    [16, 32, 48, 64, 128, 256].map((size) => ({ size, data: outputs.get(size) }))
  ),
  "icon.icns": encodeIcns([
    { type: "ic07", data: outputs.get(128) },
    { type: "ic08", data: outputs.get(256) },
    { type: "ic09", data: outputs.get(512) },
    { type: "ic10", data: outputs.get(1024) },
  ]),
};
for (const [name, data] of Object.entries(files)) {
  writeFileSync(join(outDir, name), data);
  console.log(`Wrote icons/${name} (${data.length} bytes)`);
}