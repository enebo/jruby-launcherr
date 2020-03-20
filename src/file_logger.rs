use std::sync::Mutex;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use log::{LevelFilter, Metadata, Level, Record, SetLoggerError};

pub fn init(path: &Path) -> Result<(), SetLoggerError> {
    FileLogger::init(path)
}

struct FileLogger {
    file: Mutex<File>,
}

impl FileLogger {
    pub fn init(path: &Path) -> Result<(), SetLoggerError> {
        if path.exists() {
            fs::remove_file(path).expect("Could not remove old log file");
        }
        let logger = FileLogger { file: Mutex::new(File::create(path).unwrap()) };
        log::set_boxed_logger(Box::new(logger))
            .map(|()| log::set_max_level(LevelFilter::Info))
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            write!(self.file.lock().unwrap(), "{}: {}", record.level(), record.args()).expect("Could not write to log file");
        }
    }

    fn flush(&self) {}
}
