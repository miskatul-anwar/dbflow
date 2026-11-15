import { useRef } from "react";
import { FileOutput, FolderOpen, Play, Redo2, RotateCcw, Save, Sparkles, Undo2, Upload } from "lucide-react";
import { Button, IconButton } from "@/components/ui/button";
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
  const uploadTitle = activeFiles === 0
    ? 'Upload CSV or spreadsheet files to analyse'
    : `${activeFiles} file${activeFiles === 1 ? '' : 's'} loaded. Click to upload different files.`;

  const mobile = isTauriMobile();
  // Labels collapse to icons on narrow windows; the tooltip and aria-label keep the name.
  const label = (text: string) => <span className="hidden min-[1180px]:inline">{text}</span>;

  return (
    // Three zones: history and reset on the left, file actions centred, run and results on the right. On a phone the
    // zones wrap onto two rows instead of squeezing into each other.
    <div className="relative z-30 flex h-full w-full flex-wrap items-center justify-center gap-2 px-1 py-1.5 sm:grid sm:grid-cols-[1fr_auto_1fr] sm:gap-3 sm:px-3">
      <div className="flex shrink-0 items-center gap-2.5 sm:min-w-0">
        {/* Brand: icon only (left out on a phone, where the space is needed) */}
        <img src="/icon.png" alt="DBFlow" title="DBFlow" className="hidden h-9 w-9 shrink-0 object-contain drop-shadow-xs sm:block" />

        <div className="flex items-center gap-1">
          <IconButton label="Undo (Ctrl+Z)" icon={<Undo2 />} onClick={onUndo} disabled={!canUndo || isRunning} />
          <IconButton label="Redo (Ctrl+Y)" icon={<Redo2 />} onClick={onRedo} disabled={!canRedo || isRunning} />
        </div>

        <Button onClick={handleRefresh} disabled={isRunning} title="Reload the app and reset every workspace" aria-label="Reset workspace">
          <RotateCcw />
          {label("Reset")}
        </Button>
      </div>

      <div className="flex items-center justify-center gap-2">
        {/* Android's picker filters by MIME type, and phones label CSV files inconsistently (text/csv,
            text/comma-separated-values, application/octet-stream), so the mobile app offers every file and the
            engine's input checks decide what each one really is. */}
        <input ref={inputRef} type="file" multiple accept={mobile ? undefined : INPUT_ACCEPT} className="hidden" onChange={handleFileChange} />
        <input ref={loadStateInputRef} type="file" accept={mobile ? undefined : ".json"} className="hidden" onChange={handleLoadStateFileChange} />

        <Button onClick={handleLoadStateClick} disabled={isRunning} title="Load a saved workflow (.json)" aria-label="Open workflow">
          <FolderOpen />
          {label("Open")}
        </Button>
        <Button onClick={onSaveState} disabled={isRunning} title="Save the workflow (.json)" aria-label="Save workflow">
          <Save />
          {label("Save")}
        </Button>
        <Button onClick={handleUploadClick} disabled={isRunning} title={uploadTitle} aria-label="Upload Datasets" className="relative">
          <Upload />
          {label("Upload")}
          {activeFiles > 0 && (
            <span className="absolute -top-1.5 -right-1.5 flex h-4 min-w-4 items-center justify-center rounded-full border border-white bg-teal-700 px-1 text-[10px] font-semibold leading-none text-white">
              {activeFiles}
            </span>
          )}
        </Button>
      </div>

      <div className="flex items-center justify-end gap-2">
        {onToggleAiNaming && (
          <Button
            onClick={onToggleAiNaming}
            disabled={isRunning}
            aria-pressed={aiNaming}
            aria-label="Name new tables with the on-device AI model"
            title={
              aiNaming
                ? "AI naming is on: the on-device model suggests names for new tables. Click to build names from the key columns instead (faster)."
                : "AI naming is off: new tables are named from their key columns (faster). Click to let the on-device model suggest names."
            }
            className={aiNaming ? "border-teal-600/40 text-teal-800" : "text-gray-600"}
          >
            <Sparkles className={aiNaming ? "" : "opacity-50"} />
            {label(aiNaming ? "AI names" : "Key names")}
          </Button>
        )}
        <Button
          variant="primary"
          onClick={onRun}
          loading={isRunning}
          title="Run the pipeline steps on the canvas"
          aria-label="Run the pipeline steps on the canvas"
        >
          {!isRunning && <Play />}
          <span>Run</span>
        </Button>
        <Button onClick={onViewOutput} title="View the generated ERD, SQL and reports" aria-label="View results" className="relative">
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
