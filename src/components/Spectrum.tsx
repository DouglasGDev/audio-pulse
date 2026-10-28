import { useEffect, useRef } from "react";

import { useAppStore } from "../store/appStore";

interface Palette {
  accent: string;
  accent2: string;
  line: string;
}

function readPalette(): Palette {
  const style = getComputedStyle(document.documentElement);
  return {
    accent: style.getPropertyValue("--accent").trim() || "#22d3ee",
    accent2: style.getPropertyValue("--accent-2").trim() || "#8b5cf6",
    line: style.getPropertyValue("--line").trim() || "rgba(148, 163, 184, 0.16)",
  };
}

function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  radius: number,
) {
  const r = Math.min(radius, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  ctx.lineTo(x + w, y + h - r);
  ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  ctx.lineTo(x + r, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

export function Spectrum() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let width = 0;
    let height = 0;
    let raf = 0;
    let palette = readPalette();
    let gradient: CanvasGradient | null = null;
    const visual = new Float32Array(56);
    const peaks = new Float32Array(56);

    const buildGradient = () => {
      if (height <= 0) return;
      gradient = ctx.createLinearGradient(0, height, 0, 0);
      gradient.addColorStop(0, palette.accent);
      gradient.addColorStop(0.55, palette.accent2);
      gradient.addColorStop(1, palette.accent);
    };

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) return;
      width = rect.width;
      height = rect.height;
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      palette = readPalette();
      buildGradient();
    };

    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();

    const themeObserver = new MutationObserver(() => {
      palette = readPalette();
      buildGradient();
    });
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });

    const draw = () => {
      raf = requestAnimationFrame(draw);
      if (width < 1 || height < 1) return;
      ctx.clearRect(0, 0, width, height);

      const { telemetry, view } = useAppStore.getState();
      const bands = telemetry.bands;
      const running = telemetry.running && !view?.params.bypass;
      const count = Math.max(1, bands.length);
      const padX = 2;
      const top = 12;
      const base = height - 12;

      ctx.lineWidth = 1;
      ctx.strokeStyle = palette.line;
      ctx.globalAlpha = 0.6;
      for (const freq of [100, 1000, 10000]) {
        const x =
          padX +
          (Math.log10(freq / 30) / Math.log10(20000 / 30)) * (width - padX * 2);
        ctx.beginPath();
        ctx.moveTo(x, top);
        ctx.lineTo(x, base);
        ctx.stroke();
      }
      for (const fraction of [0.25, 0.5, 0.75]) {
        const y = top + (base - top) * fraction;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;

      const slot = (width - padX * 2) / count;
      const barWidth = Math.max(1.5, slot * 0.62);

      if (gradient) ctx.fillStyle = gradient;

      for (let index = 0; index < count; index += 1) {
        const target = Math.min(1, Math.max(0, bands[index] ?? 0));
        const current = visual[index] ?? 0;
        visual[index] =
          target > current ? current + (target - current) * 0.6 : current * 0.9;
        const level = visual[index];
        peaks[index] = Math.max(level * 0.94, (peaks[index] ?? 0) - 0.008);

        const x = padX + index * slot + (slot - barWidth) / 2;
        const barHeight = Math.max(1.5, Math.pow(level, 1.1) * (base - top));

        ctx.globalAlpha = running ? 0.92 : 0.22;
        roundedRect(ctx, x, base - barHeight, barWidth, barHeight, 2.5);
        ctx.fill();

        const peakY = base - Math.max(1.5, Math.pow(peaks[index], 1.1) * (base - top));
        ctx.globalAlpha = running ? 0.55 : 0.12;
        roundedRect(ctx, x, peakY - 1, barWidth, 2, 1);
        ctx.fill();

        ctx.globalAlpha = running ? 0.1 : 0.04;
        roundedRect(
          ctx,
          x,
          base + 2,
          barWidth,
          Math.min(14, barHeight * 0.22),
          2,
        );
        ctx.fill();
      }
      ctx.globalAlpha = 1;
    };

    raf = requestAnimationFrame(draw);

    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      themeObserver.disconnect();
    };
  }, []);

  return <canvas ref={canvasRef} className="spectrum-canvas" aria-hidden="true" />;
}