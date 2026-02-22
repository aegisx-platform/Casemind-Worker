#!/usr/bin/env node
/**
 * Load test for CaseMind Worker via MQTT
 *
 * Usage:
 *   node loadtest-mqtt.mjs                    # default: 20 tasks, concurrency 4
 *   node loadtest-mqtt.mjs --total 100        # 100 tasks
 *   node loadtest-mqtt.mjs --total 50 --concurrency 10
 *   node loadtest-mqtt.mjs --batch 5          # 5 cases per task message
 */
import mqtt from "mqtt";

// ── Parse CLI args ─────────────────────────────────────────────────
const args = process.argv.slice(2);
function getArg(name, defaultVal) {
  const idx = args.indexOf(`--${name}`);
  return idx !== -1 && args[idx + 1] ? Number(args[idx + 1]) : defaultVal;
}

const TOTAL_TASKS = getArg("total", 20);
const CONCURRENCY = getArg("concurrency", 4);
const BATCH_SIZE = getArg("batch", 1); // cases per task message
const BROKER = "mqtt://localhost:1883";
const TIMEOUT_MS = 300_000; // 5 min global timeout

// ── Sample test cases (varied DRGs) ────────────────────────────────
const sampleCases = [
  { pdx: "I500", sdx: ["E119", "I10"], procedures: [], age: 65, age_in_days: null, sex: "1", discharge_type: 1, los: 5, admission_weight: null },
  { pdx: "J449", sdx: ["J960"], procedures: [], age: 72, age_in_days: null, sex: "2", discharge_type: 1, los: 3, admission_weight: null },
  { pdx: "K802", sdx: ["K810"], procedures: ["5123"], age: 55, age_in_days: null, sex: "2", discharge_type: 1, los: 7, admission_weight: null },
  { pdx: "S7200", sdx: ["W010"], procedures: ["7935"], age: 80, age_in_days: null, sex: "2", discharge_type: 1, los: 14, admission_weight: null },
  { pdx: "N189", sdx: ["E119", "I10"], procedures: ["3995"], age: 60, age_in_days: null, sex: "1", discharge_type: 1, los: 2, admission_weight: null },
  { pdx: "A099", sdx: [], procedures: [], age: 3, age_in_days: null, sex: "1", discharge_type: 1, los: 2, admission_weight: null },
  { pdx: "O800", sdx: [], procedures: ["7359"], age: 28, age_in_days: null, sex: "2", discharge_type: 1, los: 3, admission_weight: null },
  { pdx: "J181", sdx: ["J960", "E119"], procedures: [], age: 78, age_in_days: null, sex: "1", discharge_type: 1, los: 10, admission_weight: null },
];

// ── Main ───────────────────────────────────────────────────────────
console.log(`\n🔥 CaseMind Worker Load Test`);
console.log(`   Total tasks:  ${TOTAL_TASKS}`);
console.log(`   Concurrency:  ${CONCURRENCY}`);
console.log(`   Batch size:   ${BATCH_SIZE} cases/task`);
console.log(`   Broker:       ${BROKER}\n`);

const client = mqtt.connect(BROKER, {
  clientId: `loadtest-${Date.now()}`,
  clean: true,
});

const pending = new Map(); // requestId -> { sentAt }
const results = [];        // { requestId, processingMs, roundTripMs, drg, success }
let tasksSent = 0;
let tasksInFlight = 0;
let lastResultTime = 0;
const startTime = Date.now();

client.on("connect", () => {
  console.log("Connected to MQTT broker\n");

  // Subscribe to all results
  client.subscribe("tasks/drg/results/#", { qos: 1 }, (err) => {
    if (err) {
      console.error("Subscribe error:", err);
      process.exit(1);
    }

    // Start sending tasks
    fillQueue();
  });
});

client.on("message", (_topic, message) => {
  try {
    const result = JSON.parse(message.toString());
    const reqId = result.request_id;
    const info = pending.get(reqId);
    if (!info) return; // not our request

    pending.delete(reqId);
    tasksInFlight--;

    const roundTripMs = Date.now() - info.sentAt;
    const processingMs = result.processing_time_ms || 0;
    const drg = result.cases?.[0]?.drg || "??";
    const success = !result.error;

    results.push({ requestId: reqId, processingMs, roundTripMs, drg, success });
    lastResultTime = Date.now();

    // Progress
    const pct = ((results.length / TOTAL_TASKS) * 100).toFixed(0);
    process.stdout.write(
      `\r  [${pct.padStart(3)}%] ${results.length}/${TOTAL_TASKS} done | last: ${drg} ${roundTripMs}ms | in-flight: ${tasksInFlight}`
    );

    // Send more if needed
    fillQueue();

    // All done?
    if (results.length >= TOTAL_TASKS) {
      console.log("\n");
      printReport();
      client.end();
      process.exit(0);
    }
  } catch (e) {
    console.error("\nParse error:", e.message);
  }
});

