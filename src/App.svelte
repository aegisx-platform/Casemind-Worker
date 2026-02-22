<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import WorkerStatus from "./lib/WorkerStatus.svelte";
  import TaskLog from "./lib/TaskLog.svelte";
  import Settings from "./lib/Settings.svelte";
  import "./app.css";

  type Status = {
    connected: boolean;
    paused: boolean;
    tasks_completed: number;
    tasks_failed: number;
    avg_processing_ms: number;
    current_queue: number;
    exe_available: boolean;
    broker_url: string;
    worker_id: string;
    uptime_secs: number;
  };

  type LogEntry = {
    request_id: string;
    case_count: number;
    drg_codes: string[];
    processing_ms: number;
    status: string;
    completed_at: string;
  };

  let activeTab = $state<"dashboard" | "settings">("dashboard");
  let status = $state<Status>({
    connected: false,
    paused: false,
    tasks_completed: 0,
    tasks_failed: 0,
    avg_processing_ms: 0,
    current_queue: 0,
    exe_available: false,
    broker_url: "",
    worker_id: "",
    uptime_secs: 0,
  });
  let taskLog = $state<LogEntry[]>([]);
  let connecting = $state(false);
  let error = $state("");

  async function refreshStatus() {
    try {
      status = await invoke<Status>("get_status");
      taskLog = await invoke<LogEntry[]>("get_task_log");
    } catch (e: any) {
      console.error("Failed to get status:", e);
    }
  }

  async function handleConnect() {
    connecting = true;
    error = "";
    try {
      await invoke("connect_worker");
      await refreshStatus();
    } catch (e: any) {
      error = e.toString();
    } finally {
      connecting = false;
    }
  }

  async function handleDisconnect() {
    try {
      await invoke("disconnect_worker");
      await refreshStatus();
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handlePause() {
    await invoke("pause_worker");
    await refreshStatus();
  }

  async function handleResume() {
    await invoke("resume_worker");
    await refreshStatus();
  }

  onMount(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 3000);
    return () => clearInterval(interval);
  });
</script>

<div class="app-container">
  <header class="app-header">
    <div class="header-left">
      <h1>CaseMind Worker</h1>
      <span class="version">v0.1.0</span>
    </div>
    <nav class="tabs">
      <button
        class="tab"
        class:active={activeTab === "dashboard"}
        onclick={() => (activeTab = "dashboard")}
      >
        Dashboard
      </button>
      <button
        class="tab"
        class:active={activeTab === "settings"}
        onclick={() => (activeTab = "settings")}
      >
        Settings
      </button>
    </nav>
    <div class="header-right">
      {#if status.connected}
        <span class="badge success">Connected</span>
      {:else}
        <span class="badge neutral">Disconnected</span>
      {/if}
    </div>
  </header>

  <main class="app-main">
    {#if error}
      <div class="error-banner">
        <span>{error}</span>
        <button class="dismiss" onclick={() => (error = "")}>×</button>
      </div>
    {/if}

    {#if activeTab === "dashboard"}
      <div class="dashboard">
        <WorkerStatus
          {status}
          {connecting}
          onConnect={handleConnect}
          onDisconnect={handleDisconnect}
          onPause={handlePause}
          onResume={handleResume}
        />
        <TaskLog entries={taskLog} />
      </div>
    {:else}
      <Settings onSaved={refreshStatus} />
    {/if}
  </main>
</div>

<style>
  .app-container {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  .app-header {
    display: flex;
    align-items: center;
    padding: 12px 20px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    gap: 16px;
    -webkit-app-region: drag;
  }

  .header-left {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .header-left h1 {
    font-size: 16px;
    font-weight: 600;
  }

  .version {
    font-size: 11px;
    color: var(--text-muted);
  }

  .tabs {
    display: flex;
    gap: 2px;
    margin-left: auto;
    -webkit-app-region: no-drag;
  }

  .tab {
    background: none;
    color: var(--text-secondary);
    padding: 6px 14px;
    border-radius: 6px;
    font-size: 13px;
  }

  .tab:hover {
    color: var(--text-primary);
    background: var(--bg-input);
  }

  .tab.active {
    color: var(--text-primary);
    background: var(--bg-card);
  }

  .header-right {
    -webkit-app-region: no-drag;
  }

  .app-main {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .dashboard {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .error-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(239, 68, 68, 0.12);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 8px;
    padding: 10px 14px;
    color: var(--error);
    font-size: 13px;
    margin-bottom: 8px;
  }

  .dismiss {
    background: none;
    color: var(--error);
    font-size: 18px;
    padding: 0 4px;
    line-height: 1;
  }
</style>
