use std::{error::Error, fmt::Write as FmtWrite, io::Write, sync::Mutex};
static OUTPUT: Mutex<()> = Mutex::new(());

pub fn record(class: &str, method: &str, values: &[String]) -> Result<(), Box<dyn Error>> {
    if let Some(path) = std::env::var_os("SEATA_CORE_OBSERVATIONS") {
        let _guard = OUTPUT.lock().map_err(|_| "Observation lock poisoned")?;
        let mut hex = String::new();
        for byte in values.join("\u{1f}").bytes() {
            write!(&mut hex, "{byte:02x}")?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "org.apache.seata.sqlparser.{class}#{method}\t{hex}")?;
    }
    Ok(())
}
