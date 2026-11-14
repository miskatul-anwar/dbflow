//! Native integration tests of the engine (the Vitest suite covers the same code through WebAssembly).

use dbflow_core::pipeline::{process, InputFile};
use dbflow_core::{call, csv, normalization as nf, table::Table};
use serde_json::{json, Value};

fn file(name: &str, text: &str) -> InputFile {
    InputFile {
        name: name.into(),
        bytes: text.as_bytes().to_vec(),
    }
}

const ALL_OPS: [&str; 19] = [
    "CSV Data Loader",
    "Data Cleaner",
    "Atomicity Checker",
    "Data Type Profiler",
    "Column Classifier",
    "1NF",
    "FD Miner",
    "Key Discovery",
    "Cardinality Analyzer",
    "2NF",
    "3NF",
    "BCNF",
    "Lossless Check",
    "Dependency Preservation",
    "Relation Detector",
    "ERD Generator",
    "Relation Generator",
    "SQL Generator",
    "Report Generator",
];

fn ops() -> Vec<String> {
    ALL_OPS.iter().map(|s| s.to_string()).collect()
}

#[test]
fn csv_parsing_handles_quotes_bom_and_null_tokens() {
    let text = "\u{feff}id;name;note\n1;\"Ann; Lee\";N/A\n2;Bob;\"said \"\"hi\"\"\"\n";
    let r = csv::parse(text, &Default::default()).unwrap();
    assert_eq!(r.headers, vec!["id", "name", "note"]);
    assert_eq!(r.metadata.delimiter, ";");
    assert_eq!(r.data.get(0, "name").js_string(), "Ann; Lee");
    assert_eq!(r.data.get(0, "note").js_string(), "");
    assert_eq!(r.data.get(1, "note").js_string(), "said \"hi\"");
}

#[test]
fn bcnf_loses_the_classic_dependency() {
    // Enrollment(student, course, teacher): {student, course} -> teacher, teacher -> course.
    let fds: Vec<dbflow_core::types::Fd> = serde_json::from_value(json!([
        {"lhs": ["student", "course"], "rhs": "teacher"},
        {"lhs": ["teacher"], "rhs": "course"}
    ]))
    .unwrap();
    let attrs: Vec<String> = ["student", "course", "teacher"].iter().map(|s| s.to_string()).collect();
    let keys = nf::find_candidate_keys(&attrs, &fds);
    assert!(keys.contains(&vec!["course".to_string(), "student".to_string()]));
    let res = nf::decompose_bcnf_detailed(&attrs, &Table::new(), "Enrollment", &fds);
    assert!(res.is_lossless);
    assert!(!res.preservation.is_preserved);
}

#[test]
fn single_file_run_produces_every_output() {
    let mut csv = String::from("order_id,customer,city,country,amount\n");
    for i in 0..60 {
        let c = i % 6;
        csv += &format!(
            "{},Customer {c},City {},Country {},{}.50\n",
            100 + i,
            c % 3,
            c % 3,
            i * 3
        );
    }
    let r = process(
        vec![file("orders.csv", &csv)],
        ops(),
        json!({"sqlDialect": "postgresql"}),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    assert_eq!(r["engine"], json!("rust-native"));
    let names: Vec<&str> = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    for expected in [
        "database_schema.sql",
        "erd_diagram.mmd",
        "process_report.md",
        "functional_dependencies.json",
    ] {
        assert!(names.contains(&expected), "missing {expected}: {names:?}");
    }
    assert!(
        r["relations"].as_array().unwrap().len() >= 2,
        "3NF should split city/country out"
    );
    assert_eq!(r["losslessCheck"]["isLossless"], json!(true));
    let sql = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "database_schema.sql")
        .unwrap();
    assert!(sql["content"].as_str().unwrap().contains("-- Dialect: POSTGRESQL"));
}

#[test]
fn multi_file_run_detects_the_foreign_key() {
    let customers: String = std::iter::once("customer_id,name\n".to_string())
        .chain((1..=8).map(|i| format!("{i},Name {i}\n")))
        .collect();
    let orders: String = std::iter::once("order_id,customer_id,total\n".to_string())
        .chain((0..24).map(|i| format!("{},{},{}.00\n", 500 + i, (i % 8) + 1, 10 + i)))
        .collect();
    let r = process(
        vec![file("customers.csv", &customers), file("orders.csv", &orders)],
        ops(),
        json!({}),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    assert_eq!(r["isMultiTable"], json!(true));
    let links = r["crossTableRelationships"].as_array().unwrap();
    assert!(
        links.iter().any(|l| l["sourceTable"] == "orders"
            && l["sourceColumn"] == "customer_id"
            && l["targetTable"] == "customers"),
        "{links:?}"
    );
    assert!(r["sqlStatements"]
        .as_str()
        .unwrap()
        .contains("REFERENCES \"customers\""));
}

#[test]
fn failures_are_reported_not_thrown() {
    let r = process(vec![file("empty.csv", "   \n")], ops(), json!({}));
    assert_eq!(r["success"], json!(false));
    assert!(r["error"].as_str().unwrap().contains("Empty CSV file"));
    let none = process(vec![], ops(), json!({}));
    assert_eq!(none["error"], json!("No files uploaded for processing."));
}

#[test]
fn json_api_dispatches_and_rejects_unknown_methods() {
    let closure = call(
        "nf.computeClosure",
        json!([["a"], [{"lhs": ["a"], "rhs": "b"}, {"lhs": ["b"], "rhs": "c"}]]),
    )
    .unwrap();
    assert_eq!(closure, json!(["a", "b", "c"]));
    assert_eq!(
        call("ddl.quoteIdentifier", json!(["order date", "mysql"])).unwrap(),
        Value::String("`order_date`".into())
    );
    assert!(call("no.such.method", json!([])).is_err());
}
