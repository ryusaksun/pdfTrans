<script lang="ts">
  import { onMount } from "svelte";
  import ConfigView from "./lib/ConfigView.svelte";
  import ProgressView from "./lib/ProgressView.svelte";
  import ResultView from "./lib/ResultView.svelte";
  import LoadingView from "./lib/LoadingView.svelte";
  import type { JobState, JobStatus } from "./lib/types";

  type View = "loading" | "config" | "progress" | "result";

  let currentView: View = $state("loading");
  let schema: any = $state(null);
  let backendError: string = $state("");
  let logs: string[] = $state([]);
  let droppedFiles: string[] = $state([]);

  // Job registry keyed by job_id
  let jobs: Record<string, JobState> = $state({});
  let jobOrder: string[] = $state([]);

  function terminalStatuses(): JobStatus[] {
    return ["done", "error", "cancelled"];
  }

  let activeJobs = $derived(
    jobOrder
      .map((id) => jobs[id])
      .filter((j): j is JobState => !!j && (j.status === "queued" || j.status === "running"))
  );
  let finishedJobCount = $derived(
    jobOrder.filter((id) => jobs[id] && terminalStatuses().includes(jobs[id].status)).length
  );
  let activeJobCount = $derived(activeJobs.length);

  function ensureJob(jobId: string): JobState {
    if (!jobs[jobId]) {
      jobs[jobId] = {
        jobId,
        status: "queued",
        fileNames: [],
        filePaths: [],
        fileProgress: {},
        fileStages: {},
        fileResults: [],
        createdAt: Date.now(),
      };
      jobOrder = [...jobOrder, jobId];
    }
    return jobs[jobId];
  }

  function updateJob(jobId: string, mutate: (j: JobState) => void) {
    const j = jobs[jobId];
    if (!j) return;
    mutate(j);
    jobs = { ...jobs };
  }

  function maybeFinalizeJob(jobId: string) {
    const j = jobs[jobId];
    if (!j) return;
    if (j.status !== "running" && j.status !== "queued") return;
    if (j.fileNames.length === 0) return;
    if (j.fileResults.length >= j.fileNames.length) {
      const anySuccess = j.fileResults.some((r) => r.result);
      updateJob(jobId, (job) => {
        job.status = anySuccess ? "done" : "error";
      });
      // Auto-switch to result view only when the user is currently watching progress
      if (currentView === "progress" && activeJobCount === 0) {
        currentView = "result";
      }
    }
  }

  onMount(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const { listen } = await import("@tauri-apps/api/event");

      await listen("backend-event", (event: any) => {
        const data = event.payload;
        const jobId: string | undefined = data.job_id;
        const fi = data.file_index ?? 0;

        switch (data.type) {
          case "job_enqueued": {
            if (!jobId) return;
            const job = ensureJob(jobId);
            job.fileNames = data.files || [];
            job.filePaths = data.file_paths || [];
            job.status = "queued";
            jobs = { ...jobs };
            break;
          }
          case "job_cancelled": {
            if (!jobId) return;
            updateJob(jobId, (j) => {
              j.status = "cancelled";
            });
            break;
          }
          case "stage_summary": {
            if (!jobId) return;
            updateJob(jobId, (j) => {
              j.fileStages[fi] = data.stages || [];
              if (j.status === "queued") j.status = "running";
            });
            break;
          }
          case "progress_start":
          case "progress_update":
          case "progress_end": {
            if (!jobId) return;
            updateJob(jobId, (j) => {
              j.fileProgress[fi] = data;
              if (j.status === "queued") j.status = "running";
            });
            break;
          }
          case "finish": {
            if (!jobId) return;
            updateJob(jobId, (j) => {
              j.fileResults = [
                ...j.fileResults,
                {
                  result: data.translate_result,
                  tokenUsage: data.token_usage,
                  filePath: data.file_path || "",
                },
              ];
            });
            maybeFinalizeJob(jobId);
            break;
          }
          case "error": {
            // Process-level error (no job_id): sweep active jobs to error
            if (!jobId) {
              const msg = data.error || "Unknown error";
              if (activeJobCount === 0) {
                backendError = msg;
              } else {
                for (const id of jobOrder) {
                  const j = jobs[id];
                  if (j && (j.status === "queued" || j.status === "running")) {
                    updateJob(id, (jj) => {
                      jj.status = "error";
                    });
                  }
                }
              }
              return;
            }
            updateJob(jobId, (j) => {
              j.fileResults = [
                ...j.fileResults,
                {
                  result: null,
                  tokenUsage: null,
                  filePath: data.file_path || "",
                },
              ];
            });
            maybeFinalizeJob(jobId);
            break;
          }
        }
      });

      await listen("backend-log", (event: any) => {
        logs = [...logs.slice(-299), event.payload as string];
      });

      await listen("config-schema", (event: any) => {
        schema = event.payload;
        currentView = "config";
      });

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
    backendError = "";
    const jobId = crypto.randomUUID();
    // Optimistically seed the job so the badge appears immediately,
    // even before job_enqueued round-trips from Python.
    ensureJob(jobId);
    updateJob(jobId, (j) => {
      j.fileNames = files.map((f) => f.split("/").pop() || f);
      j.filePaths = files;
    });
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("translate", { jobId, settings, files });
    });
  }

  function handleCancelJob(jobId: string) {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("cancel_translate", { jobId });
    });
  }

  function handleCancelAll() {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("cancel_translate", { jobId: null });
    });
  }

  function handleOpenProgress() {
    currentView = "progress";
  }

  function handleBackToConfig() {
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
    <ConfigView
      {schema}
      error={backendError}
      {droppedFiles}
      activeCount={activeJobCount}
      finishedCount={finishedJobCount}
      onTranslate={handleTranslate}
      onOpenProgress={handleOpenProgress}
    />
  {:else if currentView === "progress"}
    <ProgressView
      jobs={jobOrder.map((id) => jobs[id]).filter((j): j is JobState => !!j)}
      {logs}
      onBack={handleBackToConfig}
      onCancelJob={handleCancelJob}
      onCancelAll={handleCancelAll}
    />
  {:else if currentView === "result"}
    <ResultView
      jobs={jobOrder.map((id) => jobs[id]).filter((j): j is JobState => !!j)}
      onNewTranslation={handleNewTranslation}
    />
  {/if}
</main>
