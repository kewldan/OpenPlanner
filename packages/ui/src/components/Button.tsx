import type { ButtonHTMLAttributes } from "react";
import styles from "./Button.module.css";

export type ButtonVariant = "ghost" | "outline" | "primary";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: "md" | "lg";
}

export function Button({
  variant = "ghost",
  size = "md",
  className,
  type = "button",
  ...rest
}: ButtonProps) {
  const classes = [
    styles.button,
    variant === "outline" && styles.outline,
    variant === "primary" && styles.primary,
    size === "lg" && styles.large,
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return <button type={type} className={classes} {...rest} />;
}

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Подпись для скринридера и подсказки — у кнопки без текста она обязательна. */
  label: string;
}

export function IconButton({ label, className, type = "button", ...rest }: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={[styles.button, styles.icon, className].filter(Boolean).join(" ")}
      {...rest}
    />
  );
}
