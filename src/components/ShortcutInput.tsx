import { useState, type KeyboardEvent as ReactKeyboardEvent } from "react";

import { formatAccelerator } from "../lib/format";
import { Icon } from "./Icons";

interface ShortcutInputProps {
  value: string;
  onChange: (accelerator: string) => void;
  hint: string;
  invalidHint: string;
}

const namedKeys: Record<string, string> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  " ": "Space",
  Enter: "Enter",
  Backspace: "Backspace",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
  Tab: "Tab",
  PrintScreen: "PrintScreen",
};

function keyName(event: KeyboardEvent): string | null {
  if (namedKeys[event.key]) return namedKeys[event.key];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(event.key)) return event.key;
  if (event.key.length === 1) {
    if (event.key === "+") return "Plus";
    if (event.key === "-") return "Minus";
    return event.key.toUpperCase();
  }
  return null;
}

export function ShortcutInput({ value, onChange, hint, invalidHint }: ShortcutInputProps) {
  const [recording, setRecording] = useState(false);
  const [invalid, setInvalid] = useState(false);

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      setRecording(false);
      setInvalid(false);
      return;
    }
    const key = keyName(event.nativeEvent);
    if (!key) return;

    const mac = /mac/i.test(navigator.platform);
    const parts: string[] = [];
    if (mac ? event.metaKey : event.ctrlKey) parts.push("CmdOrCtrl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");
    if (!mac && event.metaKey) parts.push("Super");

    if (parts.length === 0) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    setRecording(false);
    onChange([...parts, key].join("+"));
  };

  return (
    <button
      type="button"
      className={`shortcut-input ${recording ? "is-recording" : ""} ${invalid ? "is-invalid" : ""}`}
      onClick={() => {
        setRecording(true);
        setInvalid(false);
      }}
      onKeyDown={handleKeyDown}
      onBlur={() => {
        setRecording(false);
        setInvalid(false);
      }}
    >
      <Icon name="keyboard" size={14} />
      <span>
        {recording
          ? invalid
            ? invalidHint
            : hint
          : value
            ? formatAccelerator(value)
            : "—"}
      </span>
    </button>
  );
}