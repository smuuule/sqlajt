mod meta;
mod statement;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

fn spawn_input_handler()
-> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    tokio::spawn(async {
        let std_in = tokio::io::stdin();
        let std_out = tokio::io::stdout();

        let mut reader = tokio::io::BufReader::new(std_in).lines();
        let mut stdout = tokio::io::BufWriter::new(std_out);

        let start_string = concat!(
            "SQLajt version 0.0.1\n",
            "Enter \".help\" for usage hints.\n",
        );
        let prompt_string = "sqlajt> ";

        stdout.write_all(start_string.as_bytes()).await?;
        stdout.write_all(prompt_string.as_bytes()).await?;
        stdout.flush().await?;

        let mut table = statement::Table::new();

        while let Some(line) = reader.next_line().await? {
            let command = line.trim();

            if command.starts_with('.') {
                match meta::handle_meta_command(command) {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(e) => println!("{}", e),
                }
            } else {
                match statement::prepare_statement(command) {
                    Ok(stmt) => statement::execute_statement(stmt, &mut table),
                    Err(e) => println!("{}", e),
                }
            }

            stdout.write_all(prompt_string.as_bytes()).await?;
            stdout.flush().await?;
        }
        Ok(())
    })
}

#[tokio::main]
async fn main() {
    let input_handler = spawn_input_handler();

    match input_handler.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("REPL error: {}", e),
        Err(e) => eprintln!("Task join error: {}", e),
    }
}
