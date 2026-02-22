use dbase::{FieldName, FieldValue, Record, TableWriterBuilder};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// DBF record matching TGrp6305.exe schema (41 fields).
/// Input fields are set before running exe; output fields are filled by exe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExeDbfRecord {
    // Input fields
    pub dob: String,        // YYYYMMDD
    pub sex: String,        // "1"=Male, "2"=Female
    pub date_adm: String,   // YYYYMMDD
    pub date_dsc: String,   // YYYYMMDD
    pub time_adm: String,   // HHMM
    pub time_dsc: String,   // HHMM
    pub discht: String,     // Discharge type (ones digit)
    pub adm_wt: f64,        // Admission weight
    pub act_los: i32,       // Actual length of stay
    pub age: String,        // Age in years
    pub age_day: String,    // Age in days (neonates)
    pub pdx: String,        // Principal diagnosis
    pub sdx: Vec<String>,   // Secondary diagnoses (up to 12)
    pub procs: Vec<String>, // Procedures (up to 20)

    // Output fields (filled by exe)
    pub leave_day: i32,
    pub drg: String,
    pub mdc: String,
    pub err: i32,
    pub warn: i32,
    pub rw: f64,
    pub ot: i32,
    pub wtlos: f64,
    pub adjrw: f64,
}

impl Default for ExeDbfRecord {
    fn default() -> Self {
        Self {
            dob: String::new(),
            sex: String::new(),
            date_adm: String::new(),
            date_dsc: String::new(),
            time_adm: "0000".to_string(),
            time_dsc: "0000".to_string(),
            discht: "1".to_string(),
            adm_wt: 0.0,
            act_los: 0,
            age: String::new(),
            age_day: String::new(),
            pdx: String::new(),
            sdx: Vec::new(),
            procs: Vec::new(),
            leave_day: 0,
            drg: String::new(),
            mdc: String::new(),
            err: 0,
            warn: 0,
            rw: 0.0,
            ot: 0,
            wtlos: 0.0,
            adjrw: 0.0,
        }
    }
}

/// Parse a dbase Date field value into YYYYMMDD string
fn date_to_string(val: &FieldValue) -> String {
    match val {
        FieldValue::Date(Some(d)) => {
            format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
        }
        _ => String::new(),
    }
}

fn field_to_string(val: &FieldValue) -> String {
    match val {
        FieldValue::Character(Some(s)) => s.trim().to_string(),
        _ => String::new(),
    }
}

fn field_to_f64(val: &FieldValue) -> f64 {
    match val {
        FieldValue::Numeric(Some(n)) => *n,
        FieldValue::Float(Some(n)) => *n as f64,
        _ => 0.0,
    }
}

fn field_to_i32(val: &FieldValue) -> i32 {
    match val {
        FieldValue::Numeric(Some(n)) => *n as i32,
        FieldValue::Float(Some(n)) => *n as i32,
        _ => 0,
    }
}

fn fn_name(name: &str) -> FieldName {
    FieldName::try_from(name).unwrap()
}

