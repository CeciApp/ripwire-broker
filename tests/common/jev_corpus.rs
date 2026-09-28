//! The synthetic corpus of the S4.0b live recording (D-067): invented text, never read from
//! a workspace. Shared by `examples/jev_record.rs` and `tests/online_units.rs`.
use ripwire_broker::online::SemanticStage;
use ripwire_broker::online::request::{JevRequest, StateItem, build};

const QUERY: &str = "how are login tokens validated?";

fn item(id: &str, path: &str, text: &str) -> StateItem {
    StateItem {
        id: id.into(),
        path: path.into(),
        text: text.into(),
    }
}

/// The requests the live recording sent, in order.
pub fn requests() -> Vec<JevRequest> {
    let files = vec![
        item(
            "i0",
            "src/auth.py",
            "def validate_token(token):\n    return token == \"ok\"\n\n\ndef login(user, token):\n    if not validate_token(token):\n        raise ValueError(\"bad token\")\n    return user\n",
        ),
        item(
            "i1",
            "tests/test_auth.py",
            "from src.auth import login\n\n\ndef test_login():\n    assert login(\"a\", \"ok\") == \"a\"\n",
        ),
        item(
            "i2",
            "src/csv_report.py",
            "import csv\n\n\ndef write_report(rows, out):\n    w = csv.writer(out)\n    for r in rows:\n        w.writerow(r)\n",
        ),
    ];
    let blocks = vec![
        item(
            "i0",
            "src/auth.py",
            "def validate_token(token):\n    return token == \"ok\"\n",
        ),
        item(
            "i1",
            "src/auth.py",
            "def login(user, token):\n    if not validate_token(token):\n        raise ValueError(\"bad token\")\n    return user\n",
        ),
    ];
    vec![
        build("jev-1.13.0", QUERY, SemanticStage::FileAdmission, files),
        build("jev-1.13.0", QUERY, SemanticStage::SourceSelection, blocks),
    ]
}
