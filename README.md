# CaseMind Worker

Tauri desktop application that acts as a distributed DRG (Diagnosis Related Group) grouper worker agent. Connects to a central MQTT broker, receives patient cases, runs the Thai TDRG 6.3.3 grouper (TGrp6305.exe), and publishes results back.

**Stack:** Rust (Tauri 2) + Svelte 5 + MQTT (rumqttc) + Visual FoxPro DBF

## Architecture

```
                    ┌──────────────────┐
                    │  MQTT Broker     │
                    │  (Mosquitto)     │
                    └──┬───────────┬───┘
                       │           │
          subscribe    │           │  subscribe
     tasks/drg/pending │           │  tasks/drg/pending
                       │           │
              ┌────────▼──┐   ┌───▼─────────┐
              │ Worker A  │   │  Worker B   │
              │ Windows   │   │  macOS      │
              │ (native)  │   │ (via Wine)  │
              └───────────┘   └─────────────┘
```

Multiple workers connect to a shared MQTT broker. Each worker:
1. Subscribes to `tasks/drg/pending`
2. Receives a DRG task (patient case data)
3. Creates a DBF file from the case data
4. Runs `TGrp6305.exe` (native on Windows, via Wine on macOS/Linux)
5. Reads the DRG result from the DBF output
6. Publishes the result to `tasks/drg/results/{request_id}`

## Prerequisites

### All Platforms
- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) >= 1.77

