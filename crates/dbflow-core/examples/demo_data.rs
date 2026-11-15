//! Generates the DBFlow demonstration dataset: nine related, denormalised CSV exports of a retail
//! business (customers, suppliers, products, stores, employees, orders, order lines, shipments and
//! payments), each with 99 columns.
//!
//! ```text
//! cargo run --release -p dbflow-core --example demo_data -- <out-dir> [--scale small|full]
//! ```
//!
//! `full` (default) writes 9,999 to 99,999 rows per table; `small` writes 999 rows per table for
//! quick checks. The output is deterministic (seeded hashing, no randomness from the OS), so every
//! run produces the same bytes.
//!
//! `--single ROWS COLS` instead writes one order table, `orders_ROWSxCOLS.csv`, for scaling series:
//! an order key, a customer (name, segment), the customer's city and its region (the dependencies 3NF
//! splits out), then measures up to COLS columns (at least 9).
//!
//! Every table mixes three kinds of columns, so normalisation has real work to do:
//!  - a key and, where relevant, foreign keys into the other files (cross-file inclusion
//!    dependencies that DBFlow should find);
//!  - embedded entities copied into each row (e.g. `city_code → city_name, region_code` and
//!    `region_code → region_name`), i.e. transitive dependencies that 3NF must split out;
//!  - many measures that depend on the key only.
use std::fmt::Write as _;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

const COLUMNS: usize = 99;

