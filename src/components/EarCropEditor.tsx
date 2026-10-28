import {
  forwardRef,
  useEffect,
  useId,
  useImperativeHandle,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

import { Slider } from "./Controls";
import { translate, type MessageKey } from "../lib/i18n";
import { useLanguage } from "../store/appStore";

const SIZE = 512;
const GUIDE_RX = 196;
const GUIDE_RY = 240;

export interface EarCropEditorHandle {
  exportCrop: () => string | null;
}

interface EarCropEditorProps {
  src: string;
  ariaLabel: string;
  onReady?: (ready: boolean) => void;
  onError?: () => void;
}

interface Offset {
  x: number;
  y: number;
}

function paint(
  context: CanvasRenderingContext2D,
  image: CanvasImageSource | null,
  baseScaleRef: { current: number },
  zoom: number,
  rotation: number,
  offset: Offset,
) {
  context.fillStyle = "#070b13";
  context.fillRect(0, 0, SIZE, SIZE);
  if (!image) return;
  const source = image as HTMLImageElement;
  const scale = baseScaleRef.current * zoom;
  context.save();
  context.translate(SIZE / 2 + offset.x, SIZE / 2 + offset.y);
  context.rotate((rotation * Math.PI) / 180);
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  context.drawImage(
    source,
    (-source.naturalWidth * scale) / 2,
    (-source.naturalHeight * scale) / 2,
    source.naturalWidth * scale,
    source.naturalHeight * scale,
  );
  context.restore();
}

export const EarCropEditor = forwardRef<EarCropEditorHandle, EarCropEditorProps>(
  function EarCropEditor({ src, ariaLabel, onReady, onError }, ref) {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const baseScaleRef = useRef(1);
    const dragRef = useRef<{
      pointerId: number;
      startX: number;
      startY: number;
      originX: number;
      originY: number;
      scale: number;
    } | null>(null);
    const [image, setImage] = useState<HTMLImageElement | null>(null);
    const [zoom, setZoom] = useState(1);
    const [rotation, setRotation] = useState(0);
    const [offset, setOffset] = useState<Offset>({ x: 0, y: 0 });
    const language = useLanguage();
    const maskId = useId().replace(/[^a-zA-Z0-9]/g, "");
    const t = (key: MessageKey) => translate(language, key);

    useEffect(() => {
      let cancelled = false;
      setImage(null);
      setZoom(1);
      setRotation(0);
      setOffset({ x: 0, y: 0 });
      onReady?.(false);
      if (!src) return;
      const loaded = new Image();
      loaded.onload = () => {
        if (cancelled) return;
        baseScaleRef.current = Math.max(
          SIZE / loaded.naturalWidth,
          SIZE / loaded.naturalHeight,
        );
        setImage(loaded);
        onReady?.(true);
      };
      loaded.onerror = () => {
        if (!cancelled) onError?.();
      };
      loaded.src = src;
      return () => {
        cancelled = true;
      };
    }, [src, onReady, onError]);

    useEffect(() => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const pixels = Math.round(SIZE * dpr);
      if (canvas.width !== pixels || canvas.height !== pixels) {
        canvas.width = pixels;
        canvas.height = pixels;
      }
      const context = canvas.getContext("2d");
      if (!context) return;
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
      paint(context, image, baseScaleRef, zoom, rotation, offset);
    }, [image, zoom, rotation, offset]);

    useImperativeHandle(
      ref,
      () => ({
        exportCrop() {
          if (!image) return null;
          const canvas = document.createElement("canvas");
          canvas.width = SIZE;
          canvas.height = SIZE;
          const context = canvas.getContext("2d");
          if (!context) return null;
          paint(context, image, baseScaleRef, zoom, rotation, offset);
          return canvas.toDataURL("image/png");
        },
      }),
      [image, zoom, rotation, offset],
    );

    const onPointerDown = (event: ReactPointerEvent<HTMLCanvasElement>) => {
      if (event.button !== 0) return;
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      dragRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        originX: offset.x,
        originY: offset.y,
        scale: rect.width > 0 ? SIZE / rect.width : 1,
      };
      canvas.setPointerCapture(event.pointerId);
    };

    const onPointerMove = (event: ReactPointerEvent<HTMLCanvasElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      setOffset({
        x: drag.originX + (event.clientX - drag.startX) * drag.scale,
        y: drag.originY + (event.clientY - drag.startY) * drag.scale,
      });
    };

    const onPointerUp = (event: ReactPointerEvent<HTMLCanvasElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) return;
      dragRef.current = null;
      const canvas = canvasRef.current;
      if (canvas?.hasPointerCapture(event.pointerId)) {
        canvas.releasePointerCapture(event.pointerId);
      }
    };

    return (
      <div className="ear-editor">
        <div className="ear-photo">
          <canvas
            ref={canvasRef}
            aria-label={ariaLabel}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
          />
          <svg className="ear-guide" viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden="true">
            <defs>
              <mask id={maskId}>
                <rect width={SIZE} height={SIZE} fill="#fff" />
                <ellipse cx={256} cy={256} rx={GUIDE_RX} ry={GUIDE_RY} fill="#000" />
              </mask>
            </defs>
            <rect
              width={SIZE}
              height={SIZE}
              fill="rgba(2, 5, 11, 0.55)"
              mask={`url(#${maskId})`}
            />
            <ellipse
              cx={256}
              cy={256}
              rx={GUIDE_RX}
              ry={GUIDE_RY}
              fill="none"
              stroke="var(--accent)"
              strokeWidth="2"
              opacity="0.85"
            />
            <ellipse
              cx={256}
              cy={256}
              rx={GUIDE_RX * 0.78}
              ry={GUIDE_RY * 0.78}
              fill="none"
              stroke="var(--accent)"
              strokeWidth="1"
              strokeDasharray="4 6"
              opacity="0.28"
            />
          </svg>
        </div>

        <div className="ear-controls">
          <Slider
            label={t("ear.zoom")}
            value={zoom}
            min={0.5}
            max={3}
            step={0.01}
            onChange={setZoom}
            format={(value) => `${Math.round(value * 100)}%`}
          />
          <Slider
            label={t("ear.rotate")}
            value={rotation}
            min={-180}
            max={180}
            step={1}
            onChange={setRotation}
            format={(value) => `${Math.round(value)}°`}
            accent="violet"
          />
          <button
            type="button"
            className="btn ghost small"
            onClick={() => {
              setZoom(1);
              setRotation(0);
              setOffset({ x: 0, y: 0 });
            }}
          >
            {t("ear.resetFrame")}
          </button>
        </div>
      </div>
    );
  },
);
