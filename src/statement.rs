use crate::database::{Column, DataType, PAGE_SIZE, Schema, Table, Value};
use sqlparser::ast::{
    DataType as SqlDataType, Expr, SetExpr, Statement as SqlStatement, Value as SqlValue,
};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;
use std::convert::TryInto;

#[derive(Debug)]
pub struct Statement {
    pub ast: SqlStatement,
}

pub fn prepare_statement(input: &str) -> Result<Statement, String> {
    let dialect = SQLiteDialect {};
    let mut asts =
        Parser::parse_sql(&dialect, input).map_err(|e| format!("Parse Error: {:?}", e))?;
    if asts.is_empty() {
        return Err("No statement found".to_string());
    }
    Ok(Statement {
        ast: asts.remove(0),
    })
}

pub fn execute_statement(statement: Statement, table: &mut Table) {
    match statement.ast {
        SqlStatement::CreateTable(create_table) => {
            let mut cols = Vec::new();
            for col in create_table.columns {
                let dt = match col.data_type {
                    SqlDataType::Int(_) | SqlDataType::Integer(_) => DataType::Int,
                    SqlDataType::Varchar(_) | SqlDataType::String(_) | SqlDataType::Text => {
                        DataType::Text
                    }
                    _ => {
                        println!("Unsupported data type for column: {}", col.name);
                        return;
                    }
                };
                cols.push(Column {
                    name: col.name.value,
                    data_type: dt,
                });
            }
            table.schema = Some(Schema::new(cols));
            table.num_rows = 0;
            table.save_metadata();
            println!("Table {} created.", create_table.name);
        }
        SqlStatement::Insert(insert) => {
            let num_cols = if let Some(schema) = &table.schema {
                schema.columns.len()
            } else {
                println!("No table created yet.");
                return;
            };

            if let Some(source) = insert.source {
                let body = source.body.as_ref();
                if let SetExpr::Values(values) = body {
                    for row in &values.rows {
                        let mut row_vals = Vec::new();
                        for (i, expr) in row.iter().enumerate() {
                            if i >= num_cols {
                                println!("Too many values");
                                return;
                            }
                            match expr {
                                Expr::Value(val) => match val {
                                    sqlparser::ast::ValueWithSpan {
                                        value: SqlValue::Number(n, _),
                                        ..
                                    } => {
                                        if let Ok(num) = n.parse::<i32>() {
                                            row_vals.push(Value::Int(num));
                                        } else {
                                            println!("Invalid integer");
                                            return;
                                        }
                                    }
                                    sqlparser::ast::ValueWithSpan {
                                        value: SqlValue::SingleQuotedString(s),
                                        ..
                                    } => {
                                        row_vals.push(Value::Text(s.clone()));
                                    }
                                    _ => {
                                        // Fallback for direct value
                                        println!("Unsupported value type fallback");
                                    }
                                },
                                _ => {
                                    println!("Unsupported value type");
                                    return;
                                }
                            }
                        }
                        if let Err(e) = table.insert_row(row_vals) {
                            println!("Insert error: {}", e);
                            return;
                        }
                    }
                    println!("Inserted {} row(s).", values.rows.len());
                } else {
                    println!("Unsupported INSERT format");
                }
            }
        }
        SqlStatement::Query(_) => {
            if let Some(schema) = &table.schema {
                if table.num_rows == 0 {
                    println!("(Empty set)");
                    return;
                }

                let rows_per_page = PAGE_SIZE / schema.row_size;

                for row_idx in 0..table.num_rows {
                    let page_num = 1 + (row_idx / rows_per_page as u32);
                    let page = table.pager.get_page(page_num);
                    let row_offset = ((row_idx as usize) % rows_per_page) * schema.row_size;

                    let mut byte_offset = row_offset;
                    let mut output = Vec::new();

                    for col in &schema.columns {
                        match col.data_type {
                            DataType::Int => {
                                let n = i32::from_le_bytes(
                                    page.data[byte_offset..byte_offset + 4].try_into().unwrap(),
                                );
                                output.push(n.to_string());
                                byte_offset += 4;
                            }
                            DataType::Text => {
                                let len = page.data[byte_offset] as usize;
                                let s = String::from_utf8(
                                    page.data[byte_offset + 1..byte_offset + 1 + len].to_vec(),
                                )
                                .unwrap_or_default();
                                output.push(s);
                                byte_offset += 256;
                            }
                        }
                    }
                    println!("{}", output.join(" | "));
                }
                println!("{} row(s) returned.", table.num_rows);
            } else {
                println!("No table created yet.");
            }
        }
        _ => {
            println!("Unsupported statement.");
        }
    }
}
