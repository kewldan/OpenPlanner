// Иконки интерфейса: обводка 1,4 px, сетка 20×20. Нарисованы для OpenPlanner.

const paths = {
  undo: "M7 5 3.5 8.5 7 12M4 8.5h7.5a4.5 4.5 0 0 1 0 9H9",
  redo: "M13 5l3.5 3.5L13 12M16 8.5H8.5a4.5 4.5 0 0 0 0 9H11",
  fit: "M3 7V3h4M13 3h4v4M17 13v4h-4M7 17H3v-4",
  select: "M5 3l10 6.5-4.5 1L8.5 15z",
  wall: "M3 16V4h14M3 10h7",
  door: "M4 17V4h7v13M11 4a7 7 0 0 1 6 7",
  room: "M3 3h14v14H3zM3 10h6v7M12 3v4",
  ruler: "M3 13 13 3l4 4L7 17zM6 10l1.5 1.5M8.5 7.5 10 9M11 5l1.5 1.5",
} as const;

export type IconName = keyof typeof paths;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}
