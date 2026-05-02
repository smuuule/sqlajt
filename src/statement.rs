use crate::database::{Cursor, Row, Table};

#[derive(Debug)]
pub enum StatementType {
    Insert(Row),
    Select,
}

#[derive(Debug)]
pub struct Statement {
    pub statement_type: StatementType,
}

pub fn prepare_statement(input: &str) -> Result<Statement, String> {
    if input.starts_with("insert") {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 4 {
            return Err("Syntax error. Expected: insert <id> <username> <email>".to_string());
        }

        let id = parts[1]
            .parse::<u32>()
            .map_err(|_| "ID must be a positive integer".to_string())?;
        let username = parts[2].to_string();
        let email = parts[3].to_string();

        Ok(Statement {
            statement_type: StatementType::Insert(Row {
                id,
                username,
                email,
            }),
        })
    } else if input.starts_with("select") {
        Ok(Statement {
            statement_type: StatementType::Select,
        })
    } else {
        Err(format!("Unrecognized keyword at start of '{}'", input))
    }
}

pub fn execute_statement(statement: Statement, table: &mut Table) {
    match statement.statement_type {
        StatementType::Insert(row) => {
            let mut cursor = Cursor::table_end(table);
            cursor.insert_row(row);
        }
        StatementType::Select => {
            let mut cursor = Cursor::table_start(table);
            while !cursor.end_of_table {
                let row = cursor.get_row();
                println!("({}, {}, {})", row.id, row.username, row.email);
                cursor.advance();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_insert_success() {
        let stmt = prepare_statement("insert 1 tester tester@testing.org").unwrap();
        match stmt.statement_type {
            StatementType::Insert(row) => {
                assert_eq!(row.id, 1);
                assert_eq!(row.username, "tester");
                assert_eq!(row.email, "tester@testing.org");
            }
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_prepare_select_success() {
        let stmt = prepare_statement("select").unwrap();
        assert!(matches!(stmt.statement_type, StatementType::Select));
    }

    #[test]
    fn test_prepare_insert_negative_id() {
        let err = prepare_statement("insert -1 tester tester@testing.org").unwrap_err();
        assert_eq!(err, "ID must be a positive integer");
    }

    #[test]
    fn test_prepare_insert_syntax_error() {
        let err = prepare_statement("insert 1 tester").unwrap_err();
        assert_eq!(
            err,
            "Syntax error. Expected: insert <id> <username> <email>"
        );
    }
}
