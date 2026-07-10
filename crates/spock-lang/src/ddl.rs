//! SQLite DDL emission (docs/spec/v0.md §7.1). One `CREATE TABLE` per
//! contract table, declaration order preserved, all identifiers quoted.

use crate::ir::{Contract, Type};

/// Emit `CREATE TABLE` statements for every table, in declaration order.
pub fn ddl(contract: &Contract) -> Vec<String> {
    contract
        .tables
        .iter()
        .map(|table| {
            let mut lines: Vec<String> = Vec::new();

            for field in &table.fields {
                let storage = contract.storage_type(&field.ty).sql();
                let null = if field.optional { "" } else { " NOT NULL" };
                lines.push(format!("  \"{}\" {storage}{null}", field.name));
            }

            let key_list = quote_list(&table.key);
            lines.push(format!("  PRIMARY KEY ({key_list})"));

            for field in &table.fields {
                if field.unique {
                    lines.push(format!("  UNIQUE (\"{}\")", field.name));
                }
            }
            for group in &table.uniques {
                lines.push(format!("  UNIQUE ({})", quote_list(group)));
            }

            for field in &table.fields {
                if let Type::Ref { table: target, on_delete } = &field.ty {
                    let target_key = &contract
                        .table(target)
                        .expect("checked: ref target exists")
                        .key[0];
                    let action = match on_delete {
                        crate::ir::OnDelete::Restrict => "RESTRICT",
                        crate::ir::OnDelete::Cascade => "CASCADE",
                        crate::ir::OnDelete::SetNull => "SET NULL",
                    };
                    lines.push(format!(
                        "  FOREIGN KEY (\"{}\") REFERENCES \"{target}\" (\"{target_key}\") ON DELETE {action}",
                        field.name
                    ));
                }
            }

            format!(
                "CREATE TABLE \"{}\" (\n{}\n);",
                table.name,
                lines.join(",\n")
            )
        })
        .collect()
}

fn quote_list(names: &[String]) -> String {
    names
        .iter()
        .map(|n| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    #[test]
    fn emits_expected_schema() {
        let contract = compile(
            "table user {\n\
               key id: uuid = auto\n\
               username: text unique\n\
               bio: text?\n\
               joined_at: timestamp = now\n\
             }\n\
             table post {\n\
               key id: uuid = auto\n\
               author: user\n\
               caption: text?\n\
               pinned: bool = false\n\
             }\n\
             table follow {\n\
               key (follower, target)\n\
               follower: user\n\
               target: user on delete cascade\n\
             }",
        )
        .unwrap();

        let statements = ddl(&contract);
        assert_eq!(statements.len(), 3);

        assert_eq!(
            statements[0],
            "CREATE TABLE \"user\" (\n\
             \x20 \"id\" TEXT NOT NULL,\n\
             \x20 \"username\" TEXT NOT NULL,\n\
             \x20 \"bio\" TEXT,\n\
             \x20 \"joined_at\" TEXT NOT NULL,\n\
             \x20 PRIMARY KEY (\"id\"),\n\
             \x20 UNIQUE (\"username\")\n\
             );"
        );

        // bool → INTEGER; the ref column takes the target key's storage type
        assert!(statements[1].contains("\"pinned\" INTEGER NOT NULL"));
        assert!(statements[1].contains("\"author\" TEXT NOT NULL"));
        assert!(statements[1]
            .contains("FOREIGN KEY (\"author\") REFERENCES \"user\" (\"id\") ON DELETE RESTRICT"));

        assert!(statements[2].contains("PRIMARY KEY (\"follower\", \"target\")"));
        assert!(statements[2]
            .contains("FOREIGN KEY (\"target\") REFERENCES \"user\" (\"id\") ON DELETE CASCADE"));
    }

    #[test]
    fn emits_real_for_float() {
        let contract =
            compile("table tag { key id: uuid = auto\n x: float\n y: float? }").unwrap();
        let statements = ddl(&contract);
        assert!(statements[0].contains("\"x\" REAL NOT NULL"));
        assert!(statements[0].contains("\"y\" REAL,"));
    }

    #[test]
    fn emits_set_null() {
        let contract = compile(
            "table user { key id: uuid = auto }\n\
             table comment {\n\
               key id: uuid = auto\n\
               author: user\n\
               parent: comment? on delete set null\n\
             }",
        )
        .unwrap();
        let statements = ddl(&contract);
        assert!(statements[1]
            .contains("FOREIGN KEY (\"parent\") REFERENCES \"comment\" (\"id\") ON DELETE SET NULL"));
    }
}
