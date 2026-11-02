import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import type { ReactNode, RefObject } from "react";
import styles from "./Dialog.module.css";

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Заголовок. Если `hideHeader`, он остаётся только для скринридера. */
  title: string;
  hideHeader?: boolean;
  width?: number;
  /** Поле, получающее фокус при открытии. */
  initialFocus?: RefObject<HTMLElement | null>;
  children: ReactNode;
}

/** Модальное окно (Base UI Dialog): фокус внутри, Esc и клик по фону закрывают. */
export function Dialog({
  open,
  onOpenChange,
  title,
  hideHeader = false,
  width = 560,
  initialFocus,
  children,
}: DialogProps) {
  return (
    <BaseDialog.Root open={open} onOpenChange={(next) => onOpenChange(next)}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className={styles.backdrop} />
        <BaseDialog.Popup
          className={styles.popup}
          style={{ width }}
          {...(initialFocus ? { initialFocus } : {})}
        >
          {hideHeader ? (
            <BaseDialog.Title className="visually-hidden">{title}</BaseDialog.Title>
          ) : (
            <div className={styles.header}>
              <BaseDialog.Title className={styles.title}>{title}</BaseDialog.Title>
              <BaseDialog.Close className={styles.close} aria-label="Закрыть">
                <svg width="16" height="16" viewBox="0 0 20 20" aria-hidden="true">
                  <path
                    d="M5 5l10 10M15 5L5 15"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.4"
                    strokeLinecap="round"
                  />
                </svg>
              </BaseDialog.Close>
            </div>
          )}
          <div className={styles.body}>{children}</div>
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}
