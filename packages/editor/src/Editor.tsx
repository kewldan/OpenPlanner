import { PlanCanvas } from "./canvas/PlanCanvas";
import styles from "./Editor.module.css";
import { Inspector } from "./inspector/Inspector";
import { CommandPalette } from "./palette/CommandPalette";
import { ShortcutsDialog } from "./palette/ShortcutsDialog";
import { ScanWizard } from "./scan/ScanWizard";
import { Sidebar, Toolbar } from "./shell/Sidebar";
import { StatusBar } from "./shell/StatusBar";
import { useShortcuts } from "./shell/shortcuts";
import { TopBar } from "./shell/TopBar";

/** Каркас редактора: верхняя панель, инструменты, объекты, холст, инспектор, статус. */
export function Editor() {
  useShortcuts();
  return (
    <div className={styles.root}>
      <TopBar />
      <div className={styles.body}>
        <Toolbar />
        <Sidebar />
        <PlanCanvas />
        <Inspector />
      </div>
      <StatusBar />
      <CommandPalette />
      <ShortcutsDialog />
      <ScanWizard />
    </div>
  );
}
