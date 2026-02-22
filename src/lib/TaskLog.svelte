<script lang="ts">
  type LogEntry = {
    request_id: string;
    case_count: number;
    drg_codes: string[];
    processing_ms: number;
    status: string;
    completed_at: string;
    request_data?: any;
    response_data?: any;
  };

  let { entries }: { entries: LogEntry[] } = $props();
  let expandedId = $state<string | null>(null);

  function formatTime(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleTimeString("th-TH", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    } catch {
      return iso;
    }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }
</script>

<div class="log-section">
  <h2>Task Log</h2>

  {#if entries.length === 0}
    <div class="empty card">
      <p>No tasks processed yet</p>
      <p class="hint">Tasks will appear here when workers receive DRG grouping requests</p>
    </div>
  {:else}
    <div class="log-table card">
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Request ID</th>
            <th>Cases</th>
            <th>DRG</th>
            <th>Time (ms)</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as entry}
            <tr class="clickable" onclick={() => toggleExpand(entry.request_id)}>
              <td class="mono">{formatTime(entry.completed_at)}</td>
              <td class="mono">{entry.request_id.slice(0, 12)}...</td>
              <td>{entry.case_count}</td>
              <td class="mono">
                {#if entry.drg_codes.length > 0}
                  {entry.drg_codes.slice(0, 3).join(", ")}
                  {#if entry.drg_codes.length > 3}
                    <span class="more">+{entry.drg_codes.length - 3}</span>
                  {/if}
                {:else}
                  —
                {/if}
              </td>
              <td class="mono">{entry.processing_ms}</td>
              <td>
                {#if entry.status === "success"}
                  <span class="badge success">OK</span>
                {:else}
                  <span class="badge error">ERR</span>
                {/if}
              </td>
            </tr>
            {#if expandedId === entry.request_id}
              <tr class="detail-row">
                <td colspan="6">
                  <div class="detail-panels">
                    <div class="detail-panel">
                      <h4>Request</h4>
                      <pre>{JSON.stringify(entry.request_data, null, 2)}</pre>
                    </div>
                    <div class="detail-panel">
                      <h4>Response</h4>
                      <pre>{JSON.stringify(entry.response_data, null, 2)}</pre>
                    </div>
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .log-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .log-section h2 {
    font-size: 15px;
    font-weight: 600;
  }

  .empty {
    text-align: center;
    padding: 32px;
    color: var(--text-secondary);
  }

  .empty .hint {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .log-table {
    overflow-x: auto;
    padding: 0;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th {
    text-align: left;
    padding: 10px 12px;
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid var(--border);
    font-weight: 500;
  }

  td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }

  tr:last-child td {
    border-bottom: none;
  }

  .clickable {
    cursor: pointer;
  }

  .clickable:hover {
    background: rgba(99, 102, 241, 0.06);
  }

  .detail-row td {
    padding: 0;
    background: rgba(0, 0, 0, 0.15);
  }

  .detail-panels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 12px;
  }

  .detail-panel {
    overflow: hidden;
  }

  .detail-panel h4 {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 6px;
  }

  .detail-panel pre {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    line-height: 1.5;
    background: rgba(0, 0, 0, 0.2);
    padding: 10px;
    border-radius: 6px;
    overflow-x: auto;
    max-height: 300px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    color: var(--text-secondary);
  }

  .mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
  }

  .more {
    color: var(--text-muted);
    font-size: 11px;
  }
</style>
