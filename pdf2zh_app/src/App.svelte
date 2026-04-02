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
  let progressData: any = $state({ stage: "", overall_progress: 0, stage_progress: 0 });
  let logs: string[] = $state([]);
  let result: any = $state(null);
  let tokenUsage: any = $state(null);
  let stages: any[] = $state([]);

  onMount(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const { listen } = await import("@tauri-apps/api/event");

      await listen("backend-event", (event: any) => {
        const data = event.payload;
        switch (data.type) {
          case "stage_summary":
            stages = data.stages || [];
            break;
          case "progress_start":
          case "progress_update":
          case "progress_end":
            progressData = data;
            break;
          case "finish":
            result = data.translate_result;
            tokenUsage = data.token_usage;
            currentView = "result";
            break;
          case "error":
            if (currentView === "progress") {
              backendError = data.error;
              currentView = "config";
            }
            break;
        }
      });

      await listen("backend-log", (event: any) => {
        logs = [...logs.slice(-199), event.payload as string];
      });

      await listen("config-schema", (event: any) => {
        schema = event.payload;
        currentView = "config";
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
    progressData = { stage: "", overall_progress: 0, stage_progress: 0 };
    result = null;
    tokenUsage = null;
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
    <ConfigView {schema} error={backendError} onTranslate={handleTranslate} />
  {:else if currentView === "progress"}
    <ProgressView {progressData} {stages} {logs} onCancel={handleCancel} />
  {:else if currentView === "result"}
    <ResultView {result} {tokenUsage} onNewTranslation={handleNewTranslation} />
  {/if}
</main>