### macOS
- [Wine](https://wiki.winehq.org/Download) (for running TGrp6305.exe)
  ```bash
  brew install --cask wine-stable
  ```

### Windows
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (usually pre-installed on Windows 10/11)
- No Wine needed — exe runs natively

## Quick Start

```bash
# Install dependencies
npm install

# Development (hot-reload frontend + Rust rebuild on save)
npm run tauri dev

# Build for production
npm run tauri build
```

## Configuration

Config is stored at:
- **macOS:** `~/Library/Application Support/com.aegisx.casemind-worker/config.json`
- **Windows:** `%APPDATA%/aegisx/casemind-worker/config/config.json`
- **Linux:** `~/.config/com.aegisx.casemind-worker/config.json`

### Config Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `broker_url` | string | `mqtt://localhost:1883` | MQTT broker URL |
| `client_id` | string | `casemind-worker-{uuid}` | Unique worker ID (auto-generated) |
| `exe_base_path` | string | `""` | Path to folder containing TGrp6305.exe |
| `max_concurrent` | number | `4` | Max parallel exe processes |
| `version` | string | `TDS6307` | TDS version identifier |
| `auto_start` | boolean | `false` | Auto-connect to broker on app launch |
| `download_url` | string | TCMC S0021.zip URL | URL to download TGrp6305.exe package |

### Example config.json

```json
{
  "broker_url": "mqtt://192.168.1.100:1883",
  "client_id": "casemind-worker-server-01",
  "exe_base_path": "/Users/admin/TGrp6305",
  "max_concurrent": 4,
  "version": "TDS6307",
  "auto_start": true,
  "download_url": "https://www.tcmc.or.th/_content_images/download/fileupload/S0021.zip"
}
```

## TGrp6305.exe Setup

Two ways to get the exe:

### Option 1: Download from TCMC (in-app)
1. Open Settings tab
2. Click **"Download from TCMC"**
3. App downloads ~40MB ZIP, extracts to config directory
4. Exe path is set automatically

### Option 2: Manual path
1. Download from [TCMC](https://www.tcmc.or.th/download-tcmc) manually
2. Extract the ZIP
3. In Settings, click **Browse** or paste the path to the folder containing `TGrp6305.exe`

Required files in the exe folder:
```
TGrp6305.exe      # Main grouper executable
c63ccex.dbf        # CC Exclusion data
c63drg.dbf         # DRG table
c63i10.dbf         # ICD-10 codes
c63proc.dbf        # Procedure codes
vfp9r.dll          # Visual FoxPro runtime
vfp9t.dll          # Visual FoxPro runtime
msvcr71.dll        # MSVC runtime
gdiplus.dll        # GDI+ library
```

## MQTT Topics

### Subscribe (Worker listens)

| Topic | QoS | Description |
|-------|-----|-------------|
| `tasks/drg/pending` | 1 | DRG grouping tasks from CaseMind API |

### Publish (Worker sends)

| Topic | QoS | Retained | Description |
|-------|-----|----------|-------------|
| `tasks/drg/results/{request_id}` | 1 | No | Grouping result for a specific request |
| `workers/health/{worker_id}` | 0 | Yes | Heartbeat every 30 seconds |
| `workers/register/{worker_id}` | 1 | Yes | Worker registration on connect |

## Message Formats

### DRG Task (API → Worker)

Published to `tasks/drg/pending`:

```json
{
  "request_id": "req-abc123",
  "version_id": "TDS6307",
  "published_at": "2026-02-22T07:00:00.000Z",
  "cases": [
    {
      "pdx": "I500",
      "sdx": ["E119", "I10"],
      "procedures": [],
      "age": 65,
      "age_in_days": null,
      "sex": "1",
      "discharge_type": 1,
      "los": 5,
      "admission_weight": null
    }
  ]
}
```

#### Case Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pdx` | string | Yes | Principal diagnosis code (ICD-10, no dots) |
| `sdx` | string[] | Yes | Secondary diagnoses (up to 12) |
| `procedures` | string[] | Yes | Procedure codes (up to 20, ICD-9-CM) |
| `age` | number | Yes | Patient age in years |
| `age_in_days` | number | No | Age in days (for neonates) |
| `sex` | string | Yes | `"1"` = Male, `"2"` = Female |
| `discharge_type` | number | Yes | Discharge type code |
| `los` | number | Yes | Length of stay in days |
| `admission_weight` | number | No | Admission weight in kg (neonates) |

### DRG Result (Worker → API)

Published to `tasks/drg/results/{request_id}`:

```json
{
  "request_id": "req-abc123",
  "worker_id": "casemind-worker-xxxx-xxxx",
  "version_id": "TDS6307",
  "processing_time_ms": 1250,
  "completed_at": "2026-02-22T07:00:01.250Z",
  "cases": [
    {
      "drg": "05550",
      "mdc": "05",
      "rw": 0.6831,
      "adjrw": 0.6831,
      "wtlos": 3.36,
      "error_code": 0,
      "warning_code": 0
    }
  ]
}
```

#### Result Fields

| Field | Type | Description |
|-------|------|-------------|
| `drg` | string | 5-digit DRG code (e.g. `"05550"`) |
| `mdc` | string | 2-digit MDC (e.g. `"05"`) |
| `rw` | number | Relative Weight from DRG table |
| `adjrw` | number | Adjusted RW (LOS-adjusted) |
| `wtlos` | number | Weighted LOS threshold |
| `error_code` | number | `0` = OK, `1`-`5` = validation error |
| `warning_code` | number | Warning flags |

#### Error Codes

| Code | Description |
|------|-------------|
| 0 | No error |
| 1 | No principal diagnosis |
| 2 | Invalid principal diagnosis |
| 3 | Unacceptable principal diagnosis |
| 4 | Age conflict |
| 5 | Sex conflict |

### Worker Health (Heartbeat)

Published to `workers/health/{worker_id}` every 30 seconds (retained):

```json
{
  "worker_id": "casemind-worker-xxxx-xxxx",
  "status": "active",
  "tasks_completed": 142,
  "avg_processing_ms": 1850.5,
  "uptime_secs": 3600,
  "version": "TDS6307",
  "timestamp": "2026-02-22T08:00:00.000Z"
}
```

## MQTT Broker Setup

### Using Mosquitto (recommended)

#### macOS
```bash
brew install mosquitto
# Start (local only)
mosquitto -p 1883 -v
# Or as service
brew services start mosquitto
```

#### Docker
```bash
docker run -d --name mosquitto \
  -p 1883:1883 \
  eclipse-mosquitto:2 \
  sh -c 'echo "listener 1883
allow_anonymous true" > /tmp/m.conf && mosquitto -c /tmp/m.conf'
```

#### Remote Access Config
Create `mosquitto.conf`:
```
listener 1883
allow_anonymous true

# Optional: WebSocket support
listener 9001
protocol websockets
```

```bash
mosquitto -c mosquitto.conf
```

## Integration with CaseMind API

### Publishing Tasks (Node.js example)

```javascript
import mqtt from "mqtt";

const client = mqtt.connect("mqtt://localhost:1883");
const REQUEST_ID = `req-${Date.now()}`;

client.on("connect", () => {
  // Subscribe to result
  client.subscribe(`tasks/drg/results/${REQUEST_ID}`);

  // Publish task
  client.publish("tasks/drg/pending", JSON.stringify({
    request_id: REQUEST_ID,
    version_id: "TDS6307",
    published_at: new Date().toISOString(),
    cases: [{
      pdx: "I500",
      sdx: ["E119", "I10"],
      procedures: [],
      age: 65,
      age_in_days: null,
      sex: "1",
      discharge_type: 1,
      los: 5,
      admission_weight: null
    }]
  }), { qos: 1 });
});

client.on("message", (topic, message) => {
  const result = JSON.parse(message.toString());
  console.log("DRG:", result.cases[0].drg);    // "05550"
  console.log("RW:", result.cases[0].rw);       // 0.6831
  console.log("AdjRW:", result.cases[0].adjrw); // 0.6831
  client.end();
});
```

### Monitoring Workers

```javascript
// Subscribe to all worker health
client.subscribe("workers/health/#");

client.on("message", (topic, message) => {
  const health = JSON.parse(message.toString());
  console.log(`Worker ${health.worker_id}: ${health.tasks_completed} tasks, ${health.avg_processing_ms}ms avg`);
});
```

## Testing

### Run MQTT Test Script

```bash
# Start Mosquitto broker first
mosquitto -p 1883 -v &

# Start the worker app
npm run tauri dev

# In another terminal, run test
node test-mqtt.mjs
```

Expected output:
```
Connected to broker
Publishing task...
=== RESULT RECEIVED ===
DRG: 05550
MDC: 05
RW: 0.6831
AdjRW: 0.6831
Processing time: 1250 ms
```

## Building for Production

### macOS (.dmg)

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/dmg/CaseMind Worker_0.1.0_aarch64.dmg`

### Windows (.msi / .exe installer)

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/msi/CaseMind Worker_0.1.0_x64_en-US.msi`

### Cross-Compilation Notes

- **macOS → Windows:** Not directly supported. Build on Windows or use CI
- **Windows → macOS:** Not directly supported. Build on macOS or use CI
- **Recommended:** Use GitHub Actions with matrix build (see `.github/workflows/`)

## Performance

| Metric | macOS (Wine) | Windows (native) |
|--------|-------------|-------------------|
| Processing time (single) | ~10s | ~1-2s |
| Max concurrent | 4 (configurable) | 4 (configurable) |
| Throughput (4 concurrent) | ~24 req/min | ~120-240 req/min |
| **Daily capacity** | **~34,500** | **~170,000-345,000** |

> Wine startup adds ~8s overhead per invocation on macOS. On Windows, exe runs natively with no overhead.

## Project Structure

```
casemind-worker/
├── src/                          # Svelte frontend
│   ├── App.svelte                # Main layout + tab routing
│   ├── main.ts                   # App mount point
│   ├── app.css                   # Global styles (dark theme)
│   └── lib/
│       ├── WorkerStatus.svelte   # Dashboard stats + controls
│       ├── TaskLog.svelte        # Task history with detail view
│       └── Settings.svelte       # Configuration UI
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml                # Rust dependencies
│   ├── tauri.conf.json           # Tauri app configuration
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── lib.rs                # Tauri commands + app setup
│   │   ├── mqtt.rs               # MQTT client + message types
│   │   ├── worker.rs             # Task orchestration + stats
│   │   ├── exe_runner.rs         # TGrp6305.exe execution
│   │   ├── config.rs             # Configuration management
│   │   └── dbf.rs                # DBF file I/O (41-field schema)
│   └── icons/                    # App icons
├── test-mqtt.mjs                 # MQTT integration test
├── package.json
└── README.md
```

## Troubleshooting

### "TGrp6305.exe not found"
- Verify the path in Settings points to the folder containing `TGrp6305.exe`
- Try "Download from TCMC" button to auto-download

### Worker connected but not processing
- Check that the MQTT broker is running: `nc -z localhost 1883`
- Verify the broker URL in Settings matches your broker
- Check the worker isn't paused (Dashboard shows "Active")

### Wine errors on macOS
- Install Wine: `brew install --cask wine-stable`
- First run may take longer as Wine initializes
- Verify manually: `wine /path/to/TGrp6305.exe test.dbf 0`

### Download fails
- Check your internet connection
- Try the URL directly in a browser
- The download URL can be changed in Settings

## License

Proprietary - aegisx Platform