/// SplitMix64 finaliser: a stable hash of (table salt, id, column salt).
fn h(a: u64, b: u64, c: u64) -> u64 {
    let mut z = a
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(b.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(c.wrapping_mul(0x94D0_49BB_1331_11EB))
        .wrapping_add(0x2545_F491_4F6C_DD1D);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn pick<'a>(list: &'a [&'a str], x: u64) -> &'a str {
    list[(x % list.len() as u64) as usize]
}

const FIRST: &[&str] = &[
    "Ana", "Raj", "Mei", "Omar", "Lena", "Kofi", "Sara", "Ivan", "Nia", "Tom", "Aiko", "Luis", "Zara", "Ben", "Ines",
    "Yusuf", "Eva", "Arjun", "Lina", "Hugo", "Maya", "Noah", "Rina", "Sami", "Ella", "Dev", "Iris", "Jonas", "Leah",
    "Karim",
];
const LAST: &[&str] = &[
    "Silva", "Khan", "Chen", "Haddad", "Novak", "Mensah", "Costa", "Petrov", "Okafor", "Reed", "Sato", "Garcia", "Ali",
    "Moore", "Rossi", "Demir", "Berg", "Patel", "Haas", "Laurent", "Ibrahim", "Kim", "Lopez", "Nakamura", "Ahmed",
    "Fischer", "Rahman", "Dubois", "Kowalski", "Yilmaz",
];
const CITY: &[&str] = &[
    "Lisbon",
    "Dhaka",
    "Osaka",
    "Cairo",
    "Lyon",
    "Accra",
    "Porto",
    "Kazan",
    "Lagos",
    "Leeds",
    "Kyoto",
    "Seville",
    "Karachi",
    "Austin",
    "Turin",
    "Izmir",
    "Bergen",
    "Pune",
    "Graz",
    "Nantes",
    "Tunis",
    "Busan",
    "Quito",
    "Nagoya",
    "Sylhet",
    "Bremen",
    "Khulna",
    "Lille",
    "Krakow",
    "Ankara",
    "Chittagong",
    "Denver",
    "Malmo",
    "Cork",
    "Basel",
    "Perth",
    "Recife",
    "Hanoi",
    "Cebu",
    "Medan",
    "Rajshahi",
    "Bilbao",
    "Utrecht",
    "Ghent",
    "Tartu",
    "Split",
    "Brno",
    "Kosice",
    "Cluj",
    "Varna",
    "Patras",
    "Haifa",
    "Muscat",
    "Doha",
    "Sharjah",
    "Rabat",
    "Fez",
    "Arusha",
    "Kigali",
    "Gulu",
    "Mombasa",
    "Durban",
    "Windhoek",
    "Lusaka",
    "Harare",
    "Blantyre",
    "Beira",
    "Toamasina",
    "Moroni",
    "Victoria",
    "Galle",
    "Jaffna",
    "Pokhara",
    "Thimphu",
    "Male",
    "Sialkot",
    "Multan",
    "Herat",
    "Tabriz",
    "Shiraz",
    "Basra",
    "Mosul",
    "Aleppo",
    "Irbid",
    "Aqaba",
    "Tripoli",
    "Sfax",
    "Oran",
    "Annaba",
    "Nouakchott",
    "Dakar",
    "Bamako",
    "Niamey",
    "Kano",
    "Ibadan",
    "Kumasi",
    "Lome",
    "Cotonou",
    "Abidjan",
];
const REGION: &[&str] = &[
    "North", "South", "East", "West", "Central", "Coastal", "Highland", "Island", "Metro",
];
const SEGMENT: &[&str] = &["Consumer", "Corporate", "Home Office", "Education", "Government"];
const DEPT: &[&str] = &[
    "Grocery",
    "Electronics",
    "Home",
    "Garden",
    "Toys",
    "Books",
    "Sports",
    "Beauty",
    "Office",
];
const CARRIER: &[&str] = &[
    "SwiftShip",
    "BlueLine",
    "Parcelo",
    "RoadRunner",
    "SkyPost",
    "TerraFreight",
    "QuickBox",
    "Northwind Cargo",
    "Harbor Express",
];
const METHOD: &[&str] = &[
    "Card",
    "Cash",
    "Bank transfer",
    "Mobile wallet",
    "Gift card",
    "Voucher",
    "Cheque",
    "Crypto",
    "Store credit",
];
const JOB: &[&str] = &[
    "Cashier",
    "Clerk",
    "Supervisor",
    "Store manager",
    "Stocker",
    "Analyst",
    "Buyer",
    "Driver",
    "Technician",
];
const COUNTRY: &[&str] = &[
    "Portugal",
    "Bangladesh",
    "Japan",
    "Egypt",
    "France",
    "Ghana",
    "Germany",
    "India",
    "Brazil",
];
const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Column builder for one row: values are appended in header order.
struct Row<'a> {
    out: &'a mut String,
    first: bool,
}
impl Row<'_> {
    fn push(&mut self, v: impl std::fmt::Display) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        let s = v.to_string();
        if s.contains([',', '"', '\n']) {
            let _ = write!(self.out, "\"{}\"", s.replace('"', "\"\""));
        } else {
            self.out.push_str(&s);
        }
    }
}

fn date(x: u64) -> String {
    // 2021-01-01 .. 2025-12-28, valid days only.
    let y = 2021 + x % 5;
    let m = 1 + (x / 5) % 12;
    let d = 1 + (x / 60) % 28;
    format!("{y:04}-{m:02}-{d:02}")
}

fn money(x: u64, max_cents: u64) -> String {
    let c = x % max_cents;
    format!("{}.{:02}", c / 100, c % 100)
}

// --- embedded entities (the same facts appear in every file that copies them) -------------------

fn city_cols(prefix: &str) -> Vec<String> {
    [
        "city_code",
        "city_name",
        "city_population",
        "region_code",
        "region_name",
        "region_manager",
    ]
    .iter()
    .map(|c| format!("{prefix}{c}"))
    .collect()
}
fn city_vals(r: &mut Row, city: u64) {
    let region = city % REGION.len() as u64;
    r.push(format!("C{:03}", city + 1));
    r.push(CITY[city as usize % CITY.len()]);
    r.push(50_000 + h(7, city, 1) % 9_000_000);
    r.push(format!("R{}", region + 1));
    r.push(REGION[region as usize]);
    r.push(format!(
        "{} {}",
        pick(FIRST, h(8, region, 1)),
        pick(LAST, h(8, region, 2))
    ));
}

