<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  type Config = {
    broker_host: string;
    broker_port: number;
    topic_pending: string;
    topic_results: string;
    topic_health: string;
    topic_register: string;
    client_id: string;
    version: string;
    [key: string]: any;
  };

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

  type Example = {
    name: string;
    description: string;
    cases: any[];
  };

  let { connected }: { connected: boolean } = $props();

  const EXAMPLES: Example[] = [
    {
      name: "Medical - Heart Failure",
      description: "Heart failure (I500) with DM type 2 and Hypertension, male 65y, LOS 5 days",
      cases: [
        {
          pdx: "I500",
          sdx: ["E119", "I10"],
          procedures: [],
          age: 65,
          age_in_days: null,
          sex: "1",
          discharge_type: 1,
          los: 5,
          admission_weight: null,
        },
      ],
    },
    {
      name: "Surgical - Cholecystectomy",
      description: "Gallstone (K802) with laparoscopic cholecystectomy (5123), female 45y, LOS 4 days",
      cases: [
        {
          pdx: "K802",
          sdx: ["E119"],
          procedures: ["5123"],
          age: 45,
          age_in_days: null,
          sex: "2",
          discharge_type: 1,
          los: 4,
          admission_weight: null,
        },
      ],
    },
    {
      name: "Newborn - Normal Delivery",
      description: "Single liveborn (Z380), age 0y / 3 days, weight 3.2 kg, female",
      cases: [
        {
          pdx: "Z380",
          sdx: [],
          procedures: [],
          age: 0,
          age_in_days: 3,
          sex: "2",
          discharge_type: 1,
          los: 3,
          admission_weight: 3.2,
        },
      ],
    },
  ];

  // State
  let selectedExample = $state(0);
  let jsonBody = $state(JSON.stringify(EXAMPLES[0].cases, null, 2));
  let jsonError = $state("");

  let sending = $state(false);
  let polling = $state(false);
  let testRequestId = $state("");
  let testResult = $state<LogEntry | null>(null);
  let testError = $state("");
  let config = $state<Config | null>(null);

  function selectExample(index: number) {
    selectedExample = index;
    jsonBody = JSON.stringify(EXAMPLES[index].cases, null, 2);
    jsonError = "";
  }

  function validateJson(): any[] | null {
    try {
      const parsed = JSON.parse(jsonBody);
      if (!Array.isArray(parsed) || parsed.length === 0) {
        jsonError = "JSON must be a non-empty array of cases";
        return null;
      }
      // Basic validation
      for (const c of parsed) {
        if (!c.pdx || typeof c.pdx !== "string") {
          jsonError = "Each case must have a 'pdx' string field";
          return null;
        }
      }
      jsonError = "";
      return parsed;
    } catch (e: any) {
      jsonError = `Invalid JSON: ${e.message}`;
      return null;
    }
  }

  function handleJsonInput() {
    // Clear error on edit, validate on send
    if (jsonError) {
      validateJson();
    }
  }

  async function loadConfig() {
    try {
      config = await invoke<Config>("get_config");
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }

  async function sendTest() {
    if (!connected) {
      testError = "Worker not connected. Please connect from the Dashboard first.";
      return;
    }

    const cases = validateJson();
    if (!cases) return;

    sending = true;
    testError = "";
    testResult = null;
    testRequestId = "";

    try {
      testRequestId = await invoke<string>("send_test_task", { cases });

      // Poll task log for result
      polling = true;
      let attempts = 0;
      const maxAttempts = 30;
      const pollInterval = setInterval(async () => {
        attempts++;
        try {
          const log = await invoke<LogEntry[]>("get_task_log");
          const entry = log.find((e) => e.request_id === testRequestId);
          if (entry) {
            testResult = entry;
            clearInterval(pollInterval);
            polling = false;
          } else if (attempts >= maxAttempts) {
            testError = "Timeout: no result received within 30 seconds";
            clearInterval(pollInterval);
            polling = false;
          }
        } catch (e) {
          console.error("Poll error:", e);
        }
      }, 1000);
    } catch (e: any) {
      testError = e.toString();
    } finally {
      sending = false;
    }
  }

  onMount(loadConfig);
</script>

<div class="test-page">
  <h2>MQTT Test</h2>

  <!-- Topic Info -->
  {#if config}
    <div class="card topics-card">
      <h3>Active MQTT Topics</h3>
      <div class="topic-grid">
        <div class="topic-row">
          <span class="topic-dir sub">SUB</span>
          <span class="topic-label">Task Pending</span>
          <code class="topic-value">{config.topic_pending}</code>
        </div>
        <div class="topic-row">
          <span class="topic-dir pub">PUB</span>
          <span class="topic-label">Task Results</span>
          <code class="topic-value"
            >{config.topic_results}/<span class="topic-dynamic"
              >{"{request_id}"}</span
            ></code
          >
        </div>
        <div class="topic-row">
          <span class="topic-dir pub">PUB</span>
          <span class="topic-label">Worker Health</span>
          <code class="topic-value"
            >{config.topic_health}/<span class="topic-dynamic"
              >{"{worker_id}"}</span
            ></code
          >
        </div>
        <div class="topic-row">
          <span class="topic-dir pub">PUB</span>
          <span class="topic-label">Worker Register</span>
          <code class="topic-value"
            >{config.topic_register}/<span class="topic-dynamic"
              >{"{worker_id}"}</span
            ></code
          >
        </div>
      </div>
      <div class="topic-info">
        Broker: <code>{config.broker_host}:{config.broker_port}</code> &middot;
        Worker: <code>{config.client_id?.slice(0, 24)}...</code>
      </div>
    </div>
  {/if}

  <!-- Examples -->
  <div class="card">
    <h3>Test Examples</h3>
    <div class="examples-grid">
      {#each EXAMPLES as ex, i}
        <button
          class="example-card"
          class:selected={selectedExample === i}
          onclick={() => selectExample(i)}
        >
          <span class="example-name">{ex.name}</span>
          <span class="example-desc">{ex.description}</span>
        </button>
      {/each}
    </div>
  </div>

  <!-- JSON Body Editor -->
  <div class="card">
    <div class="editor-header">
      <h3>Request Body (cases)</h3>
      {#if !connected}
        <span class="badge warning">Disconnected</span>
      {/if}
    </div>

    <div class="editor-hint">
      Edit the JSON payload below. Published to topic: <code>{config?.topic_pending ?? "tasks/drg/pending"}</code>
    </div>

    <textarea
      class="json-editor"
      class:json-error={!!jsonError}
      bind:value={jsonBody}
      oninput={handleJsonInput}
      spellcheck="false"
      rows="16"
    ></textarea>

    {#if jsonError}
      <div class="json-error-msg">{jsonError}</div>
    {/if}

    <div class="send-row">
      <button
        class="primary"
        onclick={sendTest}
        disabled={sending || polling || !connected}
      >
        {#if sending}
          Sending...
        {:else if polling}
          Waiting for result...
        {:else}
          Send Test Task
        {/if}
      </button>
      <button
        class="secondary"
        onclick={() => selectExample(selectedExample)}
        disabled={sending || polling}
      >
        Reset to Example
      </button>
    </div>
  </div>

  <!-- Error -->
  {#if testError}
    <div class="error-banner">
      <span>{testError}</span>
      <button class="dismiss" onclick={() => (testError = "")}>x</button>
    </div>
  {/if}

  <!-- Result -->
  {#if testRequestId}
    <div class="card result-card">
      <div class="result-header">
        <h3>Result</h3>
        <div class="result-header-right">
          {#if polling}
            <span class="badge info">Processing...</span>
          {:else if testResult}
            {#if testResult.status === "success"}
              <span class="badge success">Success</span>
            {:else}
              <span class="badge error">Error</span>
            {/if}
          {/if}
        </div>
      </div>

      <!-- Request ID + timing bar -->
      <div class="result-id-bar">
        <div class="id-group">
          <span class="id-label">Request</span>
          <code class="id-value">{testRequestId}</code>
        </div>
        {#if testResult}
          <div class="id-group">
            <span class="id-label">Time</span>
            <code class="id-value">{testResult.processing_ms}ms</code>
          </div>
          <div class="id-group">
            <span class="id-label">At</span>
            <code class="id-value">{new Date(testResult.completed_at).toLocaleTimeString("th-TH")}</code>
          </div>
        {/if}
      </div>

      {#if testResult}
        <!-- DRG Results per case -->
        {#if testResult.response_data?.cases}
          {#each testResult.response_data.cases as caseResult, i}
            <div class="case-result">
              {#if testResult.response_data.cases.length > 1}
                <div class="case-label">Case {i + 1}</div>
              {/if}

              <!-- Main DRG display -->
              <div class="drg-display">
                <div class="drg-main">
                  <span class="drg-code">{caseResult.drg || "-----"}</span>
                  <span class="drg-label">DRG</span>
                </div>
                <div class="drg-detail-grid">
                  <div class="drg-detail">
                    <span class="detail-value">{caseResult.mdc || "--"}</span>
                    <span class="detail-label">MDC</span>
                  </div>
                  <div class="drg-detail">
                    <span class="detail-value">{typeof caseResult.rw === "number" ? caseResult.rw.toFixed(4) : "-"}</span>
                    <span class="detail-label">RW</span>
                  </div>
                  <div class="drg-detail">
                    <span class="detail-value">{typeof caseResult.adjrw === "number" ? caseResult.adjrw.toFixed(4) : "-"}</span>
                    <span class="detail-label">AdjRW</span>
                  </div>
                  <div class="drg-detail">
                    <span class="detail-value">{typeof caseResult.wtlos === "number" ? caseResult.wtlos.toFixed(2) : "-"}</span>
                    <span class="detail-label">WTLOS</span>
                  </div>
                  <div class="drg-detail">
                    <span class="detail-value {caseResult.error_code > 0 ? 'has-error' : ''}">{caseResult.error_code ?? "-"}</span>
                    <span class="detail-label">Error</span>
                  </div>
                  <div class="drg-detail">
                    <span class="detail-value {caseResult.warning_code > 0 ? 'has-warning' : ''}">{caseResult.warning_code ?? "-"}</span>
                    <span class="detail-label">Warning</span>
                  </div>
                </div>
              </div>

              <!-- Input summary for this case -->
              {#if testResult.request_data?.cases?.[i]}
                {@const req = testResult.request_data.cases[i]}
                <div class="input-summary">
                  <div class="input-row">
                    <span class="input-tag pdx">PDx</span>
                    <code>{req.pdx}</code>
                    {#if req.sdx?.length > 0}
                      <span class="input-tag sdx">SDx</span>
                      <code>{req.sdx.join(", ")}</code>
                    {/if}
                    {#if req.procedures?.length > 0}
                      <span class="input-tag proc">Proc</span>
                      <code>{req.procedures.join(", ")}</code>
                    {/if}
                  </div>
                  <div class="input-row sub">
                    <span>Age: {req.age}{req.age_in_days != null ? ` (${req.age_in_days}d)` : ""}</span>
                    <span>Sex: {req.sex === "1" ? "M" : "F"}</span>
                    <span>LOS: {req.los}d</span>
                    <span>DischT: {req.discharge_type}</span>
                    {#if req.admission_weight}
                      <span>Wt: {req.admission_weight}kg</span>
                    {/if}
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        {:else if testResult.status !== "success"}
          <!-- Error display -->
          <div class="error-result">
            <span class="error-result-label">Error</span>
            <span class="error-result-msg">{testResult.status}</span>
          </div>
        {/if}

        <!-- Collapsible raw data -->
        <div class="raw-section">
          <details class="raw-details">
            <summary>Raw Response JSON</summary>
            <pre>{JSON.stringify(testResult.response_data, null, 2)}</pre>
          </details>
          <details class="raw-details">
            <summary>Raw Request JSON</summary>
            <pre>{JSON.stringify(testResult.request_data, null, 2)}</pre>
          </details>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .test-page {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .test-page h2 {
    font-size: 15px;
    font-weight: 600;
  }

  h3 {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 12px;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  /* Topics Card */
  .topic-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .topic-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }

  .topic-dir {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
    width: 32px;
    text-align: center;
    flex-shrink: 0;
  }

  .topic-dir.sub {
    background: rgba(59, 130, 246, 0.15);
    color: var(--info);
  }

  .topic-dir.pub {
    background: rgba(34, 197, 94, 0.15);
    color: var(--success);
  }

  .topic-label {
    width: 110px;
    flex-shrink: 0;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .topic-value {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    color: var(--text-primary);
    background: var(--bg-input);
    padding: 3px 8px;
    border-radius: 4px;
    flex: 1;
  }

  .topic-dynamic {
    color: var(--accent);
    font-style: italic;
  }

  .topic-info {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-muted);
  }

  .topic-info code {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    color: var(--text-secondary);
  }

  /* Examples */
  .examples-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .example-card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    text-align: left;
    padding: 10px 12px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .example-card:hover {
    border-color: var(--accent);
  }

  .example-card.selected {
    border-color: var(--accent);
    background: rgba(99, 102, 241, 0.08);
  }

  .example-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .example-desc {
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.4;
  }

  /* JSON Editor */
  .editor-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .editor-header h3 {
    margin-bottom: 0;
  }

  .editor-hint {
    font-size: 12px;
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .editor-hint code {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-input);
    padding: 1px 4px;
    border-radius: 3px;
  }

  .json-editor {
    width: 100%;
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    line-height: 1.5;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px;
    color: var(--text-primary);
    resize: vertical;
    outline: none;
    transition: border-color 0.15s ease;
    tab-size: 2;
  }

  .json-editor:focus {
    border-color: var(--accent);
  }

  .json-editor.json-error {
    border-color: var(--error);
  }

  .json-error-msg {
    font-size: 12px;
    color: var(--error);
    margin-top: 4px;
  }

  .send-row {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }

  /* Error */
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
  }

  .dismiss {
    background: none;
    color: var(--error);
    font-size: 18px;
    padding: 0 4px;
    line-height: 1;
  }

  /* Result */
  .result-card h3 {
    margin-bottom: 0;
  }

  .result-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .result-header-right {
    display: flex;
    gap: 6px;
  }

  .result-id-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 12px;
    background: var(--bg-secondary);
    border-radius: 6px;
    margin-bottom: 14px;
  }

  .id-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .id-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .id-value {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    color: var(--text-secondary);
    background: none;
  }

  /* Case result */
  .case-result {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    margin-bottom: 10px;
  }

  .case-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 8px 12px 0;
  }

  /* DRG Display */
  .drg-display {
    display: flex;
    align-items: stretch;
    gap: 0;
  }

  .drg-main {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 14px 20px;
    background: rgba(99, 102, 241, 0.06);
    border-right: 1px solid var(--border);
    min-width: 100px;
  }

  .drg-code {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 26px;
    font-weight: 800;
    color: var(--accent);
    letter-spacing: 2px;
    line-height: 1;
  }

  .drg-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 1px;
    margin-top: 4px;
  }

  .drg-detail-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0;
    flex: 1;
  }

  .drg-detail {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 10px 8px;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .drg-detail:nth-child(3n) {
    border-right: none;
  }

  .drg-detail:nth-last-child(-n + 3) {
    border-bottom: none;
  }

  .detail-value {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
    line-height: 1;
  }

  .detail-value.has-error {
    color: var(--error);
  }

  .detail-value.has-warning {
    color: var(--warning);
  }

  .detail-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-top: 3px;
  }

  /* Input summary */
  .input-summary {
    padding: 8px 12px;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border);
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    font-size: 12px;
  }

  .input-row code {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    color: var(--text-primary);
    margin-right: 4px;
  }

  .input-row.sub {
    margin-top: 4px;
    gap: 12px;
    color: var(--text-muted);
    font-size: 11px;
  }

  .input-tag {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .input-tag.pdx {
    background: rgba(99, 102, 241, 0.15);
    color: var(--accent);
  }

  .input-tag.sdx {
    background: rgba(34, 197, 94, 0.15);
    color: var(--success);
  }

  .input-tag.proc {
    background: rgba(245, 158, 11, 0.15);
    color: var(--warning);
  }

  /* Error result */
  .error-result {
    padding: 14px;
    background: rgba(239, 68, 68, 0.06);
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 8px;
    margin-bottom: 10px;
  }

  .error-result-label {
    display: block;
    font-size: 10px;
    font-weight: 700;
    color: var(--error);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .error-result-msg {
    font-size: 13px;
    color: var(--text-secondary);
  }

  /* Raw data */
  .raw-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 6px;
  }

  .raw-details summary {
    font-size: 12px;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px 0;
  }

  .raw-details summary:hover {
    color: var(--text-secondary);
  }

  .raw-details pre {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    line-height: 1.5;
    background: rgba(0, 0, 0, 0.2);
    padding: 10px;
    border-radius: 6px;
    overflow-x: auto;
    max-height: 250px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 6px 0 0 0;
    color: var(--text-secondary);
  }

  .mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
  }
</style>
