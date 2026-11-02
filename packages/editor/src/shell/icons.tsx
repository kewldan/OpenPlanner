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
  window: "M3 6h14v8H3zM3 9h14M3 11h14",
  arch: "M4 17V9a6 6 0 0 1 12 0v8",
  leader: "M4 16l6-6h7M4 16h0.01",
  calibrate: "M3 15 15 3M3 15l3-1M3 15l1-3M15 3l-3 1M15 3l-1 3M11 15h6M14 12v6",
  image: "M3 4h14v12H3zM3 13l4-4 3 3 2-2 5 5M13 7.5h0.01",
  eye: "M2 10s3-5.5 8-5.5 8 5.5 8 5.5-3 5.5-8 5.5S2 10 2 10zM10 12.2a2.2 2.2 0 1 0 0-4.4 2.2 2.2 0 0 0 0 4.4z",
  eyeOff:
    "M3 3l14 14M8.2 5a8 8 0 0 1 1.8-.5c5 0 8 5.5 8 5.5a14 14 0 0 1-2 2.6M5.4 6.4C3.3 7.9 2 10 2 10s3 5.5 8 5.5a7.6 7.6 0 0 0 3.4-.8",
  lock: "M5 9h10v8H5zM7 9V6.5a3 3 0 0 1 6 0V9",
  unlock: "M5 9h10v8H5zM7 9V6.5a3 3 0 0 1 5.8-1",
  plus: "M10 4v12M4 10h12",
  trash: "M4 6h12M8 6V4h4v2M6 6l1 11h6l1-11",
  chevron: "M7 5l5 5-5 5",
  rooms: "M3 3h14v14H3zM10 3v14",
  demolish: "M3 17h14M5 17V9h10v8M5 9l10 8M15 9 5 17",
  scan: "M3 7V4h3M14 4h3v3M17 13v3h-3M6 16H3v-3M6 10h8M8 7.5h4M8 12.5h4",
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
