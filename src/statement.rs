pub enum StatementType {
    Insert,
}

pub struct Statement {
    pub statement_type: StatementType,
}

pub fn prepare_statement(input: &str) -> Result<Statement, String> {
    if input.starts_with("insert") {
        Ok(Statement {
            statement_type: StatementType::Insert,
        })
    } else {
        Err(format!("Unrecognized keyword at start of '{}'", input))
    }
}

pub fn execute_statement(statement: &Statement) {
    match statement.statement_type {
        StatementType::Insert => {
            println!("Executed insert.");
        }
    }
}
