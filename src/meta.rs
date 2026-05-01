pub fn handle_meta_command(command: &str) -> Result<bool, String> {
    match command {
        ".exit" => Ok(false),
        ".help" => {
            println!("Available commands:");
            println!("  .exit - Exit the program");
            println!("  .help - Show this message");
            Ok(true)
        }
        _ => Err(format!("Unrecognized command '{}'", command)),
    }
}
