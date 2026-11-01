import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import styles from "./Segmented.module.css";

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SegmentedProps<T extends string> {
  label: string;
  value: T;
  options: readonly SegmentedOption<T>[];
  onChange: (value: T) => void;
}

/** Переключатель «один из нескольких» (Base UI ToggleGroup). */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
}: SegmentedProps<T>) {
  return (
    <ToggleGroup
      aria-label={label}
      className={styles.root}
      value={[value]}
      onValueChange={(next) => {
        // Повторное нажатие на выбранный пункт не снимает выбор.
        const picked = next[0];
        if (picked !== undefined && picked !== value) onChange(picked as T);
      }}
    >
      {options.map((option) => (
        <Toggle
          key={option.value}
          value={option.value}
          disabled={option.disabled ?? false}
          className={styles.item}
        >
          {option.label}
        </Toggle>
      ))}
    </ToggleGroup>
  );
}
