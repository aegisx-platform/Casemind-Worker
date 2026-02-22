#!/usr/bin/env node
// Test script: publish a DRG task via MQTT and wait for result
import mqtt from "mqtt";

const BROKER = "mqtt://localhost:1883";
const REQUEST_ID = `test-${Date.now()}`;

const client = mqtt.connect(BROKER, { clientId: "test-publisher" });

client.on("connect", () => {
  console.log("Connected to broker");

  // Subscribe to result topic
  client.subscribe(`tasks/drg/results/${REQUEST_ID}`, (err) => {
    if (err) {
      console.error("Subscribe error:", err);
      process.exit(1);
    }
    console.log(`Subscribed to tasks/drg/results/${REQUEST_ID}`);

    // Publish test task
    const task = {
      request_id: REQUEST_ID,
      version_id: "TDS6307",
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
      published_at: new Date().toISOString(),
    };

    console.log("\nPublishing task:", JSON.stringify(task, null, 2));
    client.publish("tasks/drg/pending", JSON.stringify(task), { qos: 1 });
    console.log("Task published! Waiting for result...\n");
  });
});

client.on("message", (topic, message) => {
  console.log("=== RESULT RECEIVED ===");
  const result = JSON.parse(message.toString());
  console.log(JSON.stringify(result, null, 2));
  console.log("\nDRG:", result.cases?.[0]?.drg);
  console.log("MDC:", result.cases?.[0]?.mdc);
  console.log("RW:", result.cases?.[0]?.rw);
  console.log("AdjRW:", result.cases?.[0]?.adjrw);
  console.log("Processing time:", result.processing_time_ms, "ms");
  client.end();
  process.exit(0);
});

// Timeout after 30s
setTimeout(() => {
  console.error("Timeout: no result received after 30s");
  client.end();
  process.exit(1);
}, 30000);
