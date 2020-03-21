use std::sync::Mutex;
use std::{fs, io};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use log::{LevelFilter, Metadata, Level, Record, SetLoggerError};

pub fn init(path: Option<PathBuf>) -> Result<(), FileLoggerError> {
    FileLogger::init(path)
}

#[derive(Debug)]
pub struct FileLoggerError {
    reason: String,
}

struct FileLogger {
    file: Mutex<Option<File>>,
}

impl FileLogger {
    pub fn init(opt: Option<PathBuf>) -> Result<(), FileLoggerError> {
        let logger = if opt.is_none() {
            Mutex::new(None)
        } else {
            let path = opt.as_ref().unwrap();

            if path.exists() {
                fs::remove_file(path).expect("Could not remove old log file");
            }

            let file = File::create(path);

            if file.is_err() {
                return Err(FileLoggerError { reason: file.err().unwrap().to_string() })
            }

            Mutex::new(Some(file.unwrap()))
        };
        if log::set_boxed_logger(Box::new(FileLogger { file: logger }))
            .map(|()| log::set_max_level(LevelFilter::Info)).is_err() {
            return Err(FileLoggerError { reason: "could not set logger".to_string()})
        }
        Ok(())
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut guard = self.file.lock().unwrap();
            match guard.as_mut() {
                Some(f) => write!(f, "{}: {}\n", record.level(), record.args()).expect("Could not write to log file"),
                None => write!(io::stdout(), "{}: {}\n", record.level(), record.args()).expect("Could not write to log file"),
            };
        }
    }

    fn flush(&self) {}
}
