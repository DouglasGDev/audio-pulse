import { translate } from "../lib/i18n";
import { useAppStore, useLanguage } from "../store/appStore";
import { Icon } from "./Icons";

export function ToastHost() {
  const toasts = useAppStore((state) => state.toasts);
  const dismiss = useAppStore((state) => state.dismissToast);
  const language = useLanguage();

  if (toasts.length === 0) return null;

  return (
    <div className="toast-host" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast-${toast.tone}`}>
          <span className="toast-icon">
            <Icon
              name={
                toast.tone === "success"
                  ? "check"
                  : toast.tone === "error"
                    ? "close"
                    : "wave"
              }
              size={16}
            />
          </span>
          <span className="toast-text">{toast.message}</span>
          <button
            type="button"
            className="toast-close"
            onClick={() => dismiss(toast.id)}
            aria-label={translate(language, "common.close")}
          >
            <Icon name="close" size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}