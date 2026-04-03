<script lang="ts">
  import { onMount } from "svelte";
  import ConfigView from "./lib/ConfigView.svelte";
  import ProgressView from "./lib/ProgressView.svelte";
  import ResultView from "./lib/ResultView.svelte";
  import LoadingView from "./lib/LoadingView.svelte";

  type View = "loading" | "config" | "progress" | "result";

  let currentView: View = $state("loading");
  let schema: any = $state(null);
  let backendError: string = $state("");
  let logs: string[] = $state([]);
  let droppedFiles: string[] = $state([]);

  // Multi-file state
  let totalFiles: number = $state(0);
  let fileNames: string[] = $state([]);
  let fileProgress: Record<number, any> = $state({});
  let fileStages: Record<number, any[]> = $state({});
  let fileResults: { result: any; tokenUsage: any; filePath: string }[] = $state([]);
  let finishedCount: number = $state(0);

  onMount(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const { listen } = await import("@tauri-apps/api/event");

      await listen("backend-event", (event: any) => {
        const data = event.payload;
        const fi = data.file_index ?? 0;

        switch (data.type) {
          case "batch_start":
            totalFiles = data.total_files;
            fileNames = data.files || [];
            break;
          case "stage_summary":
            fileStages[fi] = data.stages || [];
            fileStages = { ...fileStages };
            break;
          case "progress_start":
          case "progress_update":
          case "progress_end":
            fileProgress[fi] = data;
            fileProgress = { ...fileProgress };
            break;
          case "finish":
            fileResults = [...fileResults, {
              result: data.translate_result,
              tokenUsage: data.token_usage,
              filePath: data.file_path || "",
            }];
            finishedCount = fileResults.length;
            // All files done?
            if (finishedCount >= totalFiles) {
              currentView = "result";
            }
            break;
          case "error":
            if (currentView === "progress") {
              if (totalFiles <= 1) {
                backendError = data.error;
                currentView = "config";
              } else {
                // For batch: count as finished (with error), continue
                fileResults = [...fileResults, {
                  result: null,
                  tokenUsage: null,
                  filePath: data.file_path || "",
                }];
                finishedCount = fileResults.length;
                if (finishedCount >= totalFiles) {
                  currentView = "result";
                }
              }
            }
            break;
        }
      });

      await listen("backend-log", (event: any) => {
        logs = [...logs.slice(-299), event.payload as string];
      });

      await listen("config-schema", (event: any) => {
        schema = event.payload;
        currentView = "config";
      });

      // Listen for file drop events (drag PDF onto window)
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().onDragDropEvent((event) => {
        if (event.payload.type === "drop" && currentView === "config") {
          const paths = event.payload.paths.filter((p: string) =>
            p.toLowerCase().endsWith(".pdf")
          );
          if (paths.length > 0) {
            droppedFiles = paths;
          }
        }
      });

      await invoke("start_backend");
    } catch (e: any) {
      console.error("Backend init error:", e);
      backendError = e?.toString() || "Failed to start backend";
      currentView = "config";
    }
  });

  function handleTranslate(settings: any, files: string[]) {
    logs = [];
    fileProgress = {};
    fileStages = {};
    fileResults = [];
    finishedCount = 0;
    totalFiles = files.length;
    fileNames = files.map(f => f.split("/").pop() || f);
    backendError = "";
    currentView = "progress";
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("translate", { settings, files });
    });
  }

  function handleCancel() {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("cancel_translate");
    });
    currentView = "config";
  }

  function handleNewTranslation() {
    currentView = "config";
  }
</script>

<main class="h-screen flex flex-col overflow-hidden">
  {#if currentView === "loading"}
    <LoadingView />
  {:else if currentView === "config"}
    <ConfigView {schema} error={backendError} {droppedFiles} onTranslate={handleTranslate} />
  {:else if currentView === "progress"}
    <ProgressView
      {fileProgress} {fileStages} {fileNames} {totalFiles} {finishedCount} {logs}
      onCancel={handleCancel} />
  {:else if currentView === "result"}
    <ResultView results={fileResults} onNewTranslation={handleNewTranslation} />
  {/if}
</main>
