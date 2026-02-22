<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  type Config = {
    broker_url: string;
    client_id: string;
    exe_base_path: string;
    max_concurrent: number;
    version: string;
    auto_start: boolean;
    download_url: string;
  };

  let { onSaved }: { onSaved: () => void } = $props();

  let config = $state<Config>({
    broker_url: "mqtt://localhost:1883",
    client_id: "",
    exe_base_path: "",
    max_concurrent: 4,
    version: "TDS6307",
    auto_start: false,
    download_url: "https://www.tcmc.or.th/download-tcmc",
  });

  let exeStatus = $state<"checking" | "found" | "not_found" | "unknown">("unknown");
  let saving = $state(false);
  let downloading = $state(false);
  let saved = $state(false);
  let downloadError = $state("");

  async function loadConfig() {
    try {
      config = await invoke<Config>("get_config");
      await checkExePath();
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }

  async function checkExePath() {
    if (!config.exe_base_path) {
      exeStatus = "unknown";
      return;
    }
    exeStatus = "checking";
    try {
      const found = await invoke<boolean>("check_exe_path", {
        path: config.exe_base_path,
      });
      exeStatus = found ? "found" : "not_found";
    } catch {
      exeStatus = "not_found";
    }
  }

  async function handleSave() {
    saving = true;
    saved = false;
    try {
      await invoke("save_config", { config });
      saved = true;
      onSaved();
      setTimeout(() => (saved = false), 2000);
    } catch (e: any) {
      console.error("Failed to save:", e);
    } finally {
      saving = false;
    }
  }

  async function handleBrowse() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select TGrp6305.exe folder",
      });
      if (selected) {
        config.exe_base_path = selected as string;
        await checkExePath();
      }
    } catch (e) {
      console.error("Browse failed:", e);
    }
  }

  async function handleDownload() {
    downloading = true;
    downloadError = "";
    try {
      const path = await invoke<string>("download_exe");
      config.exe_base_path = path;
      await checkExePath();
    } catch (e: any) {
      downloadError = e.toString();
    } finally {
      downloading = false;
    }
  }

  onMount(loadConfig);
</script>

<div class="settings">
  <h2>Settings</h2>

  <div class="settings-grid">
    <!-- MQTT Broker -->
    <div class="setting-group card">
      <h3>MQTT Broker</h3>
      <div class="field">
        <label for="broker_url">Broker URL</label>
        <input
          id="broker_url"
          type="text"
          bind:value={config.broker_url}
          placeholder="mqtt://broker-host:1883"
        />
      </div>
      <div class="field">
        <label for="client_id">Worker ID</label>
        <input
          id="client_id"
          type="text"
          bind:value={config.client_id}
          placeholder="Auto-generated"
        />
        <span class="hint">Unique identifier for this worker instance</span>
      </div>
    </div>

    <!-- Exe Path -->
    <div class="setting-group card">
      <h3>TGrp6305.exe</h3>
      <div class="field">
        <label for="exe_path">Exe Folder Path</label>
        <div class="path-row">
          <input
            id="exe_path"
            type="text"
            bind:value={config.exe_base_path}
            placeholder="Path to TGrp6305.exe folder"
            onchange={checkExePath}
          />
          <button class="secondary" onclick={handleBrowse}>Browse...</button>
        </div>
        <div class="exe-status">
          {#if exeStatus === "found"}
            <span class="badge success">TGrp6305.exe found</span>
          {:else if exeStatus === "not_found"}
            <span class="badge error">TGrp6305.exe not found</span>
          {:else if exeStatus === "checking"}
            <span class="badge info">Checking...</span>
          {:else}
            <span class="badge neutral">Not configured</span>
          {/if}
        </div>
      </div>

      <div class="divider"></div>

      <div class="field">
        <label for="download_url">Download URL (TCMC)</label>
        <input
          id="download_url"
          type="text"
          bind:value={config.download_url}
          placeholder="https://www.tcmc.or.th/download-tcmc"
        />
        <span class="hint">URL to download TGrp6305.exe ZIP package</span>
      </div>
      <button
        class="secondary download-btn"
        onclick={handleDownload}
        disabled={downloading}
      >
        {downloading ? "Downloading..." : "Download from TCMC"}
      </button>
      {#if downloadError}
        <div class="download-error">{downloadError}</div>
      {/if}
    </div>

    <!-- Worker Config -->
    <div class="setting-group card">
      <h3>Worker</h3>
      <div class="field">
        <label for="version">TDS Version</label>
        <input
          id="version"
          type="text"
          bind:value={config.version}
          placeholder="TDS6307"
        />
      </div>
      <div class="field">
        <label for="max_concurrent">Max Concurrent Processes</label>
        <input
          id="max_concurrent"
          type="number"
          bind:value={config.max_concurrent}
          min="1"
          max="16"
        />
        <span class="hint">Number of TGrp6305.exe instances to run in parallel</span>
      </div>
      <div class="field checkbox-field">
        <label>
          <input type="checkbox" bind:checked={config.auto_start} />
          Auto-connect on startup
        </label>
      </div>
    </div>
  </div>

  <div class="save-row">
    <button class="primary" onclick={handleSave} disabled={saving}>
      {saving ? "Saving..." : "Save Settings"}
    </button>
    {#if saved}
      <span class="save-ok">Settings saved</span>
    {/if}
  </div>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .settings h2 {
    font-size: 15px;
    font-weight: 600;
  }

  .settings-grid {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .setting-group h3 {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 12px;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .field {
    margin-bottom: 12px;
  }

  .field label {
    display: block;
    font-size: 12px;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .field input[type="text"],
  .field input[type="number"] {
    width: 100%;
  }

  .hint {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 3px;
  }

  .path-row {
    display: flex;
    gap: 8px;
  }

  .path-row input {
    flex: 1;
  }

  .exe-status {
    margin-top: 6px;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 12px 0;
  }

  .download-btn {
    width: 100%;
  }

  .download-error {
    margin-top: 6px;
    font-size: 12px;
    color: var(--error);
  }

  .checkbox-field label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-primary);
  }

  .checkbox-field input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .save-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .save-ok {
    font-size: 13px;
    color: var(--success);
  }
</style>