/// Create a DBF file with the TGrp6305 schema containing the given records.
pub fn create_dbf(path: &Path, records: &[ExeDbfRecord]) -> Result<(), String> {
    // Build schema using consuming builder pattern — chain all field additions
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

    // SDx1-SDx12
    for i in 1..=12 {
        builder = builder.add_character_field(fn_name(&format!("SDx{}", i)), 6);
    }

    // Proc1-Proc20
    for i in 1..=20 {
        builder = builder.add_character_field(fn_name(&format!("Proc{}", i)), 7);
    }

    // Output fields
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

    let mut writer = builder.build_with_file_dest(path).map_err(|e| e.to_string())?;

    for rec in records {
        let mut dbf_rec = Record::default();

        // Helper closures using String keys for Record::insert
        let set_char = |r: &mut Record, name: &str, val: &str| {
            r.insert(
                name.to_string(),
                FieldValue::Character(Some(val.to_string())),
            );
        };
        let set_date = |r: &mut Record, name: &str, yyyymmdd: &str| {
            if yyyymmdd.len() == 8 {
                let y: u32 = yyyymmdd[0..4].parse().unwrap_or(2026);
                let m: u32 = yyyymmdd[4..6].parse().unwrap_or(1);
                let d: u32 = yyyymmdd[6..8].parse().unwrap_or(1);
                let date = dbase::Date::new(d, m, y);
                r.insert(
                    name.to_string(),
                    FieldValue::Date(Some(date)),
                );
            }
        };
        let set_num = |r: &mut Record, name: &str, val: f64| {
            r.insert(
                name.to_string(),
                FieldValue::Numeric(Some(val)),
            );
        };

        set_date(&mut dbf_rec, "DOB", &rec.dob);
        set_char(&mut dbf_rec, "Sex", &rec.sex);
        set_date(&mut dbf_rec, "DateAdm", &rec.date_adm);
        set_date(&mut dbf_rec, "DateDsc", &rec.date_dsc);
        set_char(&mut dbf_rec, "TimeAdm", &rec.time_adm);
        set_char(&mut dbf_rec, "TimeDsc", &rec.time_dsc);
        set_char(&mut dbf_rec, "Discht", &rec.discht);
        set_num(&mut dbf_rec, "AdmWt", rec.adm_wt);
        set_num(&mut dbf_rec, "ActLOS", rec.act_los as f64);
        set_char(&mut dbf_rec, "Age", &rec.age);
        set_char(&mut dbf_rec, "AgeDay", &rec.age_day);
        set_char(&mut dbf_rec, "PDx", &rec.pdx);

        for i in 0..12 {
            let val = rec.sdx.get(i).map(|s| s.as_str()).unwrap_or("");
            set_char(&mut dbf_rec, &format!("SDx{}", i + 1), val);
        }

        for i in 0..20 {
            let val = rec.procs.get(i).map(|s| s.as_str()).unwrap_or("");
            set_char(&mut dbf_rec, &format!("Proc{}", i + 1), val);
        }

        // Output fields (empty/zero — exe will fill them)
        set_num(&mut dbf_rec, "LeaveDay", 0.0);
        set_char(&mut dbf_rec, "DRG", "");
        set_char(&mut dbf_rec, "MDC", "");
        set_num(&mut dbf_rec, "Err", 0.0);
        set_num(&mut dbf_rec, "Warn", 0.0);
        set_num(&mut dbf_rec, "RW", 0.0);
        set_num(&mut dbf_rec, "OT", 0.0);
        set_num(&mut dbf_rec, "WTLOS", 0.0);
        set_num(&mut dbf_rec, "ADJRW", 0.0);

        writer.write_record(&dbf_rec).map_err(|e| e.to_string())?;
    }

    writer.close().map_err(|e| e.to_string())?;
    Ok(())
}

