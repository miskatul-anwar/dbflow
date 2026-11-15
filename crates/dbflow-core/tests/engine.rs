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
    "Data Loader",
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
    assert!(
        r["error"].as_str().unwrap().contains("empty.csv is empty"),
        "{}",
        r["error"]
    );
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

/// A small three-file retail export: orders reference customers and products, and copy facts about them.
fn retail_files(rows: usize) -> Vec<InputFile> {
    let cities = ["Lisbon", "Dhaka", "Osaka", "Cairo"];
    let mut customers = String::from("customer_id,customer_name,city_code,city_name\n");
    for c in 1..=40 {
        let city = c % 4;
        customers += &format!("{c},Customer {c},C{city},{}\n", cities[city]);
    }
    let mut products = String::from("product_id,product_name,unit_price\n");
    for p in 1..=30 {
        products += &format!("{p},Product {p},{}.99\n", p * 3);
    }
    let mut orders = String::from("order_id,customer_id,customer_name,product_id,quantity\n");
    for o in 1..=rows {
        let c = 1 + (o * 7) % 40;
        orders += &format!("{o},{c},Customer {c},{},{}\n", 1 + (o * 11) % 30, 1 + o % 5);
    }
    vec![
        file("customers.csv", &customers),
        file("products.csv", &products),
        file("orders.csv", &orders),
    ]
}

/// The result without the parts that legitimately differ between runs (timings, log time stamps).
fn comparable(mut v: Value) -> Value {
    fn scrub(v: &mut Value) {
        match v {
            Value::Object(m) => {
                for key in [
                    "duration",
                    "processDuration",
                    "logs",
                    "performanceMetrics",
                    "totalDuration",
                ] {
                    m.remove(key);
                }
                m.values_mut().for_each(scrub);
            }
            Value::Array(a) => a.iter_mut().for_each(scrub),
            // Generated files carry a time stamp line.
            Value::String(t) if t.contains("Generated: ") => {
                *t = t
                    .lines()
                    .filter(|l| !l.contains("Generated: "))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            _ => {}
        }
    }
    scrub(&mut v);
    v
}

#[test]
fn parallel_and_sequential_multi_file_runs_give_the_same_result() {
    let parallel = process(retail_files(300), ops(), json!({ "maxParallelFiles": 4 }));
    let sequential = process(retail_files(300), ops(), json!({ "maxParallelFiles": 1 }));
    assert_eq!(parallel["success"], json!(true), "{parallel}");
    let (a, b) = (comparable(parallel), comparable(sequential));
    assert!(
        a == b,
        "first difference at {}",
        first_difference(&a, &b, "$".into()).unwrap_or_default()
    );
}

/// Path and values of the first place two JSON values differ.
fn first_difference(a: &Value, b: &Value, path: String) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => {
                        if let Some(d) = first_difference(v, w, format!("{path}.{k}")) {
                            return Some(d);
                        }
                    }
                    None => return Some(format!("{path}.{k} missing on the right")),
                }
            }
            y.keys()
                .find(|k| !x.contains_key(*k))
                .map(|k| format!("{path}.{k} missing on the left"))
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                if let Some(d) = first_difference(v, w, format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            (x.len() != y.len()).then(|| format!("{path}: {} vs {} items", x.len(), y.len()))
        }
        _ => (a != b).then(|| {
            format!(
                "{path}: {} vs {}",
                a.to_string().chars().take(200).collect::<String>(),
                b.to_string().chars().take(200).collect::<String>()
            )
        }),
    }
}

