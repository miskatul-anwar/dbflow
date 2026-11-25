import { useRef } from "react";
import { FileOutput, FolderOpen, Play, Redo2, RotateCcw, Save, Sparkles, Undo2 } from "lucide-react";
import { Button, IconButton } from "@/components/ui/button";
import { isTauriMobile } from "@/utils/platform";

interface HeadingProps {
  onUndo?: () => void;
  onRedo?: () => void;
  canUndo?: boolean;
  canRedo?: boolean;
  onRun?: () => void;
  onViewOutput?: () => void;
  onSaveState?: () => void;
  onLoadState?: (file: File) => void;
  isRunning?: boolean;
  isOutputReady?: boolean;
  /** Whether the on-device model names new relations (off: names come from the key columns). */
  aiNaming?: boolean;
  onToggleAiNaming?: () => void;
  /** Shows a message in the app's notification area (instead of a browser alert). */
  onNotice?: (message: string) => void;
}

export default function Heading({
  onUndo,
  onRedo,
  canUndo,
  canRedo,
  onRun,
  onViewOutput,
  onSaveState,
  onLoadState,
  isRunning,
  isOutputReady,
  aiNaming = true,
  onToggleAiNaming,
  onNotice,
}: HeadingProps) {
  const loadStateInputRef = useRef<HTMLInputElement>(null);

  const handleLoadStateClick = () => {
    loadStateInputRef.current?.click();
  };

  const handleLoadStateFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const extension = file.name.split(".").pop()?.toLowerCase();
      if (extension === "json") {
        onLoadState?.(file);
      } else {
        onNotice?.(`${file.name} is not a saved workflow. Choose a .json file saved with Save.`);
      }
    }
    e.target.value = "";
  };

  const handleRefresh = () => {
    window.location.reload();
  };

  const mobile = isTauriMobile();

  return (
    // Three zones: history on the left, the workflow file in the centre, run and results on the right. Files are added in
    // the Files card of the Menu sidebar. Secondary actions are
    // icons with a tooltip; only Run and Output carry a word. On a phone the zones wrap onto two rows.
    <div className="relative z-30 flex h-full w-full flex-wrap items-center justify-center gap-2 px-1 py-1.5 sm:grid sm:grid-cols-[1fr_auto_1fr] sm:gap-3 sm:px-3">
      <div className="flex shrink-0 items-center gap-1 sm:min-w-0">
        <img src="/icon.png" alt="DBFlow" className="mr-2 hidden h-7 w-7 shrink-0 object-contain sm:block" />
        <IconButton label="Undo (Ctrl+Z)" title="Undo" icon={<Undo2 />} onClick={onUndo} disabled={!canUndo || isRunning} />
        <IconButton label="Redo (Ctrl+Y)" title="Redo" icon={<Redo2 />} onClick={onRedo} disabled={!canRedo || isRunning} />
        <IconButton label="Reset workspace" title="Reset" icon={<RotateCcw />} onClick={handleRefresh} disabled={isRunning} />
      </div>

      <div className="flex items-center justify-center gap-1">
        <input ref={loadStateInputRef} type="file" accept={mobile ? undefined : ".json"} className="hidden" onChange={handleLoadStateFileChange} />

        <IconButton label="Open workflow" title="Open" icon={<FolderOpen />} onClick={handleLoadStateClick} disabled={isRunning} />
        <IconButton label="Save workflow" title="Save" icon={<Save />} onClick={onSaveState} disabled={isRunning} />
      </div>

      <div className="flex items-center justify-end gap-1.5">
        {onToggleAiNaming && (
          <IconButton
            label="Name new tables with the on-device AI model"
            title={aiNaming ? "AI names: on" : "AI names: off"}
            icon={<Sparkles className={aiNaming ? "text-gray-900" : "opacity-40"} />}
            onClick={onToggleAiNaming}
            disabled={isRunning}
            aria-pressed={aiNaming}
            className={aiNaming ? "bg-gray-100" : ""}
          />
        )}
        <Button variant="primary" onClick={onRun} loading={isRunning} aria-label="Run the pipeline steps on the canvas">
          {!isRunning && <Play />}
          <span>Run</span>
        </Button>
        <Button onClick={onViewOutput} aria-label="View results" className="relative">
          <FileOutput />
          <span>Output</span>
          {isOutputReady && (
            <span className="absolute -top-1 -right-1 h-2.5 w-2.5 rounded-full border-2 border-white bg-teal-600" aria-label="Results ready" />
          )}
        </Button>
      </div>
    </div>
  );
}
