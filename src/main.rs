use std::fs;

fn display_directory() {
    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                let entry_name = file_name.to_string_lossy();
                println!("{}", entry_name);
            }
        }
    }
}

fn main() {
    display_directory();
}
