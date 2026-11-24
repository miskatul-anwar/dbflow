import { useRef } from "react";
import { FileOutput, FolderOpen, Play, Redo2, RotateCcw, Save, Sparkles, Undo2, Upload } from "lucide-react";
import { Button, IconButton } from "@/components/ui/button";
import { WithTooltip } from "@/components/ui/tooltip";
import { INPUT_ACCEPT } from "@/services/processors/spreadsheetProcessor";
import { isTauriMobile } from "@/utils/platform";

interface HeadingProps {
  onUndo?: () => void;
  onRedo?: () => void;
  canUndo?: boolean;
  canRedo?: boolean;
  onRun?: () => void;
  onFileUpload?: (files: File[]) => void;
  /** Number of data files the user has uploaded. */
  uploadedFileCount?: number;
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
  onFileUpload,
  uploadedFileCount = 0,
  onViewOutput,
  onSaveState,
  onLoadState,
  isRunning,
  isOutputReady,
  aiNaming = true,
  onToggleAiNaming,
  onNotice,
}: HeadingProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const loadStateInputRef = useRef<HTMLInputElement>(null);

  const handleUploadClick = () => {
    inputRef.current?.click();
  };

  const handleLoadStateClick = () => {
    loadStateInputRef.current?.click();
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files ? Array.from(e.target.files) : [];
    if (files.length > 0) {
      onFileUpload?.(files);
    }
    e.target.value = "";
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

  const activeFiles = uploadedFileCount;
  const mobile = isTauriMobile();

  return (
    // Three zones: history on the left, files in the centre, run and results on the right. Secondary actions are
    // icons with a tooltip; only Run and Output carry a word. On a phone the zones wrap onto two rows.
    <div className="relative z-30 flex h-full w-full flex-wrap items-center justify-center gap-2 px-1 py-1.5 sm:grid sm:grid-cols-[1fr_auto_1fr] sm:gap-3 sm:px-3">
      <div className="flex shrink-0 items-center gap-1 sm:min-w-0">
        <img src="/icon.png" alt="DBFlow" className="mr-2 hidden h-7 w-7 shrink-0 object-contain sm:block" />
        <IconButton label="Undo (Ctrl+Z)" title="Undo" icon={<Undo2 />} onClick={onUndo} disabled={!canUndo || isRunning} />
        <IconButton label="Redo (Ctrl+Y)" title="Redo" icon={<Redo2 />} onClick={onRedo} disabled={!canRedo || isRunning} />
        <IconButton label="Reset workspace" title="Reset" icon={<RotateCcw />} onClick={handleRefresh} disabled={isRunning} />
      </div>

      <div className="flex items-center justify-center gap-1">
        {/* Android's picker filters by MIME type, and phones label CSV files inconsistently (text/csv,
            text/comma-separated-values, application/octet-stream), so the mobile app offers every file and the
            engine's input checks decide what each one really is. */}
        <input ref={inputRef} type="file" multiple accept={mobile ? undefined : INPUT_ACCEPT} className="hidden" onChange={handleFileChange} />
        <input ref={loadStateInputRef} type="file" accept={mobile ? undefined : ".json"} className="hidden" onChange={handleLoadStateFileChange} />

        <IconButton label="Open workflow" title="Open" icon={<FolderOpen />} onClick={handleLoadStateClick} disabled={isRunning} />
        <IconButton label="Save workflow" title="Save" icon={<Save />} onClick={onSaveState} disabled={isRunning} />
        <WithTooltip content={activeFiles === 0 ? "Add files" : `${activeFiles} file${activeFiles === 1 ? "" : "s"}`}>
          <Button variant="ghost" size="icon" onClick={handleUploadClick} disabled={isRunning} aria-label="Upload Datasets" className="relative">
            <Upload />
            {activeFiles > 0 && (
              <span className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-gray-900 px-1 text-[10px] font-semibold leading-none text-white nf-btn-primary">
                {activeFiles}
              </span>
            )}
          </Button>
        </WithTooltip>
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