function fillQueue() {
  while (tasksInFlight < CONCURRENCY && tasksSent < TOTAL_TASKS) {
    sendTask();
  }
}

function sendTask() {
  const requestId = `lt-${Date.now()}-${tasksSent}`;
  const cases = [];
  for (let i = 0; i < BATCH_SIZE; i++) {
    cases.push(sampleCases[(tasksSent + i) % sampleCases.length]);
  }

  const task = {
    request_id: requestId,
    version_id: "TDS6307",
    published_at: new Date().toISOString(),
    cases,
  };

  pending.set(requestId, { sentAt: Date.now() });
  client.publish("tasks/drg/pending", JSON.stringify(task), { qos: 1 });
  tasksSent++;
  tasksInFlight++;
}

function printReport() {
  const totalElapsed = (lastResultTime || Date.now()) - startTime;
  const successResults = results.filter((r) => r.success);
  const failResults = results.filter((r) => !r.success);

  const roundTrips = successResults.map((r) => r.roundTripMs).sort((a, b) => a - b);
  const processing = successResults.map((r) => r.processingMs).sort((a, b) => a - b);

  const avg = (arr) => arr.length ? (arr.reduce((a, b) => a + b, 0) / arr.length).toFixed(0) : "N/A";
  const p50 = (arr) => arr.length ? arr[Math.floor(arr.length * 0.5)] : "N/A";
  const p95 = (arr) => arr.length ? arr[Math.floor(arr.length * 0.95)] : "N/A";
  const p99 = (arr) => arr.length ? arr[Math.floor(arr.length * 0.99)] : "N/A";
  const min = (arr) => arr.length ? arr[0] : "N/A";
  const max = (arr) => arr.length ? arr[arr.length - 1] : "N/A";

  const throughput = (TOTAL_TASKS / (totalElapsed / 1000)).toFixed(2);
  const dailyCapacity = Math.floor(throughput * 86400);

  console.log("═══════════════════════════════════════════════════════");
  console.log("  LOAD TEST REPORT");
  console.log("═══════════════════════════════════════════════════════");
  console.log(`  Total tasks:     ${TOTAL_TASKS}`);
  console.log(`  Successful:      ${successResults.length}`);
  console.log(`  Failed:          ${failResults.length}`);
  console.log(`  Elapsed:         ${(totalElapsed / 1000).toFixed(1)}s`);
  console.log(`  Throughput:      ${throughput} tasks/sec`);
  console.log(`  Daily capacity:  ~${dailyCapacity.toLocaleString()} tasks/day`);
  console.log("───────────────────────────────────────────────────────");
  console.log("  Round-trip latency (MQTT publish → result received):");
  console.log(`    Min:    ${min(roundTrips)} ms`);
  console.log(`    Avg:    ${avg(roundTrips)} ms`);
  console.log(`    P50:    ${p50(roundTrips)} ms`);
  console.log(`    P95:    ${p95(roundTrips)} ms`);
  console.log(`    P99:    ${p99(roundTrips)} ms`);
  console.log(`    Max:    ${max(roundTrips)} ms`);
  console.log("───────────────────────────────────────────────────────");
  console.log("  Worker processing time (exe execution):");
  console.log(`    Min:    ${min(processing)} ms`);
  console.log(`    Avg:    ${avg(processing)} ms`);
  console.log(`    P50:    ${p50(processing)} ms`);
  console.log(`    P95:    ${p95(processing)} ms`);
  console.log(`    Max:    ${max(processing)} ms`);
  console.log("───────────────────────────────────────────────────────");

  // DRG distribution
  const drgCounts = {};
  for (const r of successResults) {
    drgCounts[r.drg] = (drgCounts[r.drg] || 0) + 1;
  }
  console.log("  DRG distribution:");
  for (const [drg, count] of Object.entries(drgCounts).sort((a, b) => b[1] - a[1])) {
    console.log(`    ${drg}: ${count}`);
  }
  console.log("═══════════════════════════════════════════════════════\n");
}

// Global timeout
setTimeout(() => {
  console.log(`\n\nTIMEOUT after ${TIMEOUT_MS / 1000}s`);
  console.log(`  Sent: ${tasksSent}, Received: ${results.length}, In-flight: ${tasksInFlight}\n`);
  if (results.length > 0) {
    printReport();
  }
  client.end();
  process.exit(1);
}, TIMEOUT_MS);

client.on("error", (err) => {
  console.error("MQTT error:", err.message);
});