/// Read DBF records output by TGrp6305.exe.
pub fn read_dbf(path: &Path) -> Result<Vec<ExeDbfRecord>, String> {
    let mut reader = dbase::Reader::from_path(path).map_err(|e| e.to_string())?;
    let mut results = Vec::new();

    for record_result in reader.iter_records() {
        let record = record_result.map_err(|e| e.to_string())?;

        let mut rec = ExeDbfRecord::default();
        rec.dob = date_to_string(record.get("DOB").unwrap_or(&FieldValue::Date(None)));
        rec.sex = field_to_string(record.get("Sex").unwrap_or(&FieldValue::Character(None)));
        rec.date_adm =
            date_to_string(record.get("DateAdm").unwrap_or(&FieldValue::Date(None)));
        rec.date_dsc =
            date_to_string(record.get("DateDsc").unwrap_or(&FieldValue::Date(None)));
        rec.time_adm =
            field_to_string(record.get("TimeAdm").unwrap_or(&FieldValue::Character(None)));
        rec.time_dsc =
            field_to_string(record.get("TimeDsc").unwrap_or(&FieldValue::Character(None)));
        rec.discht =
            field_to_string(record.get("Discht").unwrap_or(&FieldValue::Character(None)));
        rec.adm_wt = field_to_f64(record.get("AdmWt").unwrap_or(&FieldValue::Numeric(None)));
        rec.act_los = field_to_i32(record.get("ActLOS").unwrap_or(&FieldValue::Numeric(None)));
        rec.age = field_to_string(record.get("Age").unwrap_or(&FieldValue::Character(None)));
        rec.age_day =
            field_to_string(record.get("AgeDay").unwrap_or(&FieldValue::Character(None)));
        rec.pdx = field_to_string(record.get("PDx").unwrap_or(&FieldValue::Character(None)));

        rec.sdx = (1..=12)
            .map(|i| {
                field_to_string(
                    record
                        .get(&format!("SDx{}", i))
                        .unwrap_or(&FieldValue::Character(None)),
                )
            })
            .filter(|s| !s.is_empty())
            .collect();

        rec.procs = (1..=20)
            .map(|i| {
                field_to_string(
                    record
                        .get(&format!("Proc{}", i))
                        .unwrap_or(&FieldValue::Character(None)),
                )
            })
            .filter(|s| !s.is_empty())
            .collect();

        // Output fields
        rec.leave_day =
            field_to_i32(record.get("LeaveDay").unwrap_or(&FieldValue::Numeric(None)));
        rec.drg = field_to_string(record.get("DRG").unwrap_or(&FieldValue::Character(None)));
        rec.mdc = field_to_string(record.get("MDC").unwrap_or(&FieldValue::Character(None)));
        rec.err = field_to_i32(record.get("Err").unwrap_or(&FieldValue::Numeric(None)));
        rec.warn = field_to_i32(record.get("Warn").unwrap_or(&FieldValue::Numeric(None)));
        rec.rw = field_to_f64(record.get("RW").unwrap_or(&FieldValue::Numeric(None)));
        rec.ot = field_to_i32(record.get("OT").unwrap_or(&FieldValue::Numeric(None)));
        rec.wtlos = field_to_f64(record.get("WTLOS").unwrap_or(&FieldValue::Numeric(None)));
        rec.adjrw = field_to_f64(record.get("ADJRW").unwrap_or(&FieldValue::Numeric(None)));

        results.push(rec);
    }

    Ok(results)
}

/// Convert a task input into an ExeDbfRecord ready for writing.
pub fn input_to_record(
    pdx: &str,
    sdx: &[String],
    procs: &[String],
    age: i32,
    age_in_days: Option<i32>,
    sex: &str,
    discharge_type: i32,
    los: i32,
    admission_weight: Option<f64>,
) -> ExeDbfRecord {
    // Synthesize dates from LOS (same approach as TypeScript version)
    let reference_date = "20260101";
    let date_adm = reference_date.to_string();

    // Calculate discharge date
    let adm = chrono::NaiveDate::parse_from_str(reference_date, "%Y%m%d").unwrap();
    let dsc = adm + chrono::Duration::days(los as i64);
    let date_dsc = dsc.format("%Y%m%d").to_string();

    // Calculate DOB from age
    let dob_date = adm - chrono::Duration::days(age as i64 * 365);
    let dob = dob_date.format("%Y%m%d").to_string();

    let sex_code = match sex {
        "M" => "1",
        "F" => "2",
        _ => "1",
    };

    ExeDbfRecord {
        dob,
        sex: sex_code.to_string(),
        date_adm,
        date_dsc,
        time_adm: "0000".to_string(),
        time_dsc: "0000".to_string(),
        discht: format!("{}", discharge_type % 10),
        adm_wt: admission_weight.unwrap_or(0.0),
        act_los: los,
        age: format!("{}", age),
        age_day: age_in_days.map(|d| format!("{}", d)).unwrap_or_default(),
        pdx: pdx.to_string(),
        sdx: sdx.to_vec(),
        procs: procs.to_vec(),
        ..Default::default()
    }
}
