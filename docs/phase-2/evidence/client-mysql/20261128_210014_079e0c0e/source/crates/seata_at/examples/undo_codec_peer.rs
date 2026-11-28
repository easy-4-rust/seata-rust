//! 固定资产的编解码对照驱动；不访问数据库，不把本地往返当Java互通证明。
use seata_at::JacksonUndoLogParser;
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("input directory required")?);
    let output = PathBuf::from(args.next().ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let parser = JacksonUndoLogParser::default();
    let mut cases = 0;
    let mut files = fs::read_dir(input)?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(|file| file.path());
    for file in files {
        if file
            .path()
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let bytes = fs::read(file.path())?;
            let document = parser.decode(&bytes)?;
            fs::write(
                output.join(file.file_name()),
                parser.encode(document.as_ref())?,
            )?;
            cases += 1;
        }
    }
    println!("rust_undo_document_cases={cases}");
    Ok(())
}
