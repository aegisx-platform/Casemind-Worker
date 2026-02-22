<script lang="ts">
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

  let {
    status,
    connecting,
    onConnect,
    onDisconnect,
    onPause,
    onResume,
  }: {
    status: Status;
    connecting: boolean;
    onConnect: () => void;
    onDisconnect: () => void;
    onPause: () => void;
    onResume: () => void;
  } = $props();

  function formatUptime(secs: number): string {
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  }
</script>

<div class="status-section">
  <div class="status-header">
    <h2>Worker Status</h2>
    <div class="controls">
      {#if !status.connected}
        <button class="primary" onclick={onConnect} disabled={connecting}>
          {connecting ? "Connecting..." : "Connect"}
        </button>
      {:else}
        {#if status.paused}
          <button class="primary" onclick={onResume}>Resume</button>
        {:else}
          <button class="secondary" onclick={onPause}>Pause</button>
        {/if}
        <button class="danger" onclick={onDisconnect}>Disconnect</button>
      {/if}
    </div>
  </div>

  <div class="stats-grid">
    <div class="stat card">
      <div class="stat-label">Status</div>
      <div class="stat-value">
        {#if !status.connected}
          <span class="badge neutral">Disconnected</span>
        {:else if status.paused}
          <span class="badge warning">Paused</span>
        {:else}
          <span class="badge success">Active</span>
        {/if}
      </div>
    </div>

    <div class="stat card">
      <div class="stat-label">Tasks Completed</div>
      <div class="stat-value number">{status.tasks_completed}</div>
    </div>

    <div class="stat card">
      <div class="stat-label">Tasks Failed</div>
      <div class="stat-value number" class:error-text={status.tasks_failed > 0}>
        {status.tasks_failed}
      </div>
    </div>

    <div class="stat card">
      <div class="stat-label">Avg Processing</div>
      <div class="stat-value number">
        {status.avg_processing_ms > 0
          ? `${status.avg_processing_ms.toFixed(0)}ms`
          : "—"}
      </div>
    </div>

    <div class="stat card">
      <div class="stat-label">Uptime</div>
      <div class="stat-value number">
        {status.uptime_secs > 0 ? formatUptime(status.uptime_secs) : "—"}
      </div>
    </div>

    <div class="stat card">
      <div class="stat-label">EXE Status</div>
      <div class="stat-value">
        {#if status.exe_available}
          <span class="badge success">Available</span>
        {:else}
          <span class="badge error">Not Found</span>
        {/if}
      </div>
    </div>
  </div>

  <div class="info-row">
    <span class="info-label">Broker:</span>
    <span class="info-value">{status.broker_url || "Not configured"}</span>
    <span class="info-label">Worker ID:</span>
    <span class="info-value mono">{status.worker_id?.slice(0, 20) || "—"}...</span>
  </div>
</div>

<style>
  .status-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .status-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .status-header h2 {
    font-size: 15px;
    font-weight: 600;
  }

  .controls {
    display: flex;
    gap: 8px;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }

  .stat {
    padding: 12px;
  }

  .stat-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .stat-value {
    font-size: 14px;
    font-weight: 500;
  }

  .stat-value.number {
    font-size: 20px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .error-text {
    color: var(--error);
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
    padding: 8px 12px;
    background: var(--bg-secondary);
    border-radius: 6px;
  }

  .info-label {
    color: var(--text-muted);
  }

  .info-value {
    margin-right: 16px;
  }

  .mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
  }
</style>