fn customer_name(id: u64) -> String {
    format!("{} {}", pick(FIRST, h(1, id, 1)), pick(LAST, h(1, id, 2)))
}
fn customer_city(id: u64) -> u64 {
    h(1, id, 3) % CITY.len() as u64
}
fn customer_segment(id: u64) -> &'static str {
    pick(SEGMENT, h(1, id, 4))
}
fn supplier_country(id: u64) -> &'static str {
    pick(COUNTRY, h(2, id, 1))
}
fn product_supplier(id: u64, suppliers: u64) -> u64 {
    1 + h(3, id, 1) % suppliers
}
fn product_category(id: u64) -> u64 {
    h(3, id, 2) % 99
}
fn product_price(id: u64) -> String {
    money(h(3, id, 3), 99_999)
}
fn store_city(id: u64) -> u64 {
    h(4, id, 1) % CITY.len() as u64
}
fn employee_store(id: u64, stores: u64) -> u64 {
    1 + h(5, id, 1) % stores
}

/// Measures that depend only on the row's key: high-cardinality numbers and a few flags.
fn fill_measures(header: &mut Vec<String>, names: &[&str]) {
    let mut i = 0;
    while header.len() < COLUMNS {
        let base = names[i % names.len()];
        let month = MONTHS[(i / names.len()) % MONTHS.len()];
        let year = 2024 + i / (names.len() * MONTHS.len());
        header.push(format!("{base}_{month}_{year}"));
        i += 1;
    }
}
fn measure_vals(r: &mut Row, table: u64, id: u64, from: usize) {
    for c in from..COLUMNS {
        let x = h(table * 1000 + 17, id, c as u64);
        match c % 4 {
            0 | 1 => r.push(money(x, 9_999_999)),
            2 => r.push(x % 9_999_999),
            _ => r.push(format!("{:.3}", (x % 1_000_000) as f64 / 1000.0)),
        }
    }
}

struct Sizes {
    customers: u64,
    suppliers: u64,
    products: u64,
    stores: u64,
    employees: u64,
    orders: u64,
    lines: u64,
    shipments: u64,
    payments: u64,
}

fn write_table(dir: &Path, name: &str, header: &[String], rows: u64, mut row: impl FnMut(u64, &mut Row)) -> PathBuf {
    assert_eq!(header.len(), COLUMNS, "{name} must have {COLUMNS} columns");
    let path = dir.join(format!("{name}.csv"));
    let mut w = BufWriter::new(File::create(&path).expect("create csv"));
    writeln!(w, "{}", header.join(",")).unwrap();
    let mut line = String::new();
    for i in 0..rows {
        line.clear();
        let mut r = Row {
            out: &mut line,
            first: true,
        };
        row(i, &mut r);
        line.push('\n');
        w.write_all(line.as_bytes()).unwrap();
    }
    w.flush().unwrap();
    path
}

