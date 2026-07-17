use crate::types::{HeapBlock, Region, RegionEntry};
use quick_junit::{NonSuccessKind, Report, TestCase, TestCaseStatus, TestSuite};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub message: Option<String>,
}

pub enum FormatType {
    Json,
    CSV,
    Junit,
}

pub fn heap_to_json(blocks: Vec<HeapBlock>) -> Result<String, serde_json::Error> {
    let json_str = serde_json::to_string_pretty(&blocks)?;
    Ok(json_str)
}

pub fn region_to_json(
    regions: Vec<Region>,
    labels: Vec<&str>,
) -> Result<String, serde_json::Error> {
    let entries: Vec<RegionEntry> = regions
        .iter()
        .zip(labels.iter())
        .map(|(r, l)| RegionEntry {
            base: r.base,
            size: r.size,
            state: r.state.clone(),
            kind: r.kind.clone(),
            protect: r.protect.clone(),
            name: r.name.clone(),
            label: l.to_string(),
        })
        .collect();
    let json_str = serde_json::to_string_pretty(&entries).unwrap();
    Ok(json_str)
}

pub fn heap_to_json_file<P: AsRef<Path>>(
    file_path: P,
    blocks: Vec<HeapBlock>,
) -> Result<(), Box<dyn Error>> {
    let json_str = serde_json::to_string_pretty(&blocks)?;
    fs::write(file_path, json_str)?;
    Ok(())
}

pub fn region_to_json_file<P: AsRef<Path>>(
    file_path: P,
    regions: Vec<Region>,
    labels: Vec<&str>,
) -> Result<(), Box<dyn Error>> {
    let entries: Vec<RegionEntry> = regions
        .iter()
        .zip(labels.iter())
        .map(|(r, l)| RegionEntry {
            base: r.base,
            size: r.size,
            state: r.state.clone(),
            kind: r.kind.clone(),
            protect: r.protect.clone(),
            name: r.name.clone(),
            label: l.to_string(),
        })
        .collect();
    let json_str = serde_json::to_string_pretty(&entries)?;
    fs::write(file_path, json_str)?;
    Ok(())
}

pub fn heap_to_csv_file<P: AsRef<Path>>(
    file_path: P,
    blocks: Vec<HeapBlock>,
) -> Result<(), Box<dyn Error>> {
    let file = File::create(file_path)?;

    let mut wtr = csv::Writer::from_writer(file);

    for block in blocks {
        wtr.serialize(&block)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn region_to_csv_file<P: AsRef<Path>>(
    file_path: P,
    regions: Vec<Region>,
) -> Result<(), Box<dyn Error>> {
    let file = File::create(file_path)?;

    let mut wtr = csv::Writer::from_writer(file);

    for region in regions {
        wtr.serialize(&region)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn heap_to_junit_file<P: AsRef<Path>>(
    file_path: P,
    results: Vec<CheckResult>,
) -> Result<(), Box<dyn Error>> {
    let mut report = Report::new("mvis-ci-checks");
    let mut suite = TestSuite::new("memory-checks");

    for r in results {
        let status = if r.passed {
            TestCaseStatus::success()
        } else {
            let mut s = TestCaseStatus::non_success(NonSuccessKind::Failure);
            if let Some(msg) = r.message {
                s.set_message(msg);
            }
            s
        };
        suite.add_test_case(TestCase::new(r.name, status));
    }

    report.add_test_suite(suite);
    let xml = report.to_string()?;
    fs::write(file_path, xml)?;
    Ok(())
}
