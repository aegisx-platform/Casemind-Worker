/// POC: สร้าง DBF → รัน TGrp6305.exe ผ่าน Wine → อ่านผล DRG
/// รัน: cargo test --manifest-path src-tauri/Cargo.toml --test poc_exe -- --nocapture

use std::path::Path;
use std::process::Command;

// We need to bring in the lib modules
// Since this is an integration test, we reference the library crate
use casemind_worker_lib::*;

fn main() {}

#[test]
fn test_exe_via_wine() {
    let exe_dir = "/Users/sathitseethaphon/Downloads/TGrp6305";
    let exe_path = format!("{}/TGrp6305.exe", exe_dir);

    // Verify exe exists
    assert!(
        Path::new(&exe_path).exists(),
        "TGrp6305.exe not found at {}",
        exe_path
    );

    // Test case: I500 (Heart failure), age 65, male, LOS 5
    let dbf_filename = "test_poc.dbf";
    let dbf_path = format!("{}/{}", exe_dir, dbf_filename);

    // Create DBF using our library's dbf module
    // Since we can't directly access private modules from integration tests,
    // we'll create the DBF manually using the dbase crate
    create_test_dbf(&dbf_path);

    // Run exe via Wine
    let output = Command::new("wine")
        .arg(&exe_path)
        .arg(dbf_filename)
        .arg("0")
        .current_dir(exe_dir)
        .output()
        .expect("Failed to run wine");

    println!("Exit code: {:?}", output.status.code());
    println!(
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    println!(
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read results
    let results = read_test_dbf(&dbf_path);
    assert!(!results.is_empty(), "No results read from DBF");

    let r = &results[0];
    println!("\n=== RESULT ===");
    println!("DRG:   {}", r.drg);
    println!("MDC:   {}", r.mdc);
    println!("RW:    {}", r.rw);
    println!("ADJRW: {}", r.adjrw);
    println!("Err:   {}", r.err);
    println!("Warn:  {}", r.warn);

    assert!(!r.drg.is_empty(), "DRG should not be empty");
    assert!(r.err == 0, "Error code should be 0, got {}", r.err);

    // Cleanup
    let _ = std::fs::remove_file(&dbf_path);
    println!("\nPOC SUCCESS!");
}

fn create_test_dbf(path: &str) {
    use dbase::{FieldName, FieldValue, Record, TableWriterBuilder};

    fn fn_name(name: &str) -> FieldName {
        FieldName::try_from(name).unwrap()
    }

    let mut builder = TableWriterBuilder::new()
        .add_date_field(fn_name("DOB"))
        .add_character_field(fn_name("Sex"), 1)
        .add_date_field(fn_name("DateAdm"))
        .add_date_field(fn_name("DateDsc"))
        .add_character_field(fn_name("TimeAdm"), 4)
        .add_character_field(fn_name("TimeDsc"), 4)
        .add_character_field(fn_name("Discht"), 1)
        .add_numeric_field(fn_name("AdmWt"), 7, 3)
        .add_numeric_field(fn_name("ActLOS"), 3, 0)
        .add_character_field(fn_name("Age"), 3)
        .add_character_field(fn_name("AgeDay"), 3)
        .add_character_field(fn_name("PDx"), 6);

    for i in 1..=12 {
        builder = builder.add_character_field(fn_name(&format!("SDx{}", i)), 6);
    }
    for i in 1..=20 {
        builder = builder.add_character_field(fn_name(&format!("Proc{}", i)), 7);
    }

    builder = builder
        .add_numeric_field(fn_name("LeaveDay"), 3, 0)
        .add_character_field(fn_name("DRG"), 5)
        .add_character_field(fn_name("MDC"), 2)
        .add_numeric_field(fn_name("Err"), 2, 0)
        .add_numeric_field(fn_name("Warn"), 4, 0)
        .add_numeric_field(fn_name("RW"), 7, 4)
        .add_numeric_field(fn_name("OT"), 4, 0)
        .add_numeric_field(fn_name("WTLOS"), 6, 2)
        .add_numeric_field(fn_name("ADJRW"), 8, 4);

    let mut writer = builder
        .build_with_file_dest(Path::new(path))
        .expect("Failed to create DBF writer");

    let mut rec = Record::default();

    // DOB: 1961-01-01 (age ~65)
    rec.insert("DOB".to_string(), FieldValue::Date(Some(dbase::Date::new(1, 1, 1961))));
    rec.insert("Sex".to_string(), FieldValue::Character(Some("1".to_string())));
    // DateAdm: 2026-01-01
    rec.insert("DateAdm".to_string(), FieldValue::Date(Some(dbase::Date::new(1, 1, 2026))));
    // DateDsc: 2026-01-06 (LOS=5)
    rec.insert("DateDsc".to_string(), FieldValue::Date(Some(dbase::Date::new(6, 1, 2026))));
    rec.insert("TimeAdm".to_string(), FieldValue::Character(Some("0000".to_string())));
    rec.insert("TimeDsc".to_string(), FieldValue::Character(Some("2359".to_string())));
    rec.insert("Discht".to_string(), FieldValue::Character(Some("1".to_string())));
    rec.insert("AdmWt".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("ActLOS".to_string(), FieldValue::Numeric(Some(5.0)));
    rec.insert("Age".to_string(), FieldValue::Character(Some("65".to_string())));
    rec.insert("AgeDay".to_string(), FieldValue::Character(Some("".to_string())));
    rec.insert("PDx".to_string(), FieldValue::Character(Some("I500".to_string())));

    // SDx1-12 empty
    for i in 1..=12 {
        rec.insert(format!("SDx{}", i), FieldValue::Character(Some("".to_string())));
    }
    // Proc1-20 empty
    for i in 1..=20 {
        rec.insert(format!("Proc{}", i), FieldValue::Character(Some("".to_string())));
    }

    // Output fields (zeros)
    rec.insert("LeaveDay".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("DRG".to_string(), FieldValue::Character(Some("".to_string())));
    rec.insert("MDC".to_string(), FieldValue::Character(Some("".to_string())));
    rec.insert("Err".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("Warn".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("RW".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("OT".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("WTLOS".to_string(), FieldValue::Numeric(Some(0.0)));
    rec.insert("ADJRW".to_string(), FieldValue::Numeric(Some(0.0)));

    writer.write_record(&rec).expect("Failed to write record");
    writer.close().expect("Failed to close DBF");
}

struct TestResult {
    drg: String,
    mdc: String,
    rw: f64,
    adjrw: f64,
    err: i32,
    warn: i32,
}

fn read_test_dbf(path: &str) -> Vec<TestResult> {
    use dbase::FieldValue;

    let mut reader = dbase::Reader::from_path(path).expect("Failed to open result DBF");
    let mut results = Vec::new();

    for record_result in reader.iter_records() {
        let record = record_result.expect("Failed to read record");

        let drg = match record.get("DRG") {
            Some(FieldValue::Character(Some(s))) => s.trim().to_string(),
            _ => String::new(),
        };
        let mdc = match record.get("MDC") {
            Some(FieldValue::Character(Some(s))) => s.trim().to_string(),
            _ => String::new(),
        };
        let rw = match record.get("RW") {
            Some(FieldValue::Numeric(Some(n))) => *n,
            _ => 0.0,
        };
        let adjrw = match record.get("ADJRW") {
            Some(FieldValue::Numeric(Some(n))) => *n,
            _ => 0.0,
        };
        let err = match record.get("Err") {
            Some(FieldValue::Numeric(Some(n))) => *n as i32,
            _ => 0,
        };
        let warn = match record.get("Warn") {
            Some(FieldValue::Numeric(Some(n))) => *n as i32,
            _ => 0,
        };

        results.push(TestResult {
            drg,
            mdc,
            rw,
            adjrw,
            err,
            warn,
        });
    }

    results
}