/// One order table of `rows` × `cols` (cols ≥ 9) for scaling measurements.
fn write_single(dir: &Path, rows: u64, cols: usize) {
    let cols = cols.max(9);
    let mut header: Vec<String> = [
        "order_id",
        "order_date",
        "customer_id",
        "customer_name",
        "customer_segment",
        "city_code",
        "city_name",
        "region_code",
        "region_name",
    ]
    .map(String::from)
    .to_vec();
    for c in header.len()..cols {
        header.push(format!("measure_{:03}", c - 8));
    }
    let customers = (rows / 10).max(9);
    let path = dir.join(format!("orders_{rows}x{cols}.csv"));
    let mut w = BufWriter::new(File::create(&path).expect("create csv"));
    writeln!(w, "{}", header.join(",")).unwrap();
    let mut line = String::new();
    for i in 0..rows {
        line.clear();
        let mut r = Row {
            out: &mut line,
            first: true,
        };
        let id = i + 1;
        let cust = 1 + h(6, id, 1) % customers;
        let city = customer_city(cust);
        let region = city % REGION.len() as u64;
        r.push(id);
        r.push(date(h(6, id, 2)));
        r.push(cust);
        r.push(customer_name(cust));
        r.push(customer_segment(cust));
        r.push(format!("C{:03}", city + 1));
        r.push(CITY[city as usize % CITY.len()]);
        r.push(format!("R{}", region + 1));
        r.push(REGION[region as usize]);
        for c in 9..cols {
            let x = h(77, id, c as u64);
            if c % 2 == 0 {
                r.push(money(x, 9_999_999));
            } else {
                r.push(x % 9_999_999);
            }
        }
        line.push('\n');
        w.write_all(line.as_bytes()).unwrap();
    }
    w.flush().unwrap();
    eprintln!("wrote {}", path.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(
        args.iter()
            .find(|a| !a.starts_with("--") && a.parse::<u64>().is_err())
            .cloned()
            .unwrap_or("demo-data".into()),
    );
    if let Some(i) = args.iter().position(|a| a == "--single") {
        let rows: u64 = args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .expect("--single ROWS COLS");
        let cols: usize = args
            .get(i + 2)
            .and_then(|v| v.parse().ok())
            .expect("--single ROWS COLS");
        create_dir_all(&dir).expect("create output dir");
        write_single(&dir, rows, cols);
        return;
    }
    let small = args.windows(2).any(|w| w[0] == "--scale" && w[1] == "small");
    let s = if small {
        Sizes {
            customers: 999,
            suppliers: 999,
            products: 999,
            stores: 999,
            employees: 999,
            orders: 999,
            lines: 999,
            shipments: 999,
            payments: 999,
        }
    } else {
        Sizes {
            customers: 9_999,
            suppliers: 999,
            products: 9_999,
            stores: 999,
            employees: 9_999,
            orders: 99_999,
            lines: 99_999,
            shipments: 99_999,
            payments: 99_999,
        }
    };
    create_dir_all(&dir).expect("create output dir");

    // 1. customers: customer_id → name, contact, segment, city_code → … → region.
    let mut hd: Vec<String> = [
        "customer_id",
        "customer_name",
        "customer_email",
        "customer_phone",
        "customer_segment",
        "signup_date",
    ]
    .map(String::from)
    .to_vec();
    hd.extend(city_cols(""));
    let fixed = hd.len();
    fill_measures(&mut hd, &["spend", "visits", "basket", "loyalty"]);
    write_table(&dir, "customers", &hd, s.customers, |i, r| {
        let id = i + 1;
        r.push(id);
        r.push(customer_name(id));
        r.push(format!("customer{id}@example.org"));
        r.push(format!("+1-555-{:07}", id));
        r.push(customer_segment(id));
        r.push(date(h(1, id, 5)));
        city_vals(r, customer_city(id));
        measure_vals(r, 1, id, fixed);
    });

    // 2. suppliers: supplier_id → name, country, city.
    let mut hd: Vec<String> = [
        "supplier_id",
        "supplier_name",
        "supplier_country",
        "supplier_contact",
        "supplier_since",
    ]
    .map(String::from)
    .to_vec();
    hd.extend(city_cols(""));
    let fixed = hd.len();
    fill_measures(&mut hd, &["shipped", "returns", "lead_days", "rating"]);
    write_table(&dir, "suppliers", &hd, s.suppliers, |i, r| {
        let id = i + 1;
        r.push(id);
        r.push(format!("{} Trading {id}", pick(LAST, h(2, id, 2))));
        r.push(supplier_country(id));
        r.push(customer_name(id + 500_000));
        r.push(date(h(2, id, 3)));
        city_vals(r, h(2, id, 4) % CITY.len() as u64);
        measure_vals(r, 2, id, fixed);
    });

    // 3. products: product_id → supplier (copied: name, country), category_code → name, department.
    let mut hd: Vec<String> = [
        "product_id",
        "product_name",
        "unit_price",
        "supplier_id",
        "supplier_name",
        "supplier_country",
        "category_code",
        "category_name",
        "category_margin",
        "department_code",
        "department_name",
        "department_head",
    ]
    .map(String::from)
    .to_vec();
    let fixed = hd.len();
    fill_measures(&mut hd, &["units", "stock", "views", "returns"]);
    write_table(&dir, "products", &hd, s.products, |i, r| {
        let id = i + 1;
        let sup = product_supplier(id, s.suppliers);
        let cat = product_category(id);
        let dept = cat % DEPT.len() as u64;
        r.push(id);
        r.push(format!("{} item {id}", DEPT[dept as usize]));
        r.push(product_price(id));
        r.push(sup);
        r.push(format!("{} Trading {sup}", pick(LAST, h(2, sup, 2))));
        r.push(supplier_country(sup));
        r.push(format!("K{:02}", cat + 1));
        r.push(format!("{} line {}", DEPT[dept as usize], cat + 1));
        r.push(format!("0.{:02}", 5 + h(9, cat, 1) % 60));
        r.push(format!("D{}", dept + 1));
        r.push(DEPT[dept as usize]);
        r.push(format!(
            "{} {}",
            pick(FIRST, h(10, dept, 1)),
            pick(LAST, h(10, dept, 2))
        ));
        measure_vals(r, 3, id, fixed);
    });

    // 4. stores: store_id → name, opened, city → region.
    let mut hd: Vec<String> = ["store_id", "store_name", "store_opened", "store_sqm"]
        .map(String::from)
        .to_vec();
    hd.extend(city_cols(""));
    let fixed = hd.len();
    fill_measures(&mut hd, &["revenue", "footfall", "staff_hours", "energy"]);
    write_table(&dir, "stores", &hd, s.stores, |i, r| {
        let id = i + 1;
        let city = store_city(id);
        r.push(id);
        r.push(format!("{} #{id}", CITY[city as usize % CITY.len()]));
        r.push(date(h(4, id, 2)));
        r.push(200 + h(4, id, 3) % 9_000);
        city_vals(r, city);
        measure_vals(r, 4, id, fixed);
    });

    // 5. employees: employee_id → store_id (copied store_name), job_code → title, grade, base salary.
    let mut hd: Vec<String> = [
        "employee_id",
        "employee_name",
        "hire_date",
        "store_id",
        "store_name",
        "job_code",
        "job_title",
        "job_grade",
        "job_base_salary",
    ]
    .map(String::from)
    .to_vec();
    let fixed = hd.len();
    fill_measures(&mut hd, &["hours", "sales", "overtime", "score"]);
    write_table(&dir, "employees", &hd, s.employees, |i, r| {
        let id = i + 1;
        let store = employee_store(id, s.stores);
        let job = h(5, id, 2) % JOB.len() as u64;
        r.push(id);
        r.push(customer_name(id + 900_000));
        r.push(date(h(5, id, 3)));
        r.push(store);
        r.push(format!("{} #{store}", CITY[store_city(store) as usize % CITY.len()]));
        r.push(format!("J{}", job + 1));
        r.push(JOB[job as usize]);
        r.push(1 + job % 5);
        r.push(money(h(11, job, 1), 9_999_999));
        measure_vals(r, 5, id, fixed);
    });

    // 6. orders: order_id → customer (copied name, segment, city → region), store, employee.
    let mut hd: Vec<String> = [
        "order_id",
        "order_date",
        "customer_id",
        "customer_name",
        "customer_segment",
        "store_id",
        "employee_id",
        "channel",
        "priority",
    ]
    .map(String::from)
    .to_vec();
    hd.extend(city_cols("ship_"));
    let fixed = hd.len();
    fill_measures(&mut hd, &["subtotal", "discount", "tax", "points"]);
    write_table(&dir, "orders", &hd, s.orders, |i, r| {
        let id = i + 1;
        let cust = 1 + h(6, id, 1) % s.customers;
        r.push(id);
        r.push(date(h(6, id, 2)));
        r.push(cust);
        r.push(customer_name(cust));
        r.push(customer_segment(cust));
        r.push(1 + h(6, id, 3) % s.stores);
        r.push(1 + h(6, id, 4) % s.employees);
        r.push(pick(&["web", "store", "phone", "app"], h(6, id, 5)));
        r.push(pick(&["low", "normal", "high"], h(6, id, 6)));
        city_vals(r, customer_city(cust));
        measure_vals(r, 6, id, fixed);
    });

    // 7. order_lines: (order_id, line_no) → product (copied name, price), quantity, …
    let mut hd: Vec<String> = [
        "order_id",
        "line_no",
        "product_id",
        "product_name",
        "unit_price",
        "quantity",
        "line_total",
    ]
    .map(String::from)
    .to_vec();
    let fixed = hd.len();
    fill_measures(&mut hd, &["margin", "weight", "volume", "rebate"]);
    write_table(&dir, "order_lines", &hd, s.lines, |i, r| {
        // Orders 1..: each has 1–3 lines, enough orders to cover the requested line count.
        let order = 1 + i / 3;
        let line = 1 + i % 3;
        let prod = 1 + h(7, i, 1) % s.products;
        let qty = 1 + h(7, i, 2) % 9;
        let price_cents: u64 = product_price(prod).replace('.', "").parse().unwrap_or(0);
        let total = price_cents * qty;
        r.push(order.min(s.orders));
        r.push(line);
        r.push(prod);
        r.push(format!(
            "{} item {prod}",
            DEPT[(product_category(prod) % DEPT.len() as u64) as usize]
        ));
        r.push(product_price(prod));
        r.push(qty);
        r.push(format!("{}.{:02}", total / 100, total % 100));
        measure_vals(r, 7, i + 1, fixed);
    });

    // 8. shipments: shipment_id → order_id, carrier_code → carrier name, phone, rating.
    let mut hd: Vec<String> = [
        "shipment_id",
        "order_id",
        "shipped_on",
        "carrier_code",
        "carrier_name",
        "carrier_phone",
        "carrier_rating",
        "tracking_no",
    ]
    .map(String::from)
    .to_vec();
    let fixed = hd.len();
    fill_measures(&mut hd, &["transit_hours", "weight", "distance", "cost"]);
    write_table(&dir, "shipments", &hd, s.shipments, |i, r| {
        let id = i + 1;
        let carrier = h(8, id, 1) % CARRIER.len() as u64;
        r.push(id);
        r.push(1 + h(8, id, 2) % s.orders);
        r.push(date(h(8, id, 3)));
        r.push(format!("CR{}", carrier + 1));
        r.push(CARRIER[carrier as usize]);
        r.push(format!("+1-800-{:07}", 1_000_000 + carrier * 99_999));
        r.push(format!("{}.{}", 3 + carrier % 3, carrier % 10));
        r.push(format!("TRK{:09}", h(8, id, 4) % 999_999_999));
        measure_vals(r, 8, id, fixed);
    });

    // 9. payments: payment_id → order_id, method_code → method name, fee.
    let mut hd: Vec<String> = [
        "payment_id",
        "order_id",
        "paid_on",
        "method_code",
        "method_name",
        "method_fee_pct",
        "amount",
        "currency",
    ]
    .map(String::from)
    .to_vec();
    let fixed = hd.len();
    fill_measures(&mut hd, &["fx_rate", "fee", "refund", "risk"]);
    write_table(&dir, "payments", &hd, s.payments, |i, r| {
        let id = i + 1;
        let method = h(9, id, 1) % METHOD.len() as u64;
        r.push(id);
        r.push(1 + h(9, id, 2) % s.orders);
        r.push(date(h(9, id, 3)));
        r.push(format!("M{}", method + 1));
        r.push(METHOD[method as usize]);
        r.push(format!("{}.{}", method % 4, (method * 7) % 10));
        r.push(money(h(9, id, 4), 9_999_999));
        r.push("USD");
        measure_vals(r, 9, id, fixed);
    });

    eprintln!("wrote 9 tables × {COLUMNS} columns to {}", dir.display());
}
