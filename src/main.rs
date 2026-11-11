use std::io::{self, BufRead, Read, Write};

fn render(input: &str) -> Result<String, String> {
    let request: serde_json::Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let types = request
        .as_array()
        .or_else(|| request.get("types").and_then(|v| v.as_array()))
        .or_else(|| request.get("typeDefinitions").and_then(|v| v.as_array()))
        .ok_or("expected a type array or {types: [...], options: {...}}")?;
    bendec2rs::generate_string(
        types,
        request.get("options").unwrap_or(&serde_json::Value::Null),
    )
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut input = None;
    let mut output = None;
    let mut batch = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--batch" => batch = true,
            "--output" => output = Some(args.next().ok_or("--output requires a path")?),
            "--version" | "-V" => {
                println!("bendec2rs {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("bendec2rs [INPUT.json] [--output NEW.rs]\nbendec2rs --batch < requests.jsonl\nInput: type array or {{\"types\": [...], \"options\": {{...}}}}.\nBatch: one request per line; JSON result strings on stdout. Files are never overwritten.");
                return Ok(());
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ if input.is_none() => input = Some(arg),
            _ => return Err("only one input path is accepted".into()),
        }
    }
    if batch {
        if input.is_some() || output.is_some() {
            return Err("--batch cannot be combined with paths".into());
        }
        let stdout = io::stdout();
        let mut writer = io::BufWriter::new(stdout.lock());
        for line in io::stdin().lock().lines() {
            let generated = render(&line.map_err(|e| e.to_string())?)?;
            serde_json::to_writer(&mut writer, &generated).map_err(|e| e.to_string())?;
            writeln!(writer).map_err(|e| e.to_string())?;
        }
        writer.flush().map_err(|e| e.to_string())?;
    } else {
        let source = if let Some(path) = input {
            std::fs::read_to_string(path).map_err(|e| e.to_string())?
        } else {
            let mut s = String::new();
            io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s
        };
        let generated = render(&source)?;
        if let Some(path) = output {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            file.write_all(generated.as_bytes())
                .map_err(|e| e.to_string())?;
        } else {
            io::stdout()
                .lock()
                .write_all(generated.as_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("bendec2rs: {error}");
        std::process::exit(1);
    }
}