#[test]
fn multi_file_run_finds_the_foreign_keys() {
    let r = process(retail_files(300), ops(), json!({}));
    let links: Vec<(String, String)> = r["crossTableRelationships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["sourceColumn"].as_str().unwrap().into(),
                l["targetTable"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert!(links.contains(&("customer_id".into(), "customers".into())), "{links:?}");
    assert!(links.contains(&("product_id".into(), "products".into())), "{links:?}");
}

#[test]
fn cancelling_a_parallel_run_stops_it() {
    let cancel_now = || true;
    let none = |_: dbflow_core::analysis::Progress| {};
    let p = dbflow_core::pipeline::prepare(retail_files(300), ops(), json!({}), &none, &cancel_now);
    let out: Value = serde_json::from_str(&dbflow_core::pipeline::finish(p, None, None, &none)).unwrap();
    assert_eq!(out["success"], json!(false));
    assert!(out["error"].as_str().unwrap().contains("cancelled"), "{out}");
}

#[test]
fn rule_naming_names_every_new_relation_from_its_key() {
    let r = process(retail_files(300), ops(), json!({ "namingMode": "rules" }));
    let mut sources = std::collections::HashSet::new();
    for rel in r["relations"].as_array().unwrap() {
        let source = rel["nameSource"].as_str().unwrap_or("").to_string();
        assert_ne!(source, "placeholder", "unnamed relation: {rel}");
        sources.insert(source);
    }
    assert!(sources.contains("rule"), "{sources:?}");
    // The city facts copied into customers become their own relation, named after its key.
    assert!(r["relations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|rel| rel["displayName"] == json!("City")));
}

#[test]
fn candidate_keys_are_all_found_without_enumerating_subsets() {
    // 30 attributes, each one a key (a chain of equivalences): the old subset lattice was 2^30 here.
    let attrs: Vec<String> = (0..30).map(|i| format!("a{i:02}")).collect();
    let fds: Vec<dbflow_core::types::Fd> = (0..30)
        .map(|i| {
            serde_json::from_value(json!({"lhs": [attrs[i].clone()], "rhs": attrs[(i + 1) % 30].clone()})).unwrap()
        })
        .collect();
    let keys = nf::find_candidate_keys(&attrs, &fds);
    assert_eq!(keys.len(), 30);
    assert!(keys.iter().all(|k| k.len() == 1));
}

#[test]
fn candidate_keys_match_the_textbook_example() {
    // R(A, B, C, D) with AB -> C, C -> D, D -> A: keys are AB, BC and BD.
    let fds: Vec<dbflow_core::types::Fd> = serde_json::from_value(json!([
        {"lhs": ["A", "B"], "rhs": "C"},
        {"lhs": ["C"], "rhs": "D"},
        {"lhs": ["D"], "rhs": "A"}
    ]))
    .unwrap();
    let attrs: Vec<String> = ["A", "B", "C", "D"].iter().map(|s| s.to_string()).collect();
    let keys = nf::find_candidate_keys(&attrs, &fds);
    let keys: Vec<String> = keys.iter().map(|k| k.join("")).collect();
    assert_eq!(keys, vec!["AB", "BC", "BD"]);
}

#[test]
fn large_inputs_get_a_compact_result_with_row_counts() {
    // 20,000 rows × 52 columns > 999,999 cells.
    let mut text = String::from("id,group_id,group_name");
    for c in 0..49 {
        text += &format!(",m{c}");
    }
    text.push('\n');
    for r in 0..20_000u64 {
        let g = r % 7;
        text += &format!("{r},{g},Group {g}");
        for c in 0..49u64 {
            text += &format!(",{}", (r * 7_919 + c * 104_729) % 9_999_991);
        }
        text.push('\n');
    }
    let r = process(vec![file("wide.csv", &text)], ops(), json!({}));
    assert_eq!(r["success"], json!(true));
    let rels = r["relations"].as_array().unwrap();
    assert!(rels
        .iter()
        .all(|rel| rel["rows"].as_array().map(|a| a.is_empty()).unwrap_or(false)));
    let main = rels
        .iter()
        .find(|rel| rel["primaryKey"] == json!(["id"]))
        .expect("main relation keeps the id key");
    assert_eq!(main["rowCount"], json!(20_000));
    let group = rels
        .iter()
        .find(|rel| rel["primaryKey"] == json!(["group_id"]))
        .expect("group relation");
    assert_eq!(group["rowCount"], json!(7));
    // The data is still exported, once per relation.
    let files: Vec<&str> = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(
        files
            .iter()
            .any(|f| f.starts_with("relation_3nf_") && f.ends_with(".csv")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("relation_bcnf_")),
        "BCNF CSVs repeat the 3NF ones: {files:?}"
    );
}

#[test]
fn a_composite_key_survives_sampling() {
    // 30,000 order lines (3 per order): in a 10,000-row sample order_id looks almost unique, yet
    // (order_id, line_no) must still be found as the key on the full data.
    let mut text = String::from("order_id,line_no,product_id,quantity,amount\n");
    for i in 0..30_000u64 {
        text += &format!(
            "{},{},{},{},{}.{:02}\n",
            1 + i / 3,
            1 + i % 3,
            1 + (i * 37) % 999,
            1 + i % 9,
            (i * 7_919) % 99_999,
            i % 100
        );
    }
    let r = process(vec![file("lines.csv", &text)], ops(), json!({}));
    let keys: Vec<Value> = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == json!("keys_analysis.json"))
        .map(|f| f["content"]["candidateKeys"].as_array().unwrap().clone())
        .unwrap();
    assert!(keys.contains(&json!(["line_no", "order_id"])), "{keys:?}");
}

#[test]
fn a_determinant_that_only_looks_unique_in_the_sample_is_still_used() {
    // 20,000 orders by 5,000 customers: in a 500-row sample customer_id is ~96 % unique, on the data 25 %.
    // customer_id -> customer_name must still be found and split out.
    let mut text = String::from("order_id,customer_id,customer_name,amount\n");
    for i in 0..20_000u64 {
        let c = (i * 7_919) % 5_000;
        text += &format!("{i},{c},Customer {c},{}\n", (i * 104_729) % 9_999_991);
    }
    let r = process(vec![file("orders.csv", &text)], ops(), json!({ "fdSampleSize": 500 }));
    let rels = r["relations"].as_array().unwrap();
    assert!(
        rels.iter().any(|rel| rel["primaryKey"] == json!(["customer_id"])),
        "{:?}",
        rels.iter().map(|r| r["primaryKey"].clone()).collect::<Vec<_>>()
    );
}

#[test]
fn split_out_relations_prefer_an_identifier_column_as_key() {
    // city_code, city_name and city_population determine each other; the city relation should be keyed by the code.
    let mut text = String::from("order_id,city_population,city_name,city_code,amount\n");
    for i in 0..400u64 {
        let c = i % 9;
        text += &format!("{i},{},City {c},C{c},{}\n", 10_000 + c * 7_919, (i * 104_729) % 99_991);
    }
    let r = process(vec![file("orders.csv", &text)], ops(), json!({}));
    let city = r["relations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|rel| {
            rel["attributes"].as_array().unwrap().contains(&json!("city_name"))
                && rel["primaryKey"] != json!(["order_id"])
        })
        .expect("city relation");
    assert_eq!(city["primaryKey"], json!(["city_code"]), "{city}");
}

#[test]
fn the_former_csv_data_loader_name_still_runs() {
    let old = process(
        vec![file(
            "t.csv",
            "id,city,region\n1,A,X\n2,B,Y\n3,A,X\n4,C,Y\n5,B,Y\n6,A,X\n7,C,Y\n8,A,X\n9,B,Y\n10,C,Y\n11,A,X\n12,B,Y\n",
        )],
        vec![
            "CSV Data Loader".into(),
            "FD Miner".into(),
            "Key Discovery".into(),
            "3NF".into(),
        ],
        json!({}),
    );
    assert_eq!(old["success"], json!(true), "{old}");
}

// ----- input checks ---------------------------------------------------------------------------

fn customers_csv(edit: bool) -> String {
    let cities = [("Dhaka", "BD"), ("Lisbon", "PT"), ("Paris", "FR"), ("Delhi", "IN")];
    let mut out = String::from("customer_id,name,city,country\n");
    for i in 1..=60 {
        let (mut city, mut country) = cities[i % 4];
        if edit && [5, 17, 33].contains(&i) {
            (city, country) = ("Rome", "IT");
        }
        out.push_str(&format!("{i},Name {i},{city},{country}\n"));
    }
    if edit {
        out.push_str("61,Name 61,Paris,FR\n");
    }
    out
}

fn orders_csv() -> String {
    std::iter::once("order_id,customer_id,total\n".to_string())
        .chain((0..200).map(|i| format!("{},{},{}.99\n", 1000 + i, (i % 60) + 1, 10 + i)))
        .collect()
}

fn table_names(r: &Value) -> Vec<String> {
    r["tables"]
        .as_array()
        .map(|t| {
            t.iter()
                .map(|t| t["tableName"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn identical_files_are_analysed_once() {
    let r = process(
        vec![
            file("customers.csv", &customers_csv(false)),
            file("clients.csv", &customers_csv(false).replace('\n', "\r\n")),
            file("orders.csv", &orders_csv()),
        ],
        ops(),
        json!({ "namingMode": "rules" }),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    assert_eq!(table_names(&r), vec!["customers", "orders"]);
    let d = &r["inputChecks"]["duplicates"][0];
    assert_eq!(
        (d["file"].as_str(), d["sameAs"].as_str()),
        (Some("clients.csv"), Some("customers.csv"))
    );
    // No foreign key between a file and its own copy.
    let links = r["crossTableRelationships"].as_array().unwrap();
    assert!(links.iter().all(|l| l["sourceTable"] != "clients"), "{links:?}");
}

#[test]
fn files_with_the_same_name_are_told_apart() {
    let products = "product_id,title,category\n1,P1,A\n2,P2,B\n3,P3,C\n4,P4,A\n";
    let r = process(
        vec![
            file("data.csv", &customers_csv(false)),
            file("data.csv", products),
            file("orders.csv", &orders_csv()),
        ],
        ops(),
        json!({ "namingMode": "rules" }),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    assert_eq!(table_names(&r), vec!["data", "data_2", "orders"]);
    let logs = r["logs"].to_string();
    assert!(
        logs.contains("data.csv (file 1)") && logs.contains("data.csv (file 2)"),
        "{logs}"
    );
}

#[test]
fn overlapping_files_are_reported_and_merged_on_request() {
    let inputs = || {
        vec![
            file("customers.csv", &customers_csv(false)),
            file("customers_v2.csv", &customers_csv(true)),
            file("orders.csv", &orders_csv()),
        ]
    };
    let r = process(inputs(), ops(), json!({ "namingMode": "rules" }));
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    let o = &r["inputChecks"]["overlaps"][0];
    assert_eq!(o["files"], json!(["customers.csv", "customers_v2.csv"]));
    assert_eq!(o["sharedRows"], json!(57));
    assert_eq!(o["key"], json!("customer_id"));
    assert_eq!(o["sharedKeys"], json!(60));
    assert_eq!(o["conflicts"], json!(3));
    assert_eq!(o["conflictKeys"], json!(["5", "17", "33"]));
    assert_eq!(table_names(&r), vec!["customers", "customers_v2", "orders"]);

    let m = process(
        inputs(),
        ops(),
        json!({ "namingMode": "rules", "mergeOverlappingFiles": true }),
    );
    assert_eq!(m["success"], json!(true), "{}", m["error"]);
    let merged = &m["inputChecks"]["merged"][0];
    assert_eq!(merged["into"], json!("customers.csv"));
    assert_eq!(merged["from"], json!(["customers_v2.csv"]));
    assert_eq!(merged["rows"], json!(61));
    assert_eq!(merged["conflictsResolved"], json!(3));
    assert_eq!(table_names(&m), vec!["customers", "orders"]);
    let cust = &m["tables"][0];
    assert_eq!(cust["primaryKey"], json!(["customer_id"]), "{}", cust["primaryKey"]);
}

#[test]
fn files_that_are_not_tables_are_skipped_with_their_reason() {
    let cpp = "#include <iostream>\nint main(){\n  std::cout << \"hi, there\" << std::endl;\n  return 0;\n}\n";
    let binary: Vec<u8> = (0u8..=255).cycle().take(5000).collect();
    let r = process(
        vec![
            file("report.csv", "%PDF-1.4\n1 0 obj<<>>endobj\n%%EOF\n"),
            file("payload.csv", r#"[{"id":1,"name":"a"},{"id":2,"name":"b"}]"#),
            file("lines.csv", "{\"id\":1}\n{\"id\":2}\n"),
            file("main.csv", cpp),
            InputFile {
                name: "blob.csv".into(),
                bytes: binary,
            },
            file("page.csv", "<html><body><p>hi</p></body></html>"),
            file("empty.csv", "  \n"),
            InputFile {
                name: "photo.xlsx".into(),
                bytes: b"\x89PNG\r\n\x1a\n0000".to_vec(),
            },
            file("orders.csv", &orders_csv()),
        ],
        ops(),
        json!({ "namingMode": "rules" }),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    assert_eq!(table_names(&r).len(), 0, "single table left: single-table result");
    let reasons: Vec<String> = r["inputChecks"]["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["reason"].as_str().unwrap().to_string())
        .collect();
    let expect = [
        "report.csv is a PDF document",
        "payload.csv contains JSON",
        "lines.csv contains JSON",
        "main.csv looks like program source code",
        "blob.csv contains binary data",
        "page.csv contains XML or HTML markup",
        "empty.csv is empty",
        "photo.xlsx is a PNG image",
    ];
    assert_eq!(reasons.len(), expect.len(), "{reasons:?}");
    for (got, want) in reasons.iter().zip(expect) {
        assert!(got.starts_with(want), "{got} / {want}");
    }
}

#[test]
fn a_single_bad_file_fails_with_its_reason() {
    let r = process(vec![file("payload.csv", r#"[{"id":1}]"#)], ops(), json!({}));
    assert_eq!(r["success"], json!(false));
    assert!(
        r["error"].as_str().unwrap().starts_with("payload.csv contains JSON"),
        "{}",
        r["error"]
    );
    let two = process(
        vec![file("a.csv", "%PDF-1.4"), file("b.csv", r#"{"x":1}"#)],
        ops(),
        json!({}),
    );
    assert_eq!(two["success"], json!(false));
    let e = two["error"].as_str().unwrap();
    assert!(e.contains("a.csv is a PDF") && e.contains("b.csv contains JSON"), "{e}");
    assert_eq!(two["inputChecks"]["skipped"].as_array().unwrap().len(), 2);
}

fn zip_of(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn a_compression_bomb_is_refused_before_it_is_read() {
    let mut sheet = b"<worksheet><sheetData><row><c t=\"inlineStr\"><is><t>".to_vec();
    sheet.extend(std::iter::repeat_n(b'A', 64 << 20));
    sheet.extend(b"</t></is></c></row></sheetData></worksheet>");
    let bomb = zip_of(&[
        ("[Content_Types].xml", b"<Types/>".to_vec()),
        ("xl/workbook.xml", b"<workbook/>".to_vec()),
        ("xl/worksheets/sheet1.xml", sheet),
    ]);
    assert!(bomb.len() < 1 << 20, "{}", bomb.len());
    let t = std::time::Instant::now();
    let r = process(
        vec![
            InputFile {
                name: "bomb.xlsx".into(),
                bytes: bomb,
            },
            file("orders.csv", &orders_csv()),
        ],
        ops(),
        json!({}),
    );
    assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    let reason = r["inputChecks"]["skipped"][0]["reason"].as_str().unwrap();
    assert!(reason.contains("compression bomb"), "{reason}");
}

#[test]
fn office_files_that_are_not_spreadsheets_are_named() {
    let docx = zip_of(&[
        ("[Content_Types].xml", b"<Types/>".to_vec()),
        ("word/document.xml", b"<w:document/>".to_vec()),
    ]);
    let plain = zip_of(&[("readme.txt", b"hello".to_vec())]);
    let r = process(
        vec![
            InputFile {
                name: "letter.xlsx".into(),
                bytes: docx,
            },
            InputFile {
                name: "bundle.csv".into(),
                bytes: plain,
            },
            file("orders.csv", &orders_csv()),
        ],
        ops(),
        json!({}),
    );
    let reasons = r["inputChecks"]["skipped"].to_string();
    assert!(reasons.contains("letter.xlsx is a Word document"), "{reasons}");
    assert!(
        reasons.contains("bundle.csv is not a table") && reasons.contains("ZIP archive"),
        "{reasons}"
    );
}

// ----- 4NF --------------------------------------------------------------------------------------

fn ops_with_4nf() -> Vec<String> {
    let mut o = ops();
    let at = o.iter().position(|x| x == "BCNF").unwrap() + 1;
    o.insert(at, "4NF".into());
    o
}

/// A course's teachers and books are independent: every teacher is listed with every book. Teachers and books
/// are shared between courses, so no functional dependency holds (the table is in BCNF).
fn course_teacher_book() -> String {
    let mut out = String::from("course,teacher,book\n");
    for (course, teachers, books) in [
        ("Databases", &["Ann", "Bob"][..], &["Date", "Ullman", "Lang"][..]),
        ("Physics", &["Cy", "Ann"][..], &["Feynman", "Date"][..]),
        ("Algebra", &["Bob", "Dee"][..], &["Lang", "Artin"][..]),
        ("Biology", &["Cy", "Eve", "Dee"][..], &["Raven", "Feynman"][..]),
    ] {
        for t in teachers {
            for b in books {
                out.push_str(&format!("{course},{t},{b}\n"));
            }
        }
    }
    out
}

fn attr_sets(v: &Value) -> Vec<Vec<String>> {
    let mut s: Vec<Vec<String>> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let mut a: Vec<String> = r["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
            a.sort();
            a
        })
        .collect();
    s.sort();
    s
}

#[test]
fn fourth_normal_form_splits_independent_multivalued_facts() {
    let r = process(
        vec![file("ctb.csv", &course_teacher_book())],
        ops_with_4nf(),
        json!({ "namingMode": "rules" }),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    // No functional dependency holds, so BCNF keeps the one all-key table.
    assert_eq!(attr_sets(&r["bcnf"]), vec![vec!["book", "course", "teacher"]]);
    // 4NF splits it on course ->> teacher | book.
    let expected = vec![
        vec!["book".to_string(), "course".to_string()],
        vec!["course".to_string(), "teacher".to_string()],
    ];
    assert_eq!(attr_sets(&r["relations4NF"]), expected);
    assert_eq!(attr_sets(&r["relations"]), expected, "the 4NF set is the final schema");
    let m = &r["multivaluedDependencies"][0];
    assert_eq!(m["lhs"], json!(["course"]));
    assert_eq!(r["multivaluedDependencies"].as_array().unwrap().len(), 1);
    // The split is proven lossless by the chase with the MVD, and confirmed on the data.
    assert_eq!(r["losslessCheck"]["isLossless"], json!(true), "{}", r["losslessCheck"]);
    assert_eq!(r["losslessCheck"]["chaseLossless"], json!(true));
    let files: Vec<&str> = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(
        files.contains(&"relations_4nf.json") && files.contains(&"multivalued_dependencies.json"),
        "{files:?}"
    );
    let report = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "process_report.md")
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        report.contains("Fourth Normal Form (4NF)") && report.contains("{course} ->>"),
        "{report}"
    );
    let sql = r["outputFiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["type"] == "sql")
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(sql.matches("CREATE TABLE").count(), 2, "{sql}");
}

#[test]
fn fourth_normal_form_leaves_tables_without_multivalued_dependencies_alone() {
    let r = process(
        vec![file("orders.csv", &orders_csv())],
        ops_with_4nf(),
        json!({ "namingMode": "rules" }),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    assert_eq!(r["multivaluedDependencies"], json!([]));
    assert_eq!(attr_sets(&r["relations4NF"]), attr_sets(&r["bcnf"]));
}

#[test]
fn fourth_normal_form_in_a_multi_file_run() {
    let r = process(
        vec![
            file("ctb.csv", &course_teacher_book()),
            file("orders.csv", &orders_csv()),
        ],
        ops_with_4nf(),
        json!({ "namingMode": "rules" }),
    );
    assert_eq!(r["success"], json!(true), "{}", r["error"]);
    let ctb = r["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tableName"] == "ctb")
        .unwrap();
    assert_eq!(ctb["multivaluedDependencies"].as_array().unwrap().len(), 1);
    assert_eq!(attr_sets(&ctb["fourthNormalForm"]).len(), 2);
    assert_eq!(
        attr_sets(&ctb["thirdNormalForm"]).len(),
        2,
        "the final set (3NF slot) is the 4NF one"
    );
}
