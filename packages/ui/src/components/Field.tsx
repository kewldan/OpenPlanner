import { type InputHTMLAttributes, type Ref, useId } from "react";
import styles from "./Field.module.css";

export interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "id"> {
  label: string;
  /** Единица измерения справа от значения, например «мм». */
  unit?: string;
  /** Моноширинный шрифт для чисел. */
  numeric?: boolean;
  ref?: Ref<HTMLInputElement>;
}

/** Строка инспектора: подпись слева, поле справа. */
export function Field({ label, unit, numeric = false, className, ref, ...rest }: FieldProps) {
  const id = useId();
  return (
    <div className={styles.row}>
      <label htmlFor={id} className={styles.label}>
        {label}
      </label>
      <div className={styles.box}>
        <input
          ref={ref}
          id={id}
          className={[styles.input, numeric && styles.mono, className].filter(Boolean).join(" ")}
          {...rest}
        />
        {unit && <span className={styles.unit}>{unit}</span>}
      </div>
    </div>
  );
}
